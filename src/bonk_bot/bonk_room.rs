use std::collections::VecDeque;
use std::pin::Pin;
use std::time::Duration;

use rand::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::from_value;
use serde_json::json;
use tokio::select;
use tokio::time::Interval;
use tokio::time::Sleep;
use tokio::{
    sync::mpsc,
    time::{self, Instant},
};

use crate::bonk_bot::events;
use crate::bonk_bot::room_maker;
use crate::bonk_bot::room_maker::Mode;
use crate::leaderboard::LeaderboardMessage;

//use super::bonk_commands;
use super::room_maker::RoomParameters;

///buffer 10, blocking send
pub enum BonkRoomMessage {
    Close,
}

pub struct BonkRoom {
    pub rx: mpsc::Receiver<BonkRoomMessage>,
    pub client: fantoccini::Client,
    pub leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>>,
    pub room_parameters: RoomParameters,
    pub update_interval: Interval,
    pub chat_interval: Interval,
    pub chat_clear_interval: Interval,
    pub state: State,
    pub transition_timer: Pin<Box<Sleep>>,
    pub chat_queue: VecDeque<String>,
    pub chat_burst: i32,
    pub queue: Vec<(Instant, Player)>,
    pub game_players: GamePlayers,
    pub team_flip: bool,
    pub map_strikes: Vec<bool>,
    //(id, strikes)
    pub player_strikes: Vec<(i32, u32)>,
}

#[derive(Clone)]
pub enum GamePlayers {
    Singles {
        picker: Option<Player>,
        picked: Option<Player>,
    },
    Teams {
        teams: Vec<Vec<Player>>,
        picker_idx: usize,
    },
    FFA {
        in_game: Vec<Player>,
    },
}

#[derive(PartialEq)]
pub enum State {
    Idle,
    Pick,
    MapSelection,
    Ready,
    GameStarting,
    InGame,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Player {
    pub id: i32,
    pub team: i32,
    pub ready: bool,
    #[serde(default)]
    pub ready_cmd: bool,
    #[serde(rename = "userName")]
    pub name: String,
    #[serde(default)]
    pub in_room: bool,
}

impl Player {
    pub fn new() -> Player {
        Player {
            id: 0,
            team: 0,
            ready: false,
            ready_cmd: false,
            name: "".to_string(),
            in_room: false,
        }
    }
}

impl BonkRoom {
    ///Creates BonkRoom instance and starts intervals and timers.
    pub fn new(
        rx: mpsc::Receiver<BonkRoomMessage>,
        client: fantoccini::Client,
        leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>>,
        room_parameters: RoomParameters,
    ) -> BonkRoom {
        let mut update_interval = time::interval(Duration::from_millis(100));
        update_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let mut chat_interval = time::interval(Duration::from_secs(4));
        chat_interval.set_missed_tick_behavior(time::MissedTickBehavior::Burst);

        let mut chat_clear_interval = time::interval(Duration::from_secs(60 * 10));
        chat_clear_interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let transition_timer =
            Box::pin(time::sleep(Duration::from_secs(room_parameters.idle_time)));

        let game_players = match room_parameters.queue {
            room_maker::Queue::Singles => GamePlayers::Singles {
                picker: None,
                picked: None,
            },
            room_maker::Queue::Teams => GamePlayers::Teams {
                teams: vec![],
                picker_idx: 0,
            },
            room_maker::Queue::FFA => GamePlayers::FFA { in_game: vec![] },
        };

        BonkRoom {
            rx,
            client,
            leaderboard_tx,
            room_parameters,
            update_interval,
            chat_interval,
            chat_clear_interval,
            state: State::Idle,
            transition_timer,
            chat_queue: VecDeque::new(),
            chat_burst: 0,
            queue: Vec::new(),
            game_players,
            team_flip: false,
            player_strikes: vec![],
            map_strikes: vec![],
        }
    }

    pub async fn run(&mut self) {
        loop {
            select! {
                _ = self.transition_timer.as_mut() => events::on_transition_timer_expired(self).await,
                _ = self.update_interval.tick() => self.update().await,
                _ = self.chat_interval.tick() => {
                        self.chat_burst = i32::min(6, self.chat_burst + 1);
                        self.chat_update().await
                    }
                _ = self.chat_clear_interval.tick() => {
                    let _ = self.client.execute(
                        "document.getElementById(\"newbonklobby_chat_content\").innerHTML = \"\";",
                        vec![],
                    ).await;
                }
                message = self.rx.recv() => match message {
                    Some(BonkRoomMessage::Close) => break,
                    None => break,
                }
            }
        }
        println!("Room closed.");
    }

