use std::time::Duration;

use fantoccini::Locator;
use serde::Deserialize;
use serde::Serialize;
use serde_json::from_value;
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::{self, Instant},
};

///buffer 10, blocking send
pub enum BonkRoomMessage {
    Close,
}

pub struct BonkRoom {
    rx: mpsc::Receiver<BonkRoomMessage>,
    client: fantoccini::Client,
    state: RoomState,
    state_changed: Instant,
    chat_checked: Instant,
}

enum RoomState {
    Idle,
    BeforeGame,
    DuringGame,
}

#[derive(Deserialize, Serialize, Clone)]
struct Player {
    team: Option<i32>,
    #[serde(rename = "userName")]
    name: String,
}

impl BonkRoom {
    pub fn new(rx: mpsc::Receiver<BonkRoomMessage>, client: fantoccini::Client) -> BonkRoom {
        BonkRoom {
            rx,
            client,
            state: RoomState::Idle,
            state_changed: Instant::now(),
            chat_checked: Instant::now(),
        }
    }

    pub async fn run(&mut self) {
        let chat_wait_time = Duration::from_millis(250);
        let mut chat_next_index = 0;

        loop {
            if let RoomState::Idle = self.state {
                if self.state_changed.elapsed() > Duration::from_secs(10) {
                    //TODO select the next player in queue and ask them to pick an opponent.
                    self.state = RoomState::BeforeGame;
                    self.state_changed = Instant::now();
                }
            }

            loop {
                match self.rx.try_recv() {
                    Ok(BonkRoomMessage::Close) => return,
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => break,
                }
            }

            if self.chat_checked.elapsed() >= chat_wait_time {
                let players = self
                    .client
                    .execute("return window.bonkHost.players;", vec![])
                    .await;
                if let Ok(players) = players {
                    if let Ok(players) = from_value::<Vec<Option<Player>>>(players) {
                        for player in players.iter() {
                            if let Some(player) = player {
                                println!("{}", player.name); //TODO debug
                            }
                        }
                    }
                }

                let chat = self
                    .client
                    .find_all(Locator::Css("#newbonklobby_chat_content > *"))
                    .await;
                if let Ok(chat) = chat {
                    while chat.len() > chat_next_index {
                        let message = chat.get(chat_next_index);
                        if let Some(message) = message {
                            if let Ok(content) = message
                                .find(Locator::Css(".newbonklobby_chat_msg_txt"))
                                .await
                            {
                                if let Ok(content) = content.html(true).await {
                                    println!("{}", content); //TODO debug
                                }
                            }
                        }

                        chat_next_index += 1;
                    }
                }

                self.chat_checked = Instant::now();
            }

            //TODO use a switch statement instead of waiting for some arbitrary amount of time.
            time::sleep(Duration::from_millis(250)).await;
        }
    }
}
