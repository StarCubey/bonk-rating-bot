//pub mod bonk_commands;
pub mod bonk_commands;
pub mod bonk_room;
pub mod events;
pub mod room_maker;

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serenity::prelude::TypeMapKey;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{select, time};

use self::bonk_room::BonkRoomMessage;
use self::room_maker::{RoomMaker, RoomMakerMessage};
use crate::leaderboard::{Leaderboard, LeaderboardMessage, LeaderboardSettings};

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

pub struct BonkBotValue {
    bonk_rooms: Mutex<Vec<mpsc::Sender<BonkRoomMessage>>>,
    roommaker_tx: mpsc::Sender<RoomMakerMessage>,
    leaderboards_tx: Mutex<Vec<(i64, mpsc::WeakSender<LeaderboardMessage>)>>,
}

impl BonkBotValue {
    ///Panics
    pub async fn new() -> BonkBotValue {
        let (roommaker_tx, roommaker_receiver) = mpsc::channel(3);
        let mut roommaker = RoomMaker::new(roommaker_receiver)
            .await
            .expect("Failed to initialize room maker.");
        tokio::spawn(async move {
            roommaker.run().await;
        });

        BonkBotValue {
            bonk_rooms: Mutex::new(Vec::new()),
            roommaker_tx,
            leaderboards_tx: Mutex::new(Vec::new()),
        }
    }

    pub async fn open_room(
        &self,
        ctx: &serenity::all::Context,
        room_parameters: room_maker::RoomParameters,
    ) -> Result<String> {
        let mut leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>> = None;

        let data = ctx.data.read().await;
        let db = data
            .get::<crate::DatabaseKey>()
            .cloned()
            .ok_or(anyhow!("Failed to connect to database."))?
            .db;

        if let Some(lb) = &room_parameters.leaderboard {
            let rows: Vec<(i64, serde_json::Value)> =
                sqlx::query_as("SELECT id, settings FROM leaderboard WHERE abbreviation = $1")
                    .bind(lb)
                    .fetch_all(db.as_ref())
                    .await?;

            if rows.len() < 1 {
                return Err(anyhow!("Leaderboard not found."));
            }

            let id = rows.get(0).context("Error while loading leaderboard.")?.0;
            let settings: LeaderboardSettings = serde_json::from_value(
                rows.get(0)
                    .context("Error while loading leaderboard.")?
                    .1
                    .clone(),
            )?;

            let mut leaderboards_tx = self.leaderboards_tx.lock().await;
            leaderboards_tx.retain(|x| x.1.strong_count() > 0);
            let leaderboard_wtx = leaderboards_tx.iter().find(|x| x.0 == id);

            if let Some((_, leaderboard_wtx)) = leaderboard_wtx {
                leaderboard_tx = leaderboard_wtx.clone().upgrade();
            } else {
                let (tx, rx) = mpsc::channel(10);
                let mut leaderboard = Leaderboard::new(rx, ctx.clone(), settings).await?;

                tokio::spawn(async move {
                    let res = leaderboard.run().await;
                    if let Result::Err(err) = res {
                        println!("Leaderboard error: {}", err)
                    }
                });

                leaderboards_tx.push((id, tx.clone().downgrade()));
                leaderboard_tx = Some(tx);
            }
        }

        let (tx, rx) = oneshot::channel();
        self.roommaker_tx
            .send(RoomMakerMessage {
                bonkroom_tx: tx,
                leaderboard_tx,
                room_parameters,
            })
            .await?;

        let output = rx.await??;

        let mut bonk_rooms = self.bonk_rooms.lock().await;
        bonk_rooms.push(output.bonkroom_tx);

        Ok(output.room_link)
    }

    pub async fn close_all(&mut self) -> Result<()> {
        let bonk_rooms = self.bonk_rooms.lock().await;

        for i in 0..bonk_rooms.len() {
            bonk_rooms
                .get(i)
                .context("Index out of bounds")?
                .send(BonkRoomMessage::Close)
                .await?;
        }

        let bonk_rooms_clone = bonk_rooms.clone();
        let all_closed = tokio::spawn(async {
            for room in bonk_rooms_clone {
                room.closed().await;
            }
        });

        let sleep = Box::pin(time::sleep(Duration::from_secs(600)));
        let result;
        select! {
            _ = sleep => result = Err(anyhow!("Rooms force closed due to 10 minute timeout.")),
            _ = all_closed => result = Ok(()),
        };

        for room in bonk_rooms.iter() {
            let _ = room.send(BonkRoomMessage::ForceClose).await;
        }

        result
    }

    pub async fn force_close_all(&mut self) -> Result<()> {
        let mut bonk_rooms = self.bonk_rooms.lock().await;

        for i in (0..bonk_rooms.len()).rev() {
            bonk_rooms
                .get(i)
                .context("Index out of bounds")?
                .send(BonkRoomMessage::ForceClose)
                .await?;

            bonk_rooms.pop();
        }

        Ok(())
    }
}
