use anyhow::Context;
use anyhow::{anyhow, Result};
use fantoccini::error::CmdError;
use fantoccini::{ClientBuilder, Locator};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Instant};

use super::bonk_room::{BonkRoom, BonkRoomMessage};

pub struct RoomMakerMessage {
    pub bonkroom_tx: oneshot::Sender<Result<CreationReply>>,
    pub room_parameters: RoomParameters,
}

pub struct CreationReply {
    pub bonkroom_tx: mpsc::Sender<BonkRoomMessage>,
    pub room_link: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RoomParameters {
    pub headless: Option<bool>,
    pub name: String,
    pub password: Option<String>,
    pub max_players: i32,
    pub min_level: i32,
    pub unlisted: Option<bool>,
    pub mode: Mode,
    pub rounds: i32,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Mode {
    Football,
    Simple,
    DeathArrows,
    Arrows,
    Grapple,
    VTOL,
    Classic,
}

///Buffer 3, blocking send
pub struct RoomMaker {
    rx: mpsc::Receiver<RoomMakerMessage>,
    last_room_time: Option<Instant>,
}

impl RoomMaker {
    pub fn new(rx: mpsc::Receiver<RoomMakerMessage>) -> RoomMaker {
        RoomMaker {
            rx,
            last_room_time: None,
        }
    }

    pub async fn run(&mut self) {
        let room_rate_limit = Duration::from_secs(5);

        while let Some(message) = self.rx.recv().await {
            if let Some(last_room_time) = self.last_room_time {
                if let Some(wait_time) = room_rate_limit.checked_sub(last_room_time.elapsed()) {
                    sleep(wait_time).await;
                }
            }

            let mut i = 0;
            loop {
                let err;
                match make_client(message.room_parameters.headless.unwrap_or(true)).await {
                    Ok(c) => {
                        match make_room(&c, &message.room_parameters).await {
                            Ok(room_link) => {
                                let (tx, rx) = mpsc::channel(10);
                                let mut bonkroom = BonkRoom::new(rx, c, message.room_parameters);
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
                if i >= 4 {
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
            "args": ["--headless", "--mute-audio", "--width=1920", "--height=1080"]
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
async fn make_room(c: &fantoccini::Client, room_parameters: &RoomParameters) -> Result<String> {
    let credentials = vec![json!({
        "username": dotenv::var("BONK_USERNAME")?,
        "password": dotenv::var("BONK_PASSWORD")?,
    })];

    let room_info = vec![json!({
        "roomName": room_parameters.name,
        "roomPass": room_parameters.password.clone().unwrap_or(String::from("")),
        "maxPlayers": room_parameters.max_players,
        "minLevel": room_parameters.min_level,
        "unlisted": room_parameters.unlisted.unwrap_or(false),
    })];

    let mode = match &room_parameters.mode {
        Mode::Football => "f",
        Mode::Simple => "bs",
        Mode::DeathArrows => "ard",
        Mode::Arrows => "ar",
        Mode::Grapple => "sp",
        Mode::VTOL => "v",
        Mode::Classic => "b",
    };

    let mut injector_file = File::open("dependencies/Code Injector - Bonk.io.user.js").await?;
    let mut injector = String::new();
    injector_file.read_to_string(&mut injector).await?;

    let mut bonk_host_file = File::open("dependencies/Bonk Host.user.js").await?;
    let mut bonk_host = String::new();
    bonk_host_file.read_to_string(&mut bonk_host).await?;

    let mut bonk_playlists_file = File::open("dependencies/Bonk Playlists.user.js").await?;
    let mut bonk_playlists = String::new();
    bonk_playlists_file
        .read_to_string(&mut bonk_playlists)
        .await?;

    c.goto("https://bonk.io/").await?;

    //I tried decreasing the retry time, but it broke, so I'm just not going to question it.
    c.wait()
        .at_most(Duration::from_secs(5))
        .every(Duration::from_millis(250))
        .for_element(Locator::Id("maingameframe"))
        .await?
        .enter_frame()
        .await?;

    c.execute(&injector, Vec::new()).await?;
    c.execute(&bonk_host, Vec::new()).await?;
    c.execute(&bonk_playlists, Vec::new()).await?;

    let account_button =
        wait_for_element(&c, Locator::Id("guestOrAccountContainer_accountButton")).await?;

    retry(|| async { account_button.click().await }).await?;

    let login_button = c
        .wait()
        .for_element(Locator::Id("loginwindow_submitbutton"))
        .await?;
    c.execute(
        "\
        document.getElementById('loginwindow_username').value \
            = arguments[0].username;\
        document.getElementById('loginwindow_password').value \
            = arguments[0].password;\
        ",
        credentials,
    )
    .await?;

    //Retry in case script hasn't loaded yet
    let mut i = 0;
    loop {
        retry(|| async { login_button.click().await }).await?;

        //Waiting for top bar to appear.
        if let Ok(top_bar) = wait_for_element(&c, Locator::Id("pretty_top_volume")).await {
            if let Ok(_) = retry(|| async { top_bar.click().await }).await {
                break;
            }
        }

        if i >= 4 {
            return Err(anyhow!("Failed to log in."));
        }

        i += 1;
    }

    let music_button = wait_for_element(&c, Locator::Id("pretty_top_volume_music")).await?;
    retry(|| async { music_button.click().await }).await?;

    let custom_game_button = wait_for_element(&c, Locator::Id("classic_mid_customgame")).await?;
    retry(|| async { custom_game_button.click().await }).await?;

    let mut i = 0;
    loop {
        let create_button = wait_for_element(&c, Locator::Id("roomlistcreatebutton")).await?;
        retry(|| async { create_button.click().await }).await?;

        //Waiting for "Create Game" window to appear.
        let room_name_input =
            wait_for_element(&c, Locator::Id("roomlistcreatewindowgamename")).await?;
        retry(|| async { room_name_input.click().await }).await?;

        c.execute(
            "\
            document.getElementById('roomlistcreatewindowgamename').value \
                = arguments[0].roomName;\
            document.getElementById('roomlistcreatewindowpassword').value \
                = arguments[0].roomPass;\
            document.getElementById('roomlistcreatewindowmaxplayers').value \
                = arguments[0].maxPlayers;\
            document.getElementById('roomlistcreatewindowminlevel').value \
                = arguments[0].minLevel;\
            if(arguments[0].unlisted){\
                document.getElementById('roomlistcreatewindowunlistedcheckbox')\
                    .checked = true;\
            }\
        ",
            room_info.clone(),
        )
        .await?;

        let create_button = wait_for_element(&c, Locator::Id("roomlistcreatecreatebutton")).await?;
        retry(|| async { create_button.click().await }).await?;

        //Checking for successful room creation.
        let chat = wait_for_element(&c, Locator::Id("newbonklobby_chatbox")).await?;
        if let Ok(_) = retry(|| async { chat.click().await }).await {
            break;
        } else if i >= 4 {
            return Err(anyhow!("Room creation timeout."));
        }

        let cancel_button =
            wait_for_element(&c, Locator::Id("sm_connectingWindowCancelButton")).await?;
        retry(|| async { cancel_button.click().await }).await?;

        i += 1;
    }

    let link_button = wait_for_element(&c, Locator::Id("newbonklobby_linkbutton")).await?;
    retry(|| async { link_button.click().await }).await?;

    let status_elements = c
        .find_all(Locator::Css(".newbonklobby_chat_status"))
        .await?;
    let mut room_link = String::from("");
    if status_elements.len() > 0 {
        room_link = status_elements
            .get(status_elements.len() - 1)
            .context("Failed to parse room link.")?
            .text()
            .await?;
    }
    room_link = room_link
        .split(' ')
        .last()
        .context("Failed to parse room link.")?
        .to_string();

    let rounds_input = wait_for_element(&c, Locator::Id("newbonklobby_roundsinput")).await?;
    retry(|| async {
        rounds_input
            .send_keys(&room_parameters.rounds.to_string())
            .await
    })
    .await?;

    c.execute(
        "window.bonkHost.bonkSetMode(arguments[0]);\
        window.bonkHost.toolFunctions.networkEngine.changeOwnTeam(0);\
        window.bonkHost.toolFunctions.networkEngine.sendNoHostSwap();",
        vec![json!(mode)],
    )
    .await?;

    c.find(Locator::Id("newbonklobby_teamlockbutton"))
        .await?
        .click()
        .await?;

    println!("Room created!");

    //Game creation test.
    /*
    c.find(Locator::Id("newbonklobby_startbutton"))
        .await?
        .click()
        .await?;
    */

    Ok(room_link)
}

async fn wait_for_element(
    client: &fantoccini::Client,
    locator: Locator<'_>,
) -> Result<fantoccini::elements::Element, CmdError> {
    client
        .wait()
        .at_most(Duration::from_secs(10))
        .every(Duration::from_millis(250))
        .for_element(locator)
        .await
}

async fn retry<F, Fut, T, E>(mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let max_duration = Duration::from_secs(10);
    let interval = Duration::from_millis(250);

    loop {
        match operation().await {
            Ok(res) => return Ok(res),
            Err(e) => {
                if start.elapsed() >= max_duration {
                    return Err(e);
                }
                sleep(interval).await;
            }
        }
    }
}
