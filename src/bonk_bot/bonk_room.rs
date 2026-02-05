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
    ForceClose,
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
    pub closing: bool,
    pub warning_step: u32,
    pub chat_queue: VecDeque<String>,
    pub chat_burst: i32,
    pub queue: Vec<(Instant, Player)>,
    pub game_players: GamePlayers,
    pub team_flip: bool,
    pub map_strikes: Vec<bool>,
    //(id, strikes)
    pub player_strikes: Vec<(i32, u32)>,
    pub vote_reset: Vec<i32>,
    pub vote_cancel: Vec<i32>,
}

#[derive(Clone, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
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
        let mut update_interval = time::interval(Duration::from_millis(250));
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
            closing: false,
            warning_step: 0,
            chat_queue: VecDeque::new(),
            chat_burst: 0,
            queue: Vec::new(),
            game_players,
            team_flip: false,
            player_strikes: vec![],
            map_strikes: vec![],
            vote_reset: vec![],
            vote_cancel: vec![],
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
                    Some(BonkRoomMessage::Close) => {
                        match &self.state {
                            State::Idle => {
                                let _ = self
                                    .client
                                    .execute(
                                        "sgrAPI.toolFunctions.networkEngine.chatMessage(\"Room closed.\");",
                                        vec![],
                                    )
                                    .await;
                                time::sleep(Duration::from_secs(1)).await;
                                break;
                            },
                            _ => {
                                self.chat("Room closing soon.".to_string()).await;
                                self.closing = true;
                            }
                        }
                    },
                    Some(BonkRoomMessage::ForceClose) => {
                        let _ = self
                            .client
                            .execute(
                                "sgrAPI.toolFunctions.networkEngine.chatMessage(\"Room closed.\");",
                                vec![],
                            )
                            .await;
                        time::sleep(Duration::from_secs(1)).await;
                        break;
                    },
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

    pub fn get_in_game(&self) -> Vec<Player> {
        let mut in_game = vec![];
        match &self.game_players {
            GamePlayers::Singles { picker, picked } => {
                let Some(picker) = picker else { return in_game };
                let Some(picker) = self.queue.iter().find(|p| p.1.id == picker.id) else {
                    return in_game;
                };
                in_game.push(picker.1.clone());
                let Some(picked) = picked else { return in_game };
                let Some(picked) = self.queue.iter().find(|p| p.1.id == picked.id) else {
                    return in_game;
                };
                in_game.push(picked.1.clone());
            }
            GamePlayers::Teams {
                teams,
                picker_idx: _,
            } => {
                for team in teams {
                    for player in team {
                        let Some(player) = self.queue.iter().find(|p| p.1.id == player.id) else {
                            return in_game;
                        };
                        in_game.push(player.1.clone());
                    }
                }
            }
            GamePlayers::FFA {
                in_game: ffa_in_game,
            } => {
                for player in ffa_in_game {
                    let Some(player) = self.queue.iter().find(|p| p.1.id == player.id) else {
                        return in_game;
                    };
                    in_game.push(player.1.clone());
                }
            }
        }

        in_game
    }

    pub async fn reset(&mut self) {
        self.vote_reset = vec![];
        self.vote_cancel = vec![];

        if self.closing {
            let _ = self
                .client
                .execute(
                    "sgrAPI.toolFunctions.networkEngine.chatMessage(\"Room closed.\");",
                    vec![],
                )
                .await;
            time::sleep(Duration::from_secs(1)).await;
            self.rx.close();
            self.transition_timer = Box::pin(time::sleep(Duration::MAX));
            self.state = State::Idle;
            return;
        }

        let in_lobby = self
            .client
            .execute(
                "return document.getElementById('newbonklobby').style.opacity === '1';",
                vec![],
            )
            .await;
        if let Ok(in_lobby) = in_lobby {
            if let Ok(in_lobby) = from_value::<bool>(in_lobby) {
                if !in_lobby {
                    let _ = self
                        .client
                        .execute(
                            "document.getElementById(\"pretty_top_exit\").click();",
                            vec![],
                        )
                        .await;
                } else {
                    let _ = self
                        .client
                        .execute("sgrAPI.send(\"42[14]\");", vec![])
                        .await;
                }
            }
        }

        for player in &mut self.queue {
            player.1.ready_cmd = false;
            if player.1.in_room && player.1.team != 0 {
                let _ = self.client.execute(
                    "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                    vec![json!(player.1.id), json!(0)]
                ).await;
            }
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
        self.warning_step = 0;
        self.state = State::Idle;
    }

    pub async fn start_map_selection(&mut self) {
        if let Mode::Football = self.room_parameters.mode {
            self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                self.room_parameters.ready_time,
            )));
            self.warning_step = 0;
            self.state = State::Ready;
            self.chat("Use !r to start.".to_string()).await;
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
            self.warning_step = 0;
            self.state = State::Ready;
            self.chat("Use !r to start.".to_string()).await;
        } else {
            self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                self.room_parameters.strike_time,
            )));
            self.warning_step = 0;
            self.state = State::MapSelection;

            if let State::MapSelection = self.state {
                self.chat("Use !s to roll another map or use !r to start.".to_string())
                    .await;
            }
        }

        self.check_ready(false).await;
    }

    pub async fn check_ready(&mut self, chat: bool) {
        let in_game = self.get_in_game();
        let ready = in_game.iter().filter(|p| p.ready || p.ready_cmd).count();

        if ready >= in_game.len() {
            let _ = self.client.execute("sgrAPI.startGame();", vec![]).await;
            self.transition_timer = Box::pin(time::sleep(Duration::MAX));
            self.warning_step = 0;
            self.state = State::GameStarting;
        } else {
            if chat && self.chat_queue.len() == 0 {
                self.chat(format!("{}/{} players ready.", ready, in_game.len()))
                    .await;
            }
        }
    }

    async fn update(&mut self) {
        //TODO check if room closed.

        let remaining_time = self.transition_timer.deadline() - Instant::now();
        match self.state {
            State::Idle => (),
            State::Pick => {
                let warn = self.room_parameters.pick_time / 2;
                if self.warning_step < 1 && remaining_time < Duration::from_secs(warn) {
                    self.warning_step = 1;
                    self.chat(format!("{} left to pick.", sec_to_string(warn)))
                        .await;
                }
            }
            State::MapSelection => (),
            State::Ready => {
                let warn = self.room_parameters.ready_time / 2;
                if self.warning_step < 1 && remaining_time < Duration::from_secs(warn) {
                    self.warning_step = 1;
                    self.chat(format!(
                        "Use !r to start. {} until the match is cancelled.",
                        sec_to_string(warn)
                    ))
                    .await;
                }
            }
            State::GameStarting => (),
            State::InGame => {
                let warn1 = self.room_parameters.game_time / 2;
                let warn2 = 60;
                let warn3 = 30;
                let warn4 = 10;

                if self.warning_step < 4
                    && remaining_time < Duration::from_secs(warn4)
                    && warn1 / 2 >= warn4
                {
                    self.warning_step = 4;
                    self.chat(format!("{} left.", sec_to_string(warn4))).await;
                } else if self.warning_step < 3
                    && remaining_time < Duration::from_secs(warn3)
                    && warn1 / 2 >= warn3
                {
                    self.warning_step = 3;
                    self.chat(format!("{} left.", sec_to_string(warn3))).await;
                } else if self.warning_step < 2
                    && remaining_time < Duration::from_secs(warn2)
                    && warn1 / 2 >= warn2
                {
                    self.warning_step = 2;
                    self.chat(format!("{} until timeout.", sec_to_string(warn2)))
                        .await;
                } else if self.warning_step < 1 && remaining_time < Duration::from_secs(warn1) {
                    self.warning_step = 1;
                    self.chat(format!("{} until timeout.", sec_to_string(warn1)))
                        .await;
                }
            }
        }

        'in_lobby: {
            let (State::GameStarting | State::InGame) = self.state else {
                break 'in_lobby;
            };

            let output = self
                .client
                .execute(
                    "return document.getElementById('newbonklobby').style.opacity === '1';",
                    vec![json!(self.room_parameters.mode == Mode::Football)],
                )
                .await;
            let Ok(output) = output else {
                break 'in_lobby;
            };
            let Ok(output) = from_value::<bool>(output) else {
                break 'in_lobby;
            };

            if let State::GameStarting = self.state {
                if !output {
                    self.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                        self.room_parameters.game_time,
                    )));
                    self.warning_step = 0;
                    self.state = State::InGame
                }
            }
            if let State::InGame = self.state {
                if output {
                    events::on_game_end(self, None, false).await;
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
                            value.1.id = player.id;

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

    pub async fn chat(&mut self, message: String) {
        self.chat_queue.push_back(message);
        self.chat_update().await;
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
}

pub fn sec_to_string(time: u64) -> String {
    let mut output = vec![];
    let minutes = time / 60;
    if minutes > 0 {
        output.push(format!(
            "{} minute{}",
            minutes,
            if minutes == 1 { "" } else { "s" }
        ));
    }
    let seconds = time % 60;
    if minutes == 0 || seconds > 0 {
        output.push(format!(
            "{} second{}",
            seconds,
            if seconds == 1 { "" } else { "s" }
        ));
    }

    output.join(" and ")
}
