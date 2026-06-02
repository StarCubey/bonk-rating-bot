//pub mod bonk_commands;
pub mod bonk_commands;
pub mod bonk_room;
pub mod events;
pub mod room_manager;

use std::sync::Arc;

use serenity::prelude::{TypeMap, TypeMapKey};
use tokio::sync::{mpsc, RwLock};

use self::room_manager::{RoomManager, RoomManagerMessage};

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

#[derive(Clone)]
pub struct BonkBotValue {
    pub roommanager_tx: mpsc::Sender<RoomManagerMessage>,
}

impl BonkBotValue {
    ///Panics
    pub async fn new(data: Arc<RwLock<TypeMap>>) -> BonkBotValue {
        let (roommanager_tx, roommanager_receiver) = mpsc::channel(3);
        let mut roommanager = RoomManager::new(roommanager_receiver, data)
            .await
            .expect("Failed to initialize room maker.");
        tokio::spawn(async move {
            roommanager.run().await;
        });

        BonkBotValue { roommanager_tx }
    }
}
