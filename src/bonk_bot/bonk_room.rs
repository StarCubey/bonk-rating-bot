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

use crate::leaderboard::LeaderboardMessage;

use super::bonk_commands;
use super::room_maker::{Mode, RoomParameters};

///buffer 10, blocking send
pub enum BonkRoomMessage {
    Close,
}

pub struct BonkRoom {
    pub rx: mpsc::Receiver<BonkRoomMessage>,
    pub client: fantoccini::Client,
    pub leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>>,
    pub room_parameters: RoomParameters,
    pub state: RoomState,
    pub state_changed: Instant,
    pub chat_queue: VecDeque<String>,
    pub player_data: PlayerData,
}

pub struct PlayerData {
    pub players: Vec<(usize, Player)>,
    pub queue: Vec<(String, Instant)>,
    pub team_flip: bool,
    pub captain: (usize, Player),
    pub other_player: (usize, Player),
    pub pick_progress: i32,
}

pub enum RoomState {
    Idle,
    BeforeGame,
    //MapSelection,
    //Ready,
    DuringGame,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Player {
    pub team: i32,
    pub ready: bool,
    #[serde(rename = "userName")]
    pub name: String,
}

impl Player {
    pub fn new() -> Player {
        Player {
            team: 0,
            ready: false,
            name: "".to_string(),
        }
    }
}

impl BonkRoom {
    pub fn new(
        rx: mpsc::Receiver<BonkRoomMessage>,
        client: fantoccini::Client,
        leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>>,
        room_parameters: RoomParameters,
    ) -> BonkRoom {
        BonkRoom {
            rx,
            client,
            leaderboard_tx,
            room_parameters,
            state: RoomState::Idle,
            state_changed: Instant::now(),
            chat_queue: VecDeque::new(),
            player_data: PlayerData {
                players: Vec::new(),
                queue: Vec::new(),
                team_flip: false,
                captain: (0, Player::new()),
                other_player: (0, Player::new()),
                pick_progress: 0,
            },
        }
    }

