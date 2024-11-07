mod bonk_room;
mod room_maker;

use anyhow::Result;
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

    pub async fn open_room(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.roommaker_tx
            .send(RoomMakerMessage { bonkroom_tx: tx })
            .await?;

        let mut bonk_rooms = self.bonk_rooms.lock().await;
        bonk_rooms.push(rx.await??);

        Ok(())
    }
}