    pub fn get_queue_cloned(&self) -> Vec<Player> {
        self.queue
            .iter()
            .filter(|p| p.1.in_room)
            .map(|p| p.1.clone())
            .collect()
    }

    pub fn get_queue_mut(&mut self) -> Vec<&mut Player> {
        self.queue
            .iter_mut()
            .filter(|p| p.1.in_room)
            .map(|p| &mut p.1)
            .collect()
    }

    pub async fn reset(&mut self) {
        //TODO close game if in game, otherwise send all to lobby websocket message.

        for player in &mut self.queue {
            player.1.ready_cmd = false;
        }
        let _ = self
            .client
            .execute(
                "sgrAPI.toolFunctions.networkEngine.allReadyReset();",
                vec![],
            )
            .await;

        for p in self.get_queue_cloned() {
            let _ = self
                .client
                .execute(
                    "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                    vec![json!(p.id), json!(0)],
                )
                .await;
        }

        self.game_players = match self.room_parameters.queue {
            room_maker::Queue::Singles => GamePlayers::Singles {
                picker: None,
                picked: None,
            },
            room_maker::Queue::Teams => GamePlayers::Teams {
                teams: vec![],
                picker_idx: 0,
            },
            room_maker::Queue::FFA => GamePlayers::FFA { in_game: vec![] },
        };

        self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
            self.room_parameters.idle_time,
        )));
        self.state = State::Idle;
    }

    pub async fn start_map_selection(&mut self) {
        if let Mode::Football = self.room_parameters.mode {
            self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                self.room_parameters.ready_time,
            )));
            self.state = State::Ready;
            return;
        }

        self.map_strikes = vec![false; self.room_parameters.maps.len()];
        self.player_strikes = vec![];
        let map_idx = rand::rng().random_range(0..self.room_parameters.maps.len());
        if let Some(map_strike) = self.map_strikes.get_mut(map_idx) {
            *map_strike = true;
        }
        let Some(map) = self.room_parameters.maps.get(map_idx) else {
            return;
        };
        let _ = self
            .client
            .execute(
                "sgrAPI.loadMap(JSON.parse(arguments[0]));",
                vec![json!(map)],
            )
            .await;

        for player in &mut self.queue {
            player.1.ready_cmd = false;
        }
        let _ = self
            .client
            .execute(
                "sgrAPI.toolFunctions.networkEngine.allReadyReset();",
                vec![],
            )
            .await;

        if self.room_parameters.strike_num <= 0 || self.room_parameters.maps.len() < 2 {
            self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                self.room_parameters.ready_time,
            )));
            self.state = State::Ready;
            self.chat("Use !r to start.".to_string()).await;
        } else {
            self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                self.room_parameters.strike_time,
            )));
            self.state = State::MapSelection;

            if let State::MapSelection = self.state {
                self.chat("Use !s to roll another map or use !r to start.".to_string())
                    .await;
            }
        }

        self.check_ready(false).await;
    }

    pub async fn check_ready(&mut self, chat: bool) {
        match &self.game_players {
            GamePlayers::Singles { picker, picked } => {
                let mut ready = 0;
                let Some(picker) = picker else { return };
                let Some(picked) = picked else { return };
                let Some(picker) = self.queue.iter().find(|p| p.1.id == picker.id) else {
                    return;
                };
                let Some(picked) = self.queue.iter().find(|p| p.1.id == picked.id) else {
                    return;
                };
                let picker = picker.1.clone();
                let picked = picked.1.clone();

                if picker.ready || picker.ready_cmd {
                    ready += 1;
                }
                if picked.ready || picked.ready_cmd {
                    ready += 1;
                }

                if ready >= 2 {
                    let _ = self.client.execute("sgrAPI.startGame();", vec![]).await;
                    self.transition_timer = Box::pin(time::sleep(Duration::MAX));
                    self.state = State::GameStarting;
                } else {
                    if chat && self.chat_queue.len() == 0 {
                        self.chat(format!("{}/2 players ready.", ready)).await;
                    }
                }
            }
            GamePlayers::Teams {
                teams,
                picker_idx: _,
            } => {
                let mut ready = 0;
                let mut total = 0;
                for team in teams {
                    for player in team {
                        total += 1;
                        let Some(player) = self.queue.iter().find(|p| p.1.id == player.id) else {
                            return;
                        };
                        let player = player.1.clone();
                        if player.ready || player.ready_cmd {
                            ready += 1;
                        }
                    }
                }

                if ready >= total {
                    let _ = self.client.execute("sgrAPI.startGame();", vec![]).await;
                    self.transition_timer = Box::pin(time::sleep(Duration::MAX));
                    self.state = State::GameStarting;
                } else {
                    if chat && self.chat_queue.len() == 0 {
                        self.chat(format!("{}/{} players ready.", ready, total))
                            .await;
                    }
                }
            }
            GamePlayers::FFA { in_game } => {
                let mut ready = 0;
                for player in in_game {
                    let Some(player) = self.queue.iter().find(|p| p.1.id == player.id) else {
                        return;
                    };
                    let player = player.1.clone();
                    if player.ready || player.ready_cmd {
                        ready += 1;
                    }
                }

                if ready >= in_game.len() {
                    let _ = self.client.execute("sgrAPI.startGame();", vec![]).await;
                    self.transition_timer = Box::pin(time::sleep(Duration::MAX));
                    self.state = State::GameStarting;
                } else {
                    if chat && self.chat_queue.len() == 0 {
                        self.chat(format!("{}/{} players ready.", ready, in_game.len()))
                            .await;
                    }
                }
            }
        }
    }

    pub async fn chat(&mut self, message: String) {
        self.chat_queue.push_back(message);
        self.chat_update().await;
    }

    async fn update(&mut self) {
        //TODO Check time left on transition timer, reminders, required actions.
        //TODO check if room closed.

        if let State::GameStarting = self.state {
            let output = self
                .client
                .execute(
                    "return document.getElementById('newbonklobby').style.opacity === '1';",
                    vec![],
                )
                .await;
            if let Ok(output) = output {
                if let Ok(output) = from_value::<bool>(output) {
                    if !output {
                        self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                            self.room_parameters.game_time,
                        )));
                        self.state = State::InGame
                    }
                }
            }
        }

        if let State::InGame = self.state {
            let output = self
                .client
                .execute(
                    "return document.getElementById('newbonklobby').style.opacity === '1';",
                    vec![],
                )
                .await;
            if let Ok(output) = output {
                if let Ok(output) = from_value::<bool>(output) {
                    if output {
                        events::on_game_end(self).await;
                    }
                }
            }
        }

        let spot_hold_time = Duration::from_secs(60);
        let my_name = dotenv::var("BONK_USERNAME").unwrap_or_default();

        let players = self
            .client
            .execute("return sgrAPI.getPlayers();", vec![])
            .await;
        if let Ok(players) = players {
            if let Ok(mut players) = from_value::<Vec<Player>>(players) {
                let mut joining_players = vec![];
                for player in &mut players {
                    if player.name == my_name {
                        continue;
                    }

                    let queue_spot = self.queue.iter().position(|p| p.1.name == player.name);
                    match queue_spot {
                        Some(i) => {
                            let default = &mut (Instant::now(), Player::new());
                            let value = self.queue.get_mut(i).unwrap_or(default);
                            value.0 = Instant::now();
                            value.1.team = player.team;

                            if !value.1.in_room {
                                joining_players.push(value.1.clone());
                            }
                            value.1.in_room = true;

                            let old_ready = value.1.ready;
                            value.1.ready = player.ready;
                            if !old_ready && player.ready {
                                if self.state == State::MapSelection || self.state == State::Ready {
                                    self.check_ready(true).await;
                                }
                            }
                        }
                        None => {
                            player.in_room = true;
                            self.queue.push((Instant::now(), player.clone()));
                            joining_players.push(player.clone());
                        }
                    }
                }
                for player in joining_players {
                    events::on_player_join(self, player).await;
                }
                self.queue = self
                    .queue
                    .drain(..)
                    .filter(|p| p.0.elapsed() <= spot_hold_time)
                    .collect();

                let mut leaving_players = vec![];
                for player in &mut self.queue {
                    if players.iter().find(|p| p.name == player.1.name).is_none() {
                        if player.1.in_room {
                            leaving_players.push(player.1.clone());
                        }
                        player.1.in_room = false;
                    }
                }
                for player in leaving_players {
                    events::on_player_leave(self, player).await;
                }
            }
        }

        let messages = self
            .client
            .execute(
                "\
                let messages = window.messageBuffer;\
                window.messageBuffer = [];\
                return messages;\
            ",
                vec![],
            )
            .await;
        if let Ok(messages) = messages {
            let messages: Result<Vec<String>, serde_json::Error> = serde_json::from_value(messages);
            if let Ok(messages) = messages {
                for message in messages {
                    events::on_message(self, message).await;
                }
            }
        }
    }

    async fn chat_update(&mut self) {
        while self.chat_burst > 0 {
            let message = self.chat_queue.pop_front();
            if let Some(message) = message {
                let _ = self
                    .client
                    .execute(
                        "sgrAPI.toolFunctions.networkEngine.chatMessage(arguments[0]);",
                        vec![json!(message)],
                    )
                    .await;

                self.chat_burst -= 1;
            } else {
                break;
            }
        }
    }

    // async fn next_in_queue(&mut self) {
    //     if self.player_data.players.len() < 3 {
    //         match self.state {
    //             RoomState::Idle => (),
    //             _ => {
    //                 self.state = RoomState::Idle;
    //                 self.state_changed = Instant::now();
    //             }
    //         }
    //     } else if self.player_data.players.len() < 4 {
    //         let my_name = dotenv::var("BONK_USERNAME").unwrap_or("".to_string());
    //         let filtered_players: Vec<&(usize, Player)> = self
    //             .player_data
    //             .players
    //             .iter()
    //             .filter(|p| p.1.name != my_name)
    //             .collect();

    //         let mut captain_team = 1;
    //         let mut other_player_team = 1;
    //         if let Mode::Football = self.room_parameters.mode {
    //             self.player_data.team_flip = rand::random();
    //             match self.player_data.team_flip {
    //                 false => {
    //                     captain_team = 3;
    //                     other_player_team = 2;
    //                 }
    //                 true => {
    //                     captain_team = 2;
    //                     other_player_team = 3;
    //                 }
    //             }
    //         }

    //         if let Some(&captain) = filtered_players.get(0) {
    //             let _ = self
    //                 .client
    //                 .execute(
    //                     "sgrAPI\
    //                         .toolFunctions\
    //                         .networkEngine\
    //                         .changeOtherTeam(arguments[0], arguments[1]);",
    //                     vec![json!(captain.0), json!(captain_team)],
    //                 )
    //                 .await;

    //             self.player_data.captain = captain.clone();
    //         }

    //         if let Some(&other_player) = filtered_players.get(1) {
    //             let _ = self
    //                 .client
    //                 .execute(
    //                     "sgrAPI\
    //                         .toolFunctions\
    //                         .networkEngine\
    //                         .changeOtherTeam(arguments[0], arguments[1]);",
    //                     vec![json!(other_player.0), json!(other_player_team)],
    //                 )
    //                 .await;

    //             self.player_data.other_player = other_player.clone();
    //         }
    //         self.chat_queue
    //             .push_back("Type \"!r\" to start the game.".to_string());

    //         self.player_data.pick_progress = 1;
    //         self.state = RoomState::BeforeGame;
    //         self.state_changed = Instant::now();
    //     } else {
    //         for next_player in &self.player_data.queue {
    //             let captain = self
    //                 .player_data
    //                 .players
    //                 .iter()
    //                 .find(|player| player.1.name == next_player.0);
    //             if let Some(captain) = captain {
    //                 let mut team = 1;
    //                 if let Mode::Football = self.room_parameters.mode {
    //                     self.player_data.team_flip = rand::random();
    //                     match self.player_data.team_flip {
    //                         false => team = 3,
    //                         true => team = 2,
    //                     }
    //                 }

    //                 let _ = self
    //                     .client
    //                     .execute(
    //                         "sgrAPI\
    //                             .toolFunctions\
    //                             .networkEngine\
    //                             .changeOtherTeam(arguments[0], arguments[1]);",
    //                         vec![json!(captain.0), json!(team)],
    //                     )
    //                     .await;
    //                 //Push front because high priority.
    //                 self.chat_queue.push_front(format!(
    //                     "{}, pick an opponent with !p <name>.",
    //                     captain.1.name
    //                 ));
    //                 self.player_data.captain = captain.clone();

    //                 self.state = RoomState::BeforeGame;
    //                 self.player_data.pick_progress = 0;
    //                 self.state_changed = Instant::now();

    //                 break;
    //             }
    //         }
    //     }
    // }

    // async fn kick_player(&mut self, player: &(usize, Player)) {
    //     let _ = self
    //         .client
    //         .execute(
    //             "sgrAPI\
    //                 .toolFunctions\
    //                 .networkEngine\
    //                 .kickPlayer(arguments[0]);",
    //             vec![json!(player.0)],
    //         )
    //         .await;

    //     let index = self
    //         .player_data
    //         .queue
    //         .iter()
    //         .position(|p| p.0 == player.1.name);
    //     if let Some(index) = index {
    //         self.player_data.queue.remove(index);
    //     }
    // }

    // async fn all_to_spec(&mut self) {
    //     let _ = self
    //         .client
    //         .execute(
    //             "sgrAPI\
    //             .toolFunctions\
    //             .networkEngine\
    //             .changeOtherTeam(arguments[0], 0);\
    //         sgrAPI\
    //             .toolFunctions\
    //             .networkEngine\
    //             .changeOtherTeam(arguments[1], 0);",
    //             vec![
    //                 json!(self.player_data.captain.0),
    //                 json!(self.player_data.other_player.0),
    //             ],
    //         )
    //         .await;
    // }

    //     async fn update_players_and_chat(
    //         &mut self,
    //         chat_next_index: &mut usize,
    //         chat_checked: &mut Instant,
    //         message_sent: &mut Instant,
    //     ) {
    //         let chat_wait_time = Duration::from_millis(200);
    //         let spot_hold_time = Duration::from_secs(60);
    //         let message_rate_limit = Duration::from_secs(3);

    //         if chat_checked.elapsed() >= chat_wait_time {
    //             let players = self.client.execute("return sgrAPI.players;", vec![]).await;
    //             if let Ok(players) = players {
    //                 if let Ok(players) = from_value::<Vec<Option<Player>>>(players) {
    //                     self.player_data.players = players
    //                         .into_iter()
    //                         .enumerate()
    //                         .filter_map(|player| {
    //                             if let Some(_player) = player.1 {
    //                                 return Some((player.0, _player));
    //                             } else {
    //                                 return None;
    //                             }
    //                         })
    //                         .collect::<Vec<(usize, Player)>>();

    //                     for player in self.player_data.players.iter() {
    //                         if let Ok(my_name) = dotenv::var("BONK_USERNAME") {
    //                             if player.1.name == my_name {
    //                                 continue;
    //                             }
    //                         }

    //                         let queue_spot = self
    //                             .player_data
    //                             .queue
    //                             .iter()
    //                             .position(|entry| entry.0 == player.1.name);
    //                         match queue_spot {
    //                             Some(i) => {
    //                                 let default = &mut ("".to_string(), Instant::now());
    //                                 let value = self.player_data.queue.get_mut(i).unwrap_or(default);
    //                                 value.1 = Instant::now();
    //                             }
    //                             None => {
    //                                 self.player_data
    //                                     .queue
    //                                     .push((player.1.name.clone(), Instant::now()));
    //                             }
    //                         }
    //                     }
    //                 }
    //             }
    //             self.player_data.queue = self
    //                 .player_data
    //                 .queue
    //                 .drain(..)
    //                 .filter(|entry| entry.1.elapsed() <= spot_hold_time)
    //                 .collect();

    //             let chat = self
    //                 .client
    //                 .find_all(Locator::Css("#newbonklobby_chat_content > *"))
    //                 .await;
    //             if let Ok(chat) = chat {
    //                 while chat.len() > *chat_next_index {
    //                     let message = chat.get(*chat_next_index);
    //                     if let Some(message) = message {
    //                         let mut name = "".to_string();
    //                         if let Ok(_name) = message
    //                             .find(Locator::Css(".newbonklobby_chat_msg_name"))
    //                             .await
    //                         {
    //                             if let Ok(_name) = _name.html(true).await {
    //                                 name = _name.strip_suffix(": ").unwrap_or(&_name).to_string();
    //                             }
    //                         }

    //                         if let Ok(content) = message
    //                             .find(Locator::Css(".newbonklobby_chat_msg_txt"))
    //                             .await
    //                         {
    //                             if let Ok(content) = content.html(true).await {
    //                                 self.parse_command(content, name).await;
    //                             }
    //                         }
    //                     }

    //                     *chat_next_index += 1;
    //                 }
    //             }

    //             if let RoomState::DuringGame = self.state {
    //                 if self.state_changed.elapsed() > Duration::from_secs(10) {
    //                     let finished = self
    //                         .client
    //                         .execute(
    //                             "return document.getElementById('newbonklobby').style.opacity === '1';",
    //                             vec![],
    //                         )
    //                         .await;

    //                     if let Ok(finished) = finished {
    //                         if let Ok(true) = from_value::<bool>(finished) {
    //                             self.all_to_spec().await;
    //                             if let Some(leaderboard_tx) = &self.leaderboard_tx {
    //                                 if let Mode::Football = self.room_parameters.mode {
    //                                     let blue_win = self.client.execute("return sgrAPI.footballState.scores[3] === arguments[0];", vec![json!(self.room_parameters.rounds)]).await;
    //                                     if let Ok(blue_win) = blue_win {
    //                                         if let Ok(blue_win) = from_value::<bool>(blue_win) {
    //                                             let (match_string_tx, match_string_rx) =
    //                                                 oneshot::channel();
    //                                             let captain = self.player_data.captain.1.name.clone();
    //                                             let other_player =
    //                                                 self.player_data.other_player.1.name.clone();

    //                                             _ = leaderboard_tx
    //                                                 .send(LeaderboardMessage::Update {
    //                                                     teams: vec![
    //                                                         vec![if blue_win
    //                                                             ^ self.player_data.team_flip
    //                                                         {
    //                                                             captain.clone()
    //                                                         } else {
    //                                                             other_player.clone()
    //                                                         }],
    //                                                         vec![if blue_win
    //                                                             ^ self.player_data.team_flip
    //                                                         {
    //                                                             other_player
    //                                                         } else {
    //                                                             captain
    //                                                         }],
    //                                                     ],
    //                                                     ties: vec![false],
    //                                                     match_str: match_string_tx,
    //                                                 })
    //                                                 .await;

    //                                             if let Ok(Ok(match_string)) = match_string_rx.await {
    //                                                 self.chat_queue.push_back(match_string);
    //                                             }
    //                                         }
    //                                     }
    //                                 }
    //                             }

    //                             self.state = RoomState::Idle;
    //                             self.state_changed = Instant::now();
    //                         }
    //                     }
    //                 }
    //             }

    //             *chat_checked = Instant::now();
    //         }

    //         if message_sent.elapsed() >= message_rate_limit && self.chat_queue.len() > 0 {
    //             if let Some(message) = self.chat_queue.pop_front() {
    //                 let _ = self
    //                     .client
    //                     .execute(
    //                         "sgrAPI.toolFunctions.networkEngine.chatMessage(arguments[0]);",
    //                         vec![json!(message)],
    //                     )
    //                     .await;

    //                 *message_sent = Instant::now();
    //             }
    //         }
    //     }

    //     async fn parse_command(&mut self, command: String, name: String) {
    //         if let Ok(my_name) = dotenv::var("BONK_USERNAME") {
    //             if name == my_name {
    //                 return;
    //             }
    //         }

    //         if let Some(command) = command.strip_prefix("!") {
    //             let mut command: Vec<&str> = command.split(' ').collect();

    //             match command.remove(0) {
    //                 "help" => self.chat_queue.push_back(
    //                     "Use !queue to check the queue. Use !ping to ping me. That's all :3"
    //                         .to_string(),
    //                 ),
    //                 "ping" => self.chat_queue.push_back("Pong!".to_string()),
    //                 "queue" | "q" => self.chat_queue.push_back(
    //                     self.player_data
    //                         .queue
    //                         .iter()
    //                         .map(|entry| entry.0.clone())
    //                         .collect::<Vec<String>>()
    //                         .join(", "),
    //                 ),
    //                 "pick" | "p" => bonk_commands::pick(&command, &name, self).await,
    //                 "ready" | "r" => bonk_commands::ready(&name, self).await,
    //                 input => self.chat_queue.push_back(format!(
    //                     "Unknown command \"{}\". Run !help for a list of commands.",
    //                     input
    //                 )),
    //             }
    //         }
    //     }
}
