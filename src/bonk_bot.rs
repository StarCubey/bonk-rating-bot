mod room_maker;

use tokio::sync::Mutex;

pub struct BonkBot {
    clients: Mutex<Vec<fantoccini::Client>>,
}

impl BonkBot {
    pub fn new() -> BonkBot {
        BonkBot {
            clients: Mutex::new(Vec::new()),
        }
    }

    pub fn open_room() {
        //TODO
    }
}
