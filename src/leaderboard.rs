use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::{mpsc, oneshot},
};

#[derive(Deserialize, Serialize)]
pub struct LeaderboardSettings {
    pub name: String,
    pub abbreviation: String,
    pub algorithm: RatingAlgorithm,
    pub mean_rating: f64,
    pub rating_scale: f64,
    pub unrated_deviation: f64,
    pub deviation_per_day: f64,
    pub glicko_rp_days: Option<i32>,
}

#[derive(Deserialize, Serialize)]
pub enum RatingAlgorithm {
    OpenSkill,
}

pub enum LeaderboardMessage {}

///Buffer 10, blocking send
pub struct Leaderboard {
    //Sending senders since BonkBot isn't allowed to hold on to an UpdateMessage sender.
    rx: mpsc::Receiver<LeaderboardMessage>,
    settings: LeaderboardSettings,
}

impl Leaderboard {
    pub fn new(
        rx: mpsc::Receiver<LeaderboardMessage>,
        settings: LeaderboardSettings,
    ) -> Leaderboard {
        Leaderboard { rx, settings }
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(message) = self.rx.recv().await {
            //TODO
        }

        Ok(())
    }
}
