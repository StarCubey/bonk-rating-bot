use std::collections::VecDeque;
use std::time::Duration;

use fantoccini::Locator;
use serde::Deserialize;
use serde::Serialize;
use serde_json::from_value;
use serde_json::json;
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
    chat_queue: VecDeque<String>,
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
            chat_queue: VecDeque::new(),
        }
    }

    pub async fn run(&mut self) {
        let chat_wait_time = Duration::from_millis(200);
        let message_rate_limit = Duration::from_secs(3);

        let mut chat_next_index = 0;
        let mut state = RoomState::Idle;
        let mut state_changed = Instant::now();
        let mut chat_checked = Instant::now();
        let mut message_sent = Instant::now();

        loop {
            if let RoomState::Idle = state {
                if state_changed.elapsed() > Duration::from_secs(10) {
                    //TODO select the next player in queue and ask them to pick an opponent.
                    state = RoomState::BeforeGame;
                    state_changed = Instant::now();
                }
            }

            loop {
                match self.rx.try_recv() {
                    Ok(BonkRoomMessage::Close) => return,
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => break,
                }
            }

            if chat_checked.elapsed() >= chat_wait_time {
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
                                    self.parse_command(content).await;
                                }
                            }
                        }

                        chat_next_index += 1;
                    }
                }

                chat_checked = Instant::now();
            }

            if message_sent.elapsed() >= message_rate_limit && self.chat_queue.len() > 0 {
                if let Some(message) = self.chat_queue.pop_front() {
                    let _ = self.client.execute(
                        "window.bonkHost.toolFunctions.networkEngine.chatMessage(arguments[0]);",
                        vec![json!(message)],
                    ).await;

                    message_sent = Instant::now();
                }
            }

            time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn parse_command(&mut self, command: String) {
        println!("{}", command);
        if let Some(command) = command.strip_prefix("!") {
            let mut command: Vec<&str> = command.split(' ').collect();

            match command.remove(0) {
                "help" => {
                    self.chat_queue.push_back(
                        "Use !ping to ping me. There are no other commands :3".to_string(),
                    );
                }
                "ping" => {
                    self.chat_queue.push_back("Pong!".to_string());
                }
                input => {
                    self.chat_queue.push_back(format!(
                        "Unknown command \"{}\". Run !help for a list of commands.",
                        input
                    ));
                }
            }
        }
    }
}
