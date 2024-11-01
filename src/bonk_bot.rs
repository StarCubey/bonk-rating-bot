mod room_maker;

use serenity::prelude::TypeMapKey;

pub struct BonkBotKey;

impl TypeMapKey for BonkBotKey {
    type Value = BonkBotValue;
}

pub struct BonkBotValue {
    client: Option<fantoccini::Client>,
}

impl BonkBotValue {
    pub fn new() -> BonkBotValue {
        BonkBotValue { client: None }
    }
}

pub fn open_room() {
    println!("Room opened!");
    //TODO
}