    pub async fn run(&mut self) {
        let mut chat_next_index = 0;
        let mut chat_checked = Instant::now();
        let mut message_sent = Instant::now();

        loop {
            self.update_state().await;

            loop {
                match self.rx.try_recv() {
                    Ok(BonkRoomMessage::Close) => return,
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => break,
                }
            }

            self.update_players_and_chat(
                &mut chat_next_index,
                &mut chat_checked,
                &mut message_sent,
            )
            .await;

            time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn update_state(&mut self) {
        let idle_time = Duration::from_secs(5);
        let pick_time = Duration::from_secs(60);
        let ready_time = Duration::from_secs(60);
        let match_length = Duration::from_secs(600);

        if let RoomState::Idle = self.state {
            if self.state_changed.elapsed() > idle_time {
                self.next_in_queue().await;
            }
        }

        if let RoomState::BeforeGame = self.state {
            let updated_captain = self
                .player_data
                .players
                .iter()
                .find(|p| p.0 == self.player_data.captain.0);
            if let Some(updated_captain) = updated_captain {
                self.player_data.captain.1.ready |= updated_captain.1.ready;
            }

            let updated_other_player = self
                .player_data
                .players
                .iter()
                .find(|p| p.0 == self.player_data.other_player.0);
            if let Some(updated_other_player) = updated_other_player {
                self.player_data.other_player.1.ready |= updated_other_player.1.ready;
            }

            if self.player_data.pick_progress == 0 && self.state_changed.elapsed() > pick_time {
                self.kick_player(&self.player_data.captain.clone()).await;
                self.next_in_queue().await;
            } else if self.player_data.pick_progress > 0 {
                if self.player_data.captain.1.ready && self.player_data.other_player.1.ready {
                    let start_button = self
                        .client
                        .find(Locator::Id("newbonklobby_startbutton"))
                        .await;
                    if let Ok(start_button) = start_button {
                        let _ = start_button.click().await;

                        let other_index = self
                            .player_data
                            .queue
                            .iter()
                            .position(|p| p.0 == self.player_data.other_player.1.name);
                        if let Some(other_index) = other_index {
                            let other_player = self.player_data.queue.remove(other_index);
                            self.player_data.queue.push(other_player);
                        }

                        let captain_index = self
                            .player_data
                            .queue
                            .iter()
                            .position(|p| p.0 == self.player_data.captain.1.name);
                        if let Some(captain_index) = captain_index {
                            let captain = self.player_data.queue.remove(captain_index);
                            self.player_data.queue.push(captain);
                        }

                        self.state = RoomState::DuringGame;
                        self.state_changed = Instant::now();
                    }
                }

                if self.state_changed.elapsed() > ready_time {
                    self.all_to_spec().await;

                    if !self.player_data.captain.1.ready {
                        self.kick_player(&self.player_data.captain.clone()).await;
                    }
                    if !self.player_data.other_player.1.ready {
                        self.kick_player(&self.player_data.other_player.clone())
                            .await;
                    }

                    self.next_in_queue().await;
                }
            }
        }

        if let RoomState::DuringGame = self.state {
            if self.state_changed.elapsed() > match_length {
                let _ = self
                    .client
                    .execute(
                        "document.getElementById('pretty_top_bar').style.top=0;",
                        vec![],
                    )
                    .await;
                let exit = self.client.find(Locator::Id("pretty_top_exit")).await;
                if let Ok(exit) = exit {
                    let _ = exit.click().await;
                }

                self.all_to_spec().await;

                self.chat_queue.push_back("Timeout!".to_string());
                self.state = RoomState::Idle;
                self.state_changed = Instant::now();
            }
        }
    }

    async fn next_in_queue(&mut self) {
        if self.player_data.players.len() < 3 {
            match self.state {
                RoomState::Idle => (),
                _ => {
                    self.state = RoomState::Idle;
                    self.state_changed = Instant::now();
                }
            }
        } else if self.player_data.players.len() < 4 {
            let my_name = dotenv::var("BONK_USERNAME").unwrap_or("".to_string());
            let filtered_players: Vec<&(usize, Player)> = self
                .player_data
                .players
                .iter()
                .filter(|p| p.1.name != my_name)
                .collect();

            let mut captain_team = 1;
            let mut other_player_team = 1;
            if let Mode::Football = self.room_parameters.mode {
                self.player_data.team_flip = rand::random();
                match self.player_data.team_flip {
                    false => {
                        captain_team = 3;
                        other_player_team = 2;
                    }
                    true => {
                        captain_team = 2;
                        other_player_team = 3;
                    }
                }
            }

            if let Some(&captain) = filtered_players.get(0) {
                let _ = self
                    .client
                    .execute(
                        "window\
                            .bonkHost\
                            .toolFunctions\
                            .networkEngine\
                            .changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(captain.0), json!(captain_team)],
                    )
                    .await;

                self.player_data.captain = captain.clone();
            }

            if let Some(&other_player) = filtered_players.get(1) {
                let _ = self
                    .client
                    .execute(
                        "window\
                            .bonkHost\
                            .toolFunctions\
                            .networkEngine\
                            .changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(other_player.0), json!(other_player_team)],
                    )
                    .await;

                self.player_data.other_player = other_player.clone();
            }
            self.chat_queue
                .push_back("Type \"!r\" to start the game.".to_string());

            self.player_data.pick_progress = 1;
            self.state = RoomState::BeforeGame;
            self.state_changed = Instant::now();
        } else {
            for next_player in &self.player_data.queue {
                let captain = self
                    .player_data
                    .players
                    .iter()
                    .find(|player| player.1.name == next_player.0);
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
                                .changeOtherTeam(arguments[0], arguments[1]);",
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
                    self.state_changed = Instant::now();

                    break;
                }
            }
        }
    }

    async fn kick_player(&mut self, player: &(usize, Player)) {
        let _ = self
            .client
            .execute(
                "window\
                    .bonkHost\
                    .toolFunctions\
                    .networkEngine\
                    .kickPlayer(arguments[0]);",
                vec![json!(player.0)],
            )
            .await;

        let index = self
            .player_data
            .queue
            .iter()
            .position(|p| p.0 == player.1.name);
        if let Some(index) = index {
            self.player_data.queue.remove(index);
        }
    }

    async fn all_to_spec(&mut self) {
        let _ = self
            .client
            .execute(
                "window\
                .bonkHost\
                .toolFunctions\
                .networkEngine\
                .changeOtherTeam(arguments[0], 0);\
            window\
                .bonkHost\
                .toolFunctions\
                .networkEngine\
                .changeOtherTeam(arguments[1], 0);",
                vec![
                    json!(self.player_data.captain.0),
                    json!(self.player_data.other_player.0),
                ],
            )
            .await;
    }

    async fn update_players_and_chat(
        &mut self,
        chat_next_index: &mut usize,
        chat_checked: &mut Instant,
        message_sent: &mut Instant,
    ) {
        let chat_wait_time = Duration::from_millis(200);
        let spot_hold_time = Duration::from_secs(60);
        let message_rate_limit = Duration::from_secs(3);

        if chat_checked.elapsed() >= chat_wait_time {
            let players = self
                .client
                .execute("return window.bonkHost.players;", vec![])
                .await;
            if let Ok(players) = players {
                if let Ok(players) = from_value::<Vec<Option<Player>>>(players) {
                    self.player_data.players = players
                        .into_iter()
                        .enumerate()
                        .filter_map(|player| {
                            if let Some(_player) = player.1 {
                                return Some((player.0, _player));
                            } else {
                                return None;
                            }
                        })
                        .collect::<Vec<(usize, Player)>>();

                    for player in self.player_data.players.iter() {
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

            if let RoomState::DuringGame = self.state {
                if self.state_changed.elapsed() > Duration::from_secs(10) {
                    let finished = self
                        .client
                        .execute(
                            "return document.getElementById('newbonklobby').style.opacity === '1';",
                            vec![],
                        )
                        .await;

                    if let Ok(finished) = finished {
                        if let Ok(true) = from_value::<bool>(finished) {
                            self.all_to_spec().await;
                            self.chat_queue.push_back("gg".to_string());

                            self.state = RoomState::Idle;
                            self.state_changed = Instant::now();
                        }
                    }
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
                "pick" | "p" => bonk_commands::pick(&command, &name, self).await,
                "ready" | "r" => bonk_commands::ready(&name, self).await,
                input => self.chat_queue.push_back(format!(
                    "Unknown command \"{}\". Run !help for a list of commands.",
                    input
                )),
            }
        }
    }
}
