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

use super::bonk_commands;
use super::room_maker::{Mode, RoomParameters};

///buffer 10, blocking send
pub enum BonkRoomMessage {
    Close,
}

pub struct BonkRoom {
    pub rx: mpsc::Receiver<BonkRoomMessage>,
    pub client: fantoccini::Client,
    pub room_parameters: RoomParameters,
    pub state: RoomState,
    pub chat_queue: VecDeque<String>,
    pub player_data: PlayerData,
}

pub struct PlayerData {
    pub queue: Vec<(String, Instant)>,
    pub team_flip: bool,
    pub captain: (usize, Player),
    pub other_player: (usize, Player),
    pub pick_progress: i32,
}

pub enum RoomState {
    Idle,
    BeforeGame,
    DuringGame,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Player {
    pub team: i32,
    #[serde(rename = "userName")]
    pub name: String,
}

impl Player {
    fn new() -> Player {
        Player {
            team: 0,
            name: "".to_string(),
        }
    }
}

impl BonkRoom {
    pub fn new(
        rx: mpsc::Receiver<BonkRoomMessage>,
        client: fantoccini::Client,
        room_parameters: RoomParameters,
    ) -> BonkRoom {
        BonkRoom {
            rx,
            client,
            room_parameters,
            state: RoomState::Idle,
            chat_queue: VecDeque::new(),
            player_data: PlayerData {
                queue: Vec::new(),
                team_flip: false,
                captain: (0, Player::new()),
                other_player: (0, Player::new()),
                pick_progress: 0,
            },
        }
    }

    pub async fn run(&mut self) {
        let mut state_changed = Instant::now();

        let mut chat_next_index = 0;
        let mut players: Vec<(usize, Player)> = Vec::new();
        let mut chat_checked = Instant::now();
        let mut message_sent = Instant::now();

        loop {
            if let RoomState::Idle = self.state {
                if state_changed.elapsed() > Duration::from_secs(10) {
                    if let Some(next_player) = self.player_data.queue.get(0) {
                        let captain = players.iter().find(|player| player.1.name == next_player.0);
                        if let Some(captain) = captain {
                            let mut team = 1;
                            if let Mode::Football = self.room_parameters.mode {
                                self.player_data.team_flip = rand::random();
                                match self.player_data.team_flip {
                                    false => team = 3,
                                    true => team = 2,
                                }
                            }

                            let _ = self
                                .client
                                .execute(
                                    "window\
                                    .bonkHost\
                                    .toolFunctions\
                                    .networkEngine\
                                    .chagneOtherTeam(arguments[0], arguments[1]);",
                                    vec![json!(captain.0), json!(team)],
                                )
                                .await;
                            //Push front because high priority.
                            self.chat_queue.push_front(format!(
                                "{}, pick an opponent with !p <name>.",
                                captain.1.name
                            ));
                            self.player_data.captain = captain.clone();

                            self.state = RoomState::BeforeGame;
                            self.player_data.pick_progress = 0;
                            state_changed = Instant::now();
                        }
                    }
                }
            }

            loop {
                match self.rx.try_recv() {
                    Ok(BonkRoomMessage::Close) => return,
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => break,
                }
            }

            self.update_players_and_chat(
                &mut chat_next_index,
                &mut players,
                &mut chat_checked,
                &mut message_sent,
            )
            .await;

            time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn update_players_and_chat(
        &mut self,
        chat_next_index: &mut usize,
        players: &mut Vec<(usize, Player)>,
        chat_checked: &mut Instant,
        message_sent: &mut Instant,
    ) {
        let chat_wait_time = Duration::from_millis(200);
        let spot_hold_time = Duration::from_secs(60);
        let message_rate_limit = Duration::from_secs(3);

        if chat_checked.elapsed() >= chat_wait_time {
            let _players = self
                .client
                .execute("return window.bonkHost.players;", vec![])
                .await;
            if let Ok(_players) = _players {
                if let Ok(_players) = from_value::<Vec<Option<Player>>>(_players) {
                    *players = _players
                        .into_iter()
                        .enumerate()
                        .filter_map(|player| {
                            if let Some(_player) = player.1 {
                                return Some((player.0, _player));
                            } else {
                                return None;
                            }
                        })
                        .collect();

                    for player in players.iter() {
                        if let Ok(my_name) = dotenv::var("BONK_USERNAME") {
                            if player.1.name == my_name {
                                continue;
                            }
                        }

                        let queue_spot = self
                            .player_data
                            .queue
                            .iter()
                            .position(|entry| entry.0 == player.1.name);
                        match queue_spot {
                            Some(i) => {
                                let default = &mut ("".to_string(), Instant::now());
                                let value = self.player_data.queue.get_mut(i).unwrap_or(default);
                                value.1 = Instant::now();
                            }
                            None => {
                                self.player_data
                                    .queue
                                    .push((player.1.name.clone(), Instant::now()));
                            }
                        }
                    }
                }
            }
            self.player_data.queue = self
                .player_data
                .queue
                .drain(..)
                .filter(|entry| entry.1.elapsed() <= spot_hold_time)
                .collect();

            let chat = self
                .client
                .find_all(Locator::Css("#newbonklobby_chat_content > *"))
                .await;
            if let Ok(chat) = chat {
                while chat.len() > *chat_next_index {
                    let message = chat.get(*chat_next_index);
                    if let Some(message) = message {
                        let mut name = "".to_string();
                        if let Ok(_name) = message
                            .find(Locator::Css(".newbonklobby_chat_msg_name"))
                            .await
                        {
                            if let Ok(_name) = _name.html(true).await {
                                name = _name.strip_suffix(": ").unwrap_or(&_name).to_string();
                            }
                        }

                        if let Ok(content) = message
                            .find(Locator::Css(".newbonklobby_chat_msg_txt"))
                            .await
                        {
                            if let Ok(content) = content.html(true).await {
                                self.parse_command(content, name).await;
                            }
                        }
                    }

                    *chat_next_index += 1;
                }
            }

            *chat_checked = Instant::now();
        }

        if message_sent.elapsed() >= message_rate_limit && self.chat_queue.len() > 0 {
            if let Some(message) = self.chat_queue.pop_front() {
                let _ = self
                    .client
                    .execute(
                        "window.bonkHost.toolFunctions.networkEngine.chatMessage(arguments[0]);",
                        vec![json!(message)],
                    )
                    .await;

                *message_sent = Instant::now();
            }
        }
    }

    async fn parse_command(&mut self, command: String, name: String) {
        if let Ok(my_name) = dotenv::var("BONK_USERNAME") {
            if name == my_name {
                return;
            }
        }

        if let Some(command) = command.strip_prefix("!") {
            let mut command: Vec<&str> = command.split(' ').collect();

            match command.remove(0) {
                "help" => self.chat_queue.push_back(
                    "Use !queue to check the queue. Use !ping to ping me. That's all :3"
                        .to_string(),
                ),
                "ping" => self.chat_queue.push_back("Pong!".to_string()),
                "queue" | "q" => self.chat_queue.push_back(
                    self.player_data
                        .queue
                        .iter()
                        .map(|entry| entry.0.clone())
                        .collect::<Vec<String>>()
                        .join(", "),
                ),
                "pick" | "p" => bonk_commands::pick(&command, &name, self),
                input => self.chat_queue.push_back(format!(
                    "Unknown command \"{}\". Run !help for a list of commands.",
                    input
                )),
            }
        }
    }
}
