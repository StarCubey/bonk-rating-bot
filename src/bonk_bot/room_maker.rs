use tokio::sync::mpsc;

struct RoomMakerMessage {
    name: String,
}

struct RoomMaker {
    receiver: mpsc::Receiver<RoomMakerMessage>,
}

impl RoomMaker {
    fn new(receiver: mpsc::Receiver<RoomMakerMessage>) -> RoomMaker {
        RoomMaker { receiver }
    }

    async fn run(&mut self) {
        while let Some(message) = self.receiver.recv().await {
            //TODO make room.
        }
    }
}
