use tokio::sync::mpsc;

//buffer 10, blocking send
pub enum BonkRoomMessage {
    Close,
}

pub struct BonkRoom {
    rx: mpsc::Receiver<BonkRoomMessage>,
    client: fantoccini::Client,
}

impl BonkRoom {
    pub fn new(rx: mpsc::Receiver<BonkRoomMessage>, client: fantoccini::Client) -> BonkRoom {
        BonkRoom { rx, client }
    }

    pub async fn run(&mut self) {
        while let Some(message) = self.rx.recv().await {
            match message {
                BonkRoomMessage::Close => {
                    return;
                }
            }
        }
    }
}
