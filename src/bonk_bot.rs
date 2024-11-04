mod room_maker;

use room_maker::{RoomMaker, RoomMakerMessage};

use anyhow::Result;
use serenity::prelude::TypeMapKey;
use tokio::sync::{mpsc, oneshot};

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

pub struct BonkBotValue {
    client: Option<fantoccini::Client>,
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
            client: None,
            roommaker_tx,
        }
    }

    pub async fn open_room(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.roommaker_tx
            .send(RoomMakerMessage { client_sender: tx })
            .await?;
        self.client = Some(rx.await??);

        Ok(())
    }
}
