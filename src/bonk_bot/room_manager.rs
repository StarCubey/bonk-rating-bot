use anyhow::Context;
use anyhow::{anyhow, Result};
use fantoccini::ClientBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serenity::prelude::TypeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::sync::mpsc::WeakSender;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Instant};

use crate::bonk_bot::bonk_room::{BonkRoom, BonkRoomMessage};
use crate::leaderboard::{Leaderboard, LeaderboardMessage, LeaderboardSettings};

const ROOM_RATE_LIMIT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct RoomData {
    link: String,
    parameters: ParamType,
    tx: mpsc::Sender<BonkRoomMessage>,
}

pub enum RoomManagerMessage {
    MakeRoom {
        bonkroom_tx: oneshot::Sender<Result<CreationReply>>,
        room_parameters: ParamType,
    },
    UpdateRoomLink {
        old: String,
        new: String,
    },
    CloseAll {
        result_tx: oneshot::Sender<Result<()>>,
    },
    ForceCloseAll {
        result_tx: oneshot::Sender<Result<()>>,
    },
}

pub struct CreationReply {
    pub name: String,
    pub room_link: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ParamType {
    #[serde(rename = "normal")]
    Normal(RoomParameters),
    #[serde(rename = "dayroom")]
    DayRoom(DayRoomParameters),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DayRoomParameters {
    dayroom: Vec<RoomParameters>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RoomParameters {
    pub name: String,
    pub max_players: i32,
    pub min_level: i32,
    pub mode: Mode,
    pub queue: Queue,
    pub rounds: i32,
    pub maps: Vec<String>,

    #[serde(default = "strike_num_default")]
    pub strike_num: u32,
    #[serde(default = "team_size_default")]
    pub team_size: usize,
    #[serde(default = "team_num_default")]
    pub team_num: usize,
    #[serde(default = "ffa_min_default")]
    pub ffa_min: usize,
    #[serde(default = "ffa_max_default")]
    pub ffa_max: usize,
    #[serde(default = "idle_time_default")]
    pub idle_time: u64,
    #[serde(default = "pick_time_default")]
    pub pick_time: u64,
    #[serde(default = "ready_time_default")]
    pub ready_time: u64,
    #[serde(default = "strike_time_default")]
    pub strike_time: u64,
    #[serde(default = "game_time_default")]
    pub game_time: u64,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_headless")]
    pub headless: bool,
    #[serde(default = "default_unlisted")]
    pub unlisted: bool,
    pub leaderboard: Option<String>,
}

fn strike_num_default() -> u32 {
    2
}
fn team_size_default() -> usize {
    2
}
fn team_num_default() -> usize {
    2
}
fn ffa_min_default() -> usize {
    2
}
fn ffa_max_default() -> usize {
    7
}
fn idle_time_default() -> u64 {
    1
}
fn pick_time_default() -> u64 {
    60
}
fn ready_time_default() -> u64 {
    60
}
fn strike_time_default() -> u64 {
    20
}
fn game_time_default() -> u64 {
    600
}
fn default_headless() -> bool {
    true
}
fn default_unlisted() -> bool {
    true
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum Mode {
    Football,
    Simple,
    DeathArrows,
    Arrows,
    Grapple,
    VTOL,
    Classic,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Queue {
    Singles,
    Teams,
    FFA,
}

///Buffer 3, blocking send
pub struct RoomManager {
    rx: mpsc::Receiver<RoomManagerMessage>,
    data: Arc<RwLock<TypeMap>>,
    last_room_time: Option<Instant>,
    mods: String,
    rooms: Vec<RoomData>,
    leaderboards_tx: Vec<(i64, WeakSender<LeaderboardMessage>)>,
}

impl RoomManager {
    pub async fn new(
        rx: mpsc::Receiver<RoomManagerMessage>,
        data: Arc<RwLock<TypeMap>>,
    ) -> Result<RoomManager> {
        let mut sgr_api_file = File::open("dependencies/sgrAPI.user.js").await?;
        let mut sgr_api = String::new();
        sgr_api_file.read_to_string(&mut sgr_api).await?;

        let mut injector_file = File::open("dependencies/sgrInjector.user.js").await?;
        let mut injector = String::new();
        injector_file.read_to_string(&mut injector).await?;

        Ok(RoomManager {
            rx,
            data,
            last_room_time: None,
            mods: format!("{}{}", injector, sgr_api),
            rooms: vec![],
            leaderboards_tx: vec![],
        })
    }

    pub async fn run(&mut self) {
        while let Some(message) = self.rx.recv().await {
            match message {
                RoomManagerMessage::MakeRoom {
                    bonkroom_tx,
                    room_parameters,
                } => {
                    self.make_room(bonkroom_tx, room_parameters).await;
                }
                RoomManagerMessage::UpdateRoomLink { old, new } => {
                    match self.rooms.iter_mut().find(|room| room.link == old) {
                        Some(room) => {
                            room.link = new;
                        }
                        None => {
                            println!("Failed to update room link. Couldn't find {}", old);
                        }
                    }
                }
                RoomManagerMessage::CloseAll { result_tx } => {
                    let _ = result_tx.send(self.close_all().await);
                }
                RoomManagerMessage::ForceCloseAll { result_tx } => {
                    let _ = result_tx.send(self.force_close_all().await);
                }
            }
        }
    }

    async fn make_room(
        &mut self,
        bonkroom_tx: oneshot::Sender<std::result::Result<CreationReply, anyhow::Error>>,
        room_parameters: ParamType,
    ) {
        if let Some(last_room_time) = self.last_room_time {
            if let Some(wait_time) = ROOM_RATE_LIMIT.checked_sub(last_room_time.elapsed()) {
                sleep(wait_time).await;
            }
        }

        //TODO this should be the 5min check for remaking a room instead of ignoring closed rooms that shouldn't be closed.
        //There should also be a flag that says if the bot is currently waiting for rooms to closed so it doesn't remake rooms that should be closed.
        for i in (0..self.rooms.len()).rev() {
            let current = self.rooms.get(i);
            if let Some(room) = current {
                if room.tx.is_closed() {
                    self.rooms.remove(i);
                }
            }
        }

        let mut room_parameters = room_parameters;
        let params;
        let mut placeholder;
        match room_parameters {
            ParamType::Normal(ref mut room_parameters) => params = room_parameters,
            ParamType::DayRoom(ref day_room_parameters) => {
                let now = SystemTime::now();
                let Ok(days) = now.duration_since(UNIX_EPOCH) else {
                    println!("Failed to open dayroom because system time is set to before the Unix Epoch.");
                    return;
                };
                let days = days.as_secs() / (60 * 60 * 24);

                let rooms = day_room_parameters.dayroom.clone();
                let Some(output) = rooms.get(days as usize % rooms.len()) else {
                    return;
                };

                placeholder = output.clone();
                params = &mut placeholder;
            }
        }

        if params.min_level < 1 {
            let _ = bonkroom_tx.send(Err(anyhow!("min_level below 1 isn't supported.")));
            return;
        }

        let mut i = 0;
        loop {
            let err;

            match make_client(params.headless).await {
                Ok(c) => {
                    match init_room(&c, params, &self.mods).await {
                        Ok(ref room_link) => {
                            let leaderboard_tx;
                            match &params.leaderboard {
                                Some(lb) => match self.make_leaderboard(lb.clone()).await {
                                    Ok(lb) => {
                                        leaderboard_tx = Some(lb);
                                    }
                                    Err(e) => {
                                        leaderboard_tx = None;
                                        println!("Error while making leaderboard: {}", e);
                                    }
                                },
                                None => leaderboard_tx = None,
                            };

                            let (tx, rx) = mpsc::channel(10);
                            let mut bonkroom = BonkRoom::new(
                                room_link.clone(),
                                self.data.clone(),
                                rx,
                                c,
                                leaderboard_tx,
                                params.clone(),
                            )
                            .await;
                            tokio::spawn(async move {
                                bonkroom.run().await;
                            });

                            let _ = bonkroom_tx.send(Ok(CreationReply {
                                name: params.name.clone(),
                                room_link: room_link.clone(),
                            }));

                            self.rooms.push(RoomData {
                                link: room_link.clone(),
                                parameters: room_parameters,
                                tx,
                            });

                            break;
                        }
                        Err(e) => {
                            let _ = c.close().await;
                            err = e;
                        }
                    };
                }
                Err(e) => {
                    err = e;
                }
            };
            println!("Failed to make room: {}", err);
            if i >= 9 {
                let _ = bonkroom_tx.send(Err(err));
                break;
            }

            i += 1;
        }
        self.last_room_time = Some(Instant::now());
    }

    async fn make_leaderboard(&mut self, lb: String) -> Result<mpsc::Sender<LeaderboardMessage>> {
        let mut leaderboard_tx = None;

        let data = self.data.read().await;
        let db = data
            .get::<crate::ConnectionsKey>()
            .cloned()
            .ok_or(anyhow!("Failed to connect to database."))?
            .db;

        let rows: Vec<(i64, serde_json::Value)> =
            sqlx::query_as("SELECT id, settings FROM leaderboard WHERE abbreviation = $1")
                .bind(lb)
                .fetch_all(db.as_ref())
                .await?;

        if rows.len() < 1 {
            return Err(anyhow!("Leaderboard not found."));
        }

        let id = rows.get(0).context("Error while loading leaderboard.")?.0;
        let settings: LeaderboardSettings = serde_json::from_value(
            rows.get(0)
                .context("Error while loading leaderboard.")?
                .1
                .clone(),
        )?;

        self.leaderboards_tx.retain(|x| x.1.strong_count() > 0);
        let leaderboard_wtx = self.leaderboards_tx.iter().find(|x| x.0 == id);

        if let Some((_, leaderboard_wtx)) = leaderboard_wtx {
            leaderboard_tx = leaderboard_wtx.clone().upgrade();
        }

        if let None = leaderboard_tx {
            let (tx, rx) = mpsc::channel(10);
            let mut leaderboard = Leaderboard::new(rx, self.data.clone(), settings).await?;

            tokio::spawn(async move { leaderboard.run().await });

            self.leaderboards_tx.push((id, tx.clone().downgrade()));
            leaderboard_tx = Some(tx);
        }

        if let Some(output) = leaderboard_tx {
            return Ok(output);
        } else {
            return Err(anyhow!("Failed to get leaderboard transmitter."));
        }
    }

    async fn close_all(&mut self) -> Result<()> {
        for i in 0..self.rooms.len() {
            //Ignores failed send() because this means the room is already closed.
            let _ = self
                .rooms
                .get(i)
                .context("Index out of bounds.")?
                .tx
                .send(BonkRoomMessage::Close)
                .await;
        }

        let bonk_rooms_clone = self
            .rooms
            .iter()
            .map(|r| r.tx.clone())
            .collect::<Vec<mpsc::Sender<BonkRoomMessage>>>();
        let all_closed = tokio::spawn(async {
            for room in bonk_rooms_clone {
                room.closed().await;
            }
        });

        let sleep = Box::pin(sleep(Duration::from_secs(600)));
        let result;
        select! {
            _ = sleep => result = Err(anyhow!("Rooms force closed due to 10 minute timeout.")),
            _ = all_closed => result = Ok(()),
        };

        for room in self.rooms.iter() {
            let _ = room.tx.send(BonkRoomMessage::ForceClose).await;
        }

        self.rooms = vec![];

        result
    }

    async fn force_close_all(&mut self) -> Result<()> {
        for i in 0..self.rooms.len() {
            let _ = self
                .rooms
                .get(i)
                .context("Index out of bounds.")?
                .tx
                .send(BonkRoomMessage::ForceClose)
                .await;
        }

        self.rooms = vec![];

        Ok(())
    }
}

async fn make_client(headless: bool) -> Result<fantoccini::Client> {
    let port = dotenv::var("CHROMEDRIVER_PORT")?;

    let capabilities_headless = json!({
        "moz:firefoxOptions": {
            "args": ["--headless", "--mute-audio", "--width=1280", "--height=720"]
        },
        "goog:chromeOptions": {
            "binary": dotenv::var("CHROME_PATH")?,
            "args": ["--window-size=1920,1080", "--headless", "--mute-audio"],
        },
        "pageLoadStrategy": "none",
    });

    let capabilities_headful = json!({
        "moz:firefoxOptions": {
            "args": ["--mute-audio", "--width=1920", "--height=1080"]
        },
        "goog:chromeOptions": {
            "binary": dotenv::var("CHROME_PATH")?,
            "args": ["--window-size=1920,1080"],
        },
        "pageLoadStrategy": "none",
    });

    let capabilities = match headless {
        true => capabilities_headless,
        false => capabilities_headful,
    };

    let capabilities = match capabilities {
        Value::Object(map) => map,
        _ => return Err(anyhow!("Failed to generate capabilities value.")),
    };

    let c = ClientBuilder::native()
        .capabilities(capabilities.clone())
        .connect(&format!("http://localhost:{}", port).as_str())
        .await
        .context("Failed to connect to WebDriver.")?;

    Ok(c)
}

///Returns room link.
async fn init_room(
    c: &fantoccini::Client,
    room_parameters: &mut RoomParameters,
    mods: &String,
) -> Result<String> {
    //Force no guests for leaderboard rooms.
    if room_parameters.leaderboard.is_some() && room_parameters.min_level < 1 {
        room_parameters.min_level = 1;
    }

    let mut teams = false;
    if let Queue::Teams = &room_parameters.queue {
        teams = true;
    }
    if let Mode::Football = &room_parameters.mode {
        teams = false;
    }
    let mode = match &room_parameters.mode {
        Mode::Football => "f",
        Mode::Simple => "bs",
        Mode::DeathArrows => "ard",
        Mode::Arrows => "ar",
        Mode::Grapple => "sp",
        Mode::VTOL => "v",
        Mode::Classic => "b",
    };
    let credentials = vec![json!({
        "username": dotenv::var("BONK_USERNAME")?,
        "password": dotenv::var("BONK_PASSWORD")?,
    })];
    let room_data = vec![json!({
        "roomName": room_parameters.name,
        "roomPass": room_parameters.password.clone(),
        "maxPlayers": room_parameters.max_players,
        "minLevel": room_parameters.min_level,
        "unlisted": room_parameters.unlisted,
        "teams": teams,
        "mode": mode,
        "rounds": room_parameters.rounds,
    })];

    println!("Opening bonk.io...");

    c.goto("https://bonk.io/sgr").await?;

    println!("Loading mods...");

    c.execute(
        &format!(
            "{}{}{}{}",
            "window.done = new Promise(async resolve => {",
            mods,
            "await window.sgrAPIFunctionsLoaded;",
            "resolve();});",
        ),
        vec![],
    )
    .await?;
    let mut success = false;
    for _ in 0..5 {
        if let Ok(_) = c.execute("await window.done;", vec![]).await {
            success = true;
            break;
        }
    }
    if !success {
        return Err(anyhow!("Timeout on loading mods."));
    }

    println!("Logging in...");

    c.execute(
        &format!(
            "{}",
            "let credentials = arguments[0];\
            window.done = new Promise(async resolve => {;\
                await sgrAPI.logIn(credentials.username, credentials.password);\
            resolve();});"
        ),
        credentials,
    )
    .await?;
    let mut success = false;
    for _ in 0..5 {
        if let Ok(_) = c.execute("await window.done;", vec![]).await {
            success = true;
            break;
        }
    }
    if !success {
        return Err(anyhow!("Timeout on logging in."));
    }

    println!("Creating room...");

    c.execute(
        &format!(
            "{}",
            "let data = arguments[0];\
            window.done = new Promise(async resolve => {\
                let roomLink = await sgrAPI.makeRoom(\
                    data.roomName,\
                    data.roomPass,\
                    data.maxPlayers,\
                    data.minLevel,\
                    999,\
                    data.unlisted,\
                );\
                sgrAPI.setTeams(data.teams);
                sgrAPI.setMode(data.mode);
                sgrAPI.gameInfo[2].wl = data.rounds;
                sgrAPI.toolFunctions.networkEngine.changeOwnTeam(0);\
                sgrAPI.toolFunctions.networkEngine.sendNoHostSwap();\
                sgrAPI.toolFunctions.networkEngine.doTeamLock(true);\
                window.messageBuffer = [];\
                sgrAPI.onReceive = message => {\
                    window.messageBuffer.push(message);return true;\
                };\
                window.gameFrame = await window.gameFrame;\
                window.gdoc = gameFrame.contentDocument;\
            resolve(roomLink);});",
        ),
        room_data,
    )
    .await?;

    let mut room_link = "".to_string();
    let mut success = false;
    for _ in 0..5 {
        if let Ok(output) = c.execute("return await window.done;", vec![]).await {
            success = true;
            room_link = serde_json::from_value(output)?;
            break;
        }
    }
    if !success {
        return Err(anyhow!("Timeout on room creation."));
    }

    if let Some(map) = room_parameters.maps.get(0) {
        let _ = c
            .execute(
                "sgrAPI.loadMap(JSON.parse(arguments[0]));",
                vec![json!(map)],
            )
            .await;
    }

    println!("Room created: {}", room_link);

    Ok(room_link)
}
