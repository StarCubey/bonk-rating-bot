use anyhow::Context;
use anyhow::{anyhow, Result};
use fantoccini::ClientBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serenity::all::Http;
use serenity::prelude::TypeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Instant};

use crate::leaderboard::LeaderboardMessage;

use super::bonk_room::{BonkRoom, BonkRoomMessage};

pub struct RoomMakerMessage {
    pub http: Arc<Http>,
    pub data: Arc<RwLock<TypeMap>>,
    pub bonkroom_tx: oneshot::Sender<Result<CreationReply>>,
    pub leaderboard_tx: Option<mpsc::Sender<LeaderboardMessage>>,
    pub room_parameters: RoomParameters,
}

pub struct CreationReply {
    pub bonkroom_tx: mpsc::Sender<BonkRoomMessage>,
    pub room_link: String,
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
    0
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
pub struct RoomMaker {
    rx: mpsc::Receiver<RoomMakerMessage>,
    last_room_time: Option<Instant>,
    mods: String,
}

impl RoomMaker {
    pub async fn new(rx: mpsc::Receiver<RoomMakerMessage>) -> Result<RoomMaker> {
        let mut sgr_api_file = File::open("dependencies/sgrAPI.user.js").await?;
        let mut sgr_api = String::new();
        sgr_api_file.read_to_string(&mut sgr_api).await?;

        let mut injector_file = File::open("dependencies/sgrInjector.user.js").await?;
        let mut injector = String::new();
        injector_file.read_to_string(&mut injector).await?;

        Ok(RoomMaker {
            rx,
            last_room_time: None,
            mods: format!("{}{}", injector, sgr_api),
        })
    }

    pub async fn run(&mut self) {
        let room_rate_limit = Duration::from_secs(5);

        while let Some(mut message) = self.rx.recv().await {
            if let Some(last_room_time) = self.last_room_time {
                if let Some(wait_time) = room_rate_limit.checked_sub(last_room_time.elapsed()) {
                    sleep(wait_time).await;
                }
            }

            if message.room_parameters.min_level < 1 {
                let _ = message
                    .bonkroom_tx
                    .send(Err(anyhow!("min_level below 1 isn't supported.")));
                break;
            }

            let mut i = 0;
            loop {
                let err;
                match make_client(message.room_parameters.headless).await {
                    Ok(c) => {
                        match make_room(&c, &mut message.room_parameters, &self.mods).await {
                            Ok(room_link) => {
                                let (tx, rx) = mpsc::channel(10);
                                let mut bonkroom = BonkRoom::new(
                                    room_link.clone(),
                                    message.http,
                                    message.data,
                                    rx,
                                    c,
                                    message.leaderboard_tx,
                                    message.room_parameters,
                                );
                                tokio::spawn(async move {
                                    bonkroom.run().await;
                                });
                                let _ = message.bonkroom_tx.send(Ok(CreationReply {
                                    bonkroom_tx: tx,
                                    room_link,
                                }));

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
                    let _ = message.bonkroom_tx.send(Err(err));
                    break;
                }

                i += 1;
            }
            self.last_room_time = Some(Instant::now());
        }
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
async fn make_room(
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
