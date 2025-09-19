pub mod bonk_commands;
pub mod bonk_room;
pub mod room_maker;

use anyhow::{anyhow, Context, Result};
use serenity::prelude::TypeMapKey;
use tokio::sync::{mpsc, oneshot, Mutex};

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
    leaderboards_tx: Vec<(i32, mpsc::WeakSender<LeaderboardMessage>)>,
}

impl BonkBotValue {
    pub fn new() -> BonkBotValue {
        let (roommaker_tx, roommaker_receiver) = mpsc::channel(3);
        let mut roommaker = RoomMaker::new(roommaker_receiver);
        tokio::spawn(async move {
            roommaker.run().await;
        });

        BonkBotValue {
            bonk_rooms: Mutex::new(Vec::new()),
            roommaker_tx,
            leaderboards_tx: Vec::new(),
        }
    }

    pub async fn open_room(
        &mut self,
        ctx: &serenity::all::Context,
        room_parameters: room_maker::RoomParameters,
    ) -> Result<String> {
        let db = {
            let data = ctx.data.read().await;
            data.get::<crate::DatabaseKey>().cloned()
        }
        .ok_or(anyhow!("Failed to connect to database."))?;

        let mut leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>> = None;

        if let Some(lb) = &room_parameters.leaderboard {
            let rows: Vec<(i32, serde_json::Value)> =
                sqlx::query_as("SELECT (id, settings) FROM leaderboard WHERE abbreviation = ($1)")
                    .bind(lb)
                    .fetch_all(db.db.as_ref())
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

            self.leaderboards_tx.retain(|x| x.1.strong_count() > 0);
            let leaderboard_wtx = self.leaderboards_tx.iter().find(|x| x.0 == id);

            if let Some((_, leaderboard_wtx)) = leaderboard_wtx {
                leaderboard_tx = leaderboard_wtx.clone().upgrade();
            } else {
                let (tx, rx) = mpsc::channel(10);
                let mut leaderboard = Leaderboard::new(rx, settings);

                tokio::spawn(async move {
                    leaderboard.run().await;
                });

                leaderboard_tx = Some(tx);
                self.leaderboards_tx.push((id, tx.clone().downgrade()));
            }
        }

        //TODO RoomMakerMessage must be updated to accept an optional leaderboard_tx
        let (tx, rx) = oneshot::channel();
        self.roommaker_tx
            .send(RoomMakerMessage {
                bonkroom_tx: tx,
                room_parameters,
            })
            .await?;

        let output = rx.await??;

        let mut bonk_rooms = self.bonk_rooms.lock().await;
        bonk_rooms.push(output.bonkroom_tx);

        Ok(output.room_link)
    }

    pub async fn close_all(&mut self) -> Result<()> {
        let mut bonk_rooms = self.bonk_rooms.lock().await;

        for i in (0..bonk_rooms.len()).rev() {
            bonk_rooms
                .get(i)
                .context("Index out of bounds")?
                .send(BonkRoomMessage::Close)
                .await?;

            bonk_rooms.pop();
        }

        Ok(())
    }
}
