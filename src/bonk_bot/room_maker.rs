use std::fs::File;
use std::io::Write;
use std::time::Duration;

use anyhow::Context;
use anyhow::{anyhow, Result};
use fantoccini::{ClientBuilder, Locator};
use image::GenericImageView;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Instant};

pub struct RoomMakerMessage {
    pub client_sender: oneshot::Sender<Result<fantoccini::Client>>,
}

//Buffer 3, blocking send
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
        let room_rate_limit = Duration::from_secs(10);

        while let Some(message) = self.rx.recv().await {
            if let Some(last_room_time) = self.last_room_time {
                if let Some(wait_time) = room_rate_limit.checked_sub(last_room_time.elapsed()) {
                    sleep(wait_time).await;
                }
            }

            let c = make_client().await;

            if let Ok(c) = c {
                let result = make_room(&c).await;

                let result = match result {
                    Ok(_) => Ok(c),
                    Err(e) => {
                        let _ = c.close();
                        Err(e)
                    }
                };

                let _ = message.client_sender.send(result);

                self.last_room_time = Some(Instant::now());
            }
        }
    }
}

async fn make_client() -> Result<fantoccini::Client> {
    let port = dotenv::var("CHROMEDRIVER_PORT")?;

    let capabilities = json!({
        "moz:firefoxOptions": {
            "args": ["--headless", "--mute-audio", "--width=1920", "--height=1080"]
        },
        "goog:chromeOptions": {
            "binary": dotenv::var("CHROME_PATH")?,
            "args": ["--window-size=1920,1080"]
            // "args": ["--window-size=1920,1080", "--headless", "--mute-audio"]
        },
    });

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

async fn make_room(c: &fantoccini::Client) -> Result<()> {
    let credentials = vec![json!({
        "username": dotenv::var("BONK_USERNAME")?,
        "password": dotenv::var("BONK_PASSWORD")?,
    })];

    let room_info = vec![json!({
        "roomName": "test",
        "roomPass": "fkdsa;lfdjskflas;jf",
        "maxPlayers": 4,
        "minLevel": 1,
        "unlisted": true,
    })];

    c.goto("https://bonk.io/").await?;

    let game_frame = c.find(Locator::Id("maingameframe")).await?;
    game_frame.enter_frame().await?;

    //Waiting for login window to appear.
    loop {
        match c
            .find(Locator::Id("guestOrAccountContainer_accountButton"))
            .await?
            .click()
            .await
        {
            Ok(_) => break,
            _ => (),
        }
    }
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
    c.find(Locator::Id("loginwindow_submitbutton"))
        .await?
        .click()
        .await?;

    //Waiting for top bar to appear.
    loop {
        match c
            .find(Locator::Id("pretty_top_volume"))
            .await?
            .click()
            .await
        {
            Ok(_) => break,
            _ => (),
        }
    }
    c.find(Locator::Id("pretty_top_volume_music"))
        .await?
        .click()
        .await?;
    c.find(Locator::Id("classic_mid_customgame"))
        .await?
        .click()
        .await?;
    c.find(Locator::Id("roomlistcreatebutton"))
        .await?
        .click()
        .await?;

    //Waiting for "Create Game" window to appear.
    loop {
        match c
            .find(Locator::Id("roomlistcreatewindowgamename"))
            .await?
            .click()
            .await
        {
            Ok(_) => break,
            _ => (),
        }
    }
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
        room_info,
    )
    .await?;

    c.find(Locator::Id("roomlistcreatecreatebutton"))
        .await?
        .click()
        .await?;

    let mut room_link;
    loop {
        match c
            .find(Locator::Id("newbonklobby_linkbutton"))
            .await?
            .click()
            .await
        {
            Err(_) => continue,
            _ => (),
        };
        let status_elements = c
            .find_all(Locator::Css(".newbonklobby_chat_status"))
            .await?;
        room_link = String::from("");
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
        if room_link != "".to_string() {
            break;
        }
    }
    println!("Room created: {}", room_link);

    //Game creation test.
    c.find(Locator::Id("newbonklobby_modebutton"))
        .await?
        .click()
        .await?;
    loop {
        if let Ok(_) = c
            .find(Locator::Id("newbonklobby_mode_football"))
            .await?
            .click()
            .await
        {
            break;
        }
    }
    c.find(Locator::Id("newbonklobby_startbutton"))
        .await?
        .click()
        .await?;
    loop {
        if c.find(Locator::Id("gamerenderer"))
            .await?
            .css_value("visibility")
            .await?
            != "hidden"
        {
            break;
        }
    }
    let screenshot = c
        .find(Locator::Id("gamerenderer"))
        .await?
        .screenshot()
        .await?;
    let mut file = File::create("screenshot.png")?;
    file.write_all(&screenshot)?;

    let img = image::load_from_memory_with_format(&screenshot, image::ImageFormat::Png)?;
    let pixel = img.get_pixel(100, 100);
    //Note: you can iterate over the pixels in an image with img.pixels().
    println!("{} {} {}", pixel.0[0], pixel.0[1], pixel.0[2]);

    Ok(())
}
