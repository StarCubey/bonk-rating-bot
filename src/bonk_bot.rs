//pub mod bonk_commands;
pub mod bonk_commands;
pub mod bonk_room;
pub mod events;
pub mod room_maker;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serenity::prelude::TypeMapKey;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{select, time};

use self::bonk_room::BonkRoomMessage;
use self::room_maker::{RoomMaker, RoomMakerMessage};
use crate::bonk_bot::room_maker::RoomParameters;
use crate::leaderboard::{Leaderboard, LeaderboardMessage, LeaderboardSettings};

#[derive(Clone)]
pub struct BonkRoom {
    link: String,
    parameters: RoomParameters,
    tx: mpsc::Sender<BonkRoomMessage>,
}

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

/// When acquiring locks outside of bonk_bot.rs, use try lock or limit backpressure.
#[derive(Clone)]
pub struct BonkBotValue {
    bonk_rooms: Arc<Mutex<Vec<BonkRoom>>>,
    roommaker_tx: mpsc::Sender<RoomMakerMessage>,
    leaderboards_tx: Arc<Mutex<Vec<(i64, mpsc::WeakSender<LeaderboardMessage>)>>>,
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
            bonk_rooms: Arc::new(Mutex::new(Vec::new())),
            roommaker_tx,
            leaderboards_tx: Arc::new(Mutex::new(Vec::new())),
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

                tokio::spawn(async move { leaderboard.run().await });

                leaderboards_tx.push((id, tx.clone().downgrade()));
                leaderboard_tx = Some(tx);
            }
        }

        let (tx, rx) = oneshot::channel();
        self.roommaker_tx
            .send(RoomMakerMessage {
                http: ctx.http.clone(),
                data: ctx.data.clone(),
                bonkroom_tx: tx,
                leaderboard_tx,
                room_parameters: room_parameters.clone(),
            })
            .await?;

        let output = rx.await??;

        let mut bonk_rooms = self.bonk_rooms.lock().await;
        bonk_rooms.push(BonkRoom {
            link: output.room_link.clone(),
            parameters: room_parameters,
            tx: output.bonkroom_tx,
        });

        Ok(output.room_link)
    }

    pub async fn close_all(&mut self) -> Result<()> {
        let bonk_rooms = self.bonk_rooms.lock().await;

        for i in 0..bonk_rooms.len() {
            if let Err(_) = bonk_rooms
                .get(i)
                .context("Index out of bounds")?
                .tx
                .send(BonkRoomMessage::Close)
                .await
            {
                println!("Room already closed.");
            };
        }

        let bonk_rooms_clone = bonk_rooms
            .iter()
            .map(|r| r.tx.clone())
            .collect::<Vec<mpsc::Sender<BonkRoomMessage>>>();
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
            let _ = room.tx.send(BonkRoomMessage::ForceClose).await;
        }

        result
    }

    pub async fn force_close_all(&mut self) -> Result<()> {
        let mut bonk_rooms = self.bonk_rooms.lock().await;

        for i in (0..bonk_rooms.len()).rev() {
            let _ = bonk_rooms
                .get(i)
                .context("Index out of bounds")?
                .tx
                .send(BonkRoomMessage::ForceClose)
                .await;

            bonk_rooms.pop();
        }

        Ok(())
    }
}
