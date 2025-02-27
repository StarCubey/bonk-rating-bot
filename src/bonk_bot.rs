pub mod bonk_commands;
pub mod bonk_room;
pub mod room_maker;

use anyhow::{Context, Result};
use serenity::prelude::TypeMapKey;
use tokio::sync::{mpsc, oneshot, Mutex};

use self::bonk_room::BonkRoomMessage;
use self::room_maker::{RoomMaker, RoomMakerMessage};

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

pub struct BonkBotValue {
    bonk_rooms: Mutex<Vec<mpsc::Sender<BonkRoomMessage>>>,
    roommaker_tx: mpsc::Sender<RoomMakerMessage>,
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
        }
    }

    pub async fn open_room(
        &mut self,
        room_parameters: room_maker::RoomParameters,
    ) -> Result<String> {
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
