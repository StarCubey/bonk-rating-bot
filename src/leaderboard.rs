pub mod openskill;

use std::{f64, pin::Pin, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serenity::all::ChannelId;
use sqlx::{
    prelude::FromRow,
    types::time::{Date, OffsetDateTime},
    Pool, Postgres, Transaction,
};
use tokio::{
    sync::{mpsc, oneshot},
    time,
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
    pub cre: Option<f64>,
}

#[derive(Deserialize, Serialize)]
pub enum RatingAlgorithm {
    OpenSkill,
}

pub enum LeaderboardMessage {
    Update {
        teams: Vec<Vec<String>>,
        ties: Vec<bool>,
        match_str: oneshot::Sender<Result<String>>,
    },
}

#[derive(FromRow, Clone, Debug)]
pub struct PlayerData {
    pub id: i64,
    pub name: String,
    pub rating: f64,
    pub display_rating: f64,
    #[sqlx(skip)]
    pub old_rating: f64,
    pub rating_deviation: f64,
    pub last_updated: Date,
}

///Buffer 10, blocking send
pub struct Leaderboard {
    rx: mpsc::Receiver<LeaderboardMessage>,
    db: Arc<Pool<Postgres>>,
    pub ctx: serenity::all::Context,
    pub settings: LeaderboardSettings,
    id: i64,
    season: i32,
    update_timer: Pin<Box<time::Sleep>>,
    needs_update: bool,
    can_update: bool,
}

const DISCORD_MARKDOWN: [char; 9] = ['\\', '*', '_', '~', '`', '>', ':', '#', '-'];
const DISCORD_CHARACTER_LIMIT: usize = 2000;
const LEADERBOARD_DISPLAYED_PLACEMENTS: usize = 500;

impl Leaderboard {
    pub async fn new(
        rx: mpsc::Receiver<LeaderboardMessage>,
        ctx: serenity::all::Context,
        settings: LeaderboardSettings,
    ) -> Result<Leaderboard> {
        let db;
        let id: i64;
        let season;

        {
            let data = ctx.data.read().await;
            db = data
                .get::<crate::DatabaseKey>()
                .cloned()
                .ok_or(anyhow!("Failed to connect to database."))?
                .db;

            id = sqlx::query_scalar("SELECT id FROM leaderboard WHERE abbreviation = $1")
                .bind(&settings.abbreviation)
                .fetch_one(db.as_ref())
                .await?;

            let season_option: Option<i32> =
                sqlx::query_scalar("SELECT MAX(season_num) from lb_seasons WHERE lb_id = $1")
                    .bind(id)
                    .fetch_one(db.as_ref())
                    .await?;

            if let Some(_season) = season_option {
                season = _season;
            } else {
                let today = OffsetDateTime::now_utc().date();
                season = 0;
                sqlx::query(
                    "INSERT INTO lb_seasons \
                (lb_id, season_num, start, hard_reset) \
                VALUES ($1, $2, $3, $4)",
                )
                .bind(id)
                .bind(season)
                .bind(today)
                .bind(false)
                .execute(db.as_ref())
                .await?;
            }
        }

        Ok(Leaderboard {
            rx,
            db,
            ctx,
            settings,
            id,
            season,
            update_timer: Box::pin(time::sleep(Duration::MAX)),
            needs_update: false,
            can_update: true,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // let mut update_timer = interval(Duration::from_secs(5 * 60));
        // update_timer.tick().await;
        // update_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let update_delay = 120;

        match self.settings.algorithm {
            RatingAlgorithm::OpenSkill => loop {
                tokio::select! {
                    message = self.rx.recv() => {
                        let _ = match message {
                            Some(LeaderboardMessage::Update {
                                teams,
                                ties,
                                match_str,
                            }) => _ = {
                                _ = match_str.send(openskill::update(self, teams, ties).await);

                                if self.can_update {
                                    self.update_leaderboard().await?;
                                    self.update_timer = Box::pin(time::sleep(Duration::from_secs(update_delay)));
                                    self.can_update = false;
                                } else {
                                    self.needs_update = true;
                                }
                            },
                            None => break,
                        };
                    },
                    _ = self.update_timer.as_mut() => {
                        if self.needs_update {
                            self.update_leaderboard().await?;
                            self.update_timer = Box::pin(time::sleep(Duration::from_secs(update_delay)));
                            self.needs_update = false;
                        } else {
                            self.update_timer = Box::pin(time::sleep(Duration::MAX));
                            self.can_update = true;
                        }
                    },
                }
            },
        }

        Ok(())
    }

    pub async fn save_game(
        &mut self,
        trans: &mut Transaction<'static, Postgres>,
        teams: &Vec<Vec<PlayerData>>,
        day: Date,
        score: Option<&Vec<f64>>,
        ties: Option<&Vec<bool>>,
    ) -> Result<String> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO lb_games \
            (lb_id, season_num, day, score, ties) \
            VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(self.id)
        .bind(self.season)
        .bind(day)
        .bind(score)
        .bind(ties)
        .fetch_one(&mut **trans)
        .await?;

        for (i, team) in teams.iter().enumerate() {
            let player_ids: Vec<i64> = team.iter().map(|player| player.id).collect();
            let old_rating: Vec<f64> = team.iter().map(|player| player.old_rating).collect();
            let new_rating: Vec<f64> = team.iter().map(|player| player.display_rating).collect();

            sqlx::query(
                "INSERT INTO lb_game_teams \
                (game_id, team, player_ids, old_rating, new_rating) \
                VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(i as i32)
            .bind(player_ids)
            .bind(old_rating)
            .bind(new_rating)
            .execute(&mut **trans)
            .await?;
        }

        let match_string = match_string(teams, score, ties);

        let channel_id: Option<i64> =
            sqlx::query_scalar("SELECT match_channel FROM leaderboard WHERE id = $1")
                .bind(self.id)
                .fetch_one(&mut **trans)
                .await?;
        if let Some(channel_id) = channel_id {
            let channel_id = ChannelId::new(channel_id as u64);
            channel_id.say(&self.ctx.http, match_string.0).await?;
        }

        self.needs_update = true;

        Ok(match_string.1)
    }

    async fn update_leaderboard(&self) -> Result<()> {
        let mut players: Vec<PlayerData> =
            sqlx::query_as("SELECT * from lb_players WHERE lb_id = $1")
                .bind(self.id)
                .fetch_all(self.db.as_ref())
                .await?;

        players.sort_by(|a, b| b.display_rating.total_cmp(&a.display_rating));

        let mut lb_strings: Vec<String> = vec![self.settings.name.clone()];
        for (i, player) in players.iter().enumerate() {
            if i >= LEADERBOARD_DISPLAYED_PLACEMENTS {
                break;
            }

            let lb_string = lb_strings
                .last_mut()
                .context("Failed to generate leaderboard string.")?;
            let player_str = format!(
                "\n{}. {} ({:.0}, σ = {:.2})",
                i + 1,
                player.name,
                player.display_rating,
                player.rating_deviation
            );
            if lb_string.encode_utf16().count() + player_str.encode_utf16().count()
                > DISCORD_CHARACTER_LIMIT
            {
                lb_strings.push(player_str);
            } else {
                lb_string.push_str(&player_str);
            }
        }

        let channel_id: i64 = sqlx::query_scalar("SELECT channel FROM leaderboard WHERE id = $1")
            .bind(self.id)
            .fetch_one(self.db.as_ref())
            .await?;

        let channel_id = ChannelId::new(channel_id as u64);

        let messages: Vec<i64> =
            sqlx::query_scalar("SELECT messages FROM leaderboard WHERE id = $1")
                .bind(self.id)
                .fetch_one(self.db.as_ref())
                .await?;

        let mut new_messages: Vec<i64> = vec![];
        for (i, message_id) in messages.iter().enumerate() {
            let message = channel_id.message(&self.ctx.http, *message_id as u64).await;

            if let Ok(message) = message {
                if let Some(lb_string) = lb_strings.get(i) {
                    if i == new_messages.len() && message.content == *lb_string {
                        lb_strings.remove(i); //panics
                        new_messages.push(*message_id);
                    } else {
                        message.delete(&self.ctx.http).await?;
                    }
                } else {
                    message.delete(&self.ctx.http).await?;
                }
            }
        }

        for lb_string in lb_strings {
            new_messages.push(channel_id.say(&self.ctx.http, lb_string).await?.id.get() as i64);
        }
        sqlx::query("UPDATE leaderboard SET messages = $1 WHERE id = $2")
            .bind(new_messages)
            .bind(self.id)
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }
}

pub fn match_string(
    teams: &Vec<Vec<PlayerData>>,
    score: Option<&Vec<f64>>,
    ties: Option<&Vec<bool>>,
) -> (String, String) {
    if teams.len() < 1 {
        return (String::new(), String::new());
    }

    let ties_default = vec![false; teams.len() - 1];
    let ties = ties.unwrap_or(&ties_default);
    let no_teams = teams.iter().all(|players| players.len() == 1);

    let mut game = String::new();
    let summary;
    if let Some(score) = score {
        let mut score_teams: Vec<(&f64, Option<&Vec<PlayerData>>)> = score
            .iter()
            .enumerate()
            .map(|(i, score)| (score, teams.get(i)))
            .collect();
        score_teams.sort_by(|a, b| b.0.partial_cmp(a.0).unwrap_or(std::cmp::Ordering::Greater));

        summary = score_teams
            .iter()
            .map(|x| x.1)
            .filter_map(|team| team)
            .map(|team| {
                team.iter()
                    .map(|player| {
                        format!(
                            "{} {}",
                            player.name,
                            rating_string(player.display_rating, player.old_rating),
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            })
            .collect::<Vec<String>>()
            .join(", ");

        game += &score_teams
            .iter()
            .map(|x| x.0.to_string())
            .collect::<Vec<String>>()
            .join(" - ");
        game += "\n";

        if no_teams {
            game += &score_teams
                .iter()
                .map(|x| x.1)
                .filter_map(|team| team)
                .map(|team| team.get(0))
                .filter_map(|player| player)
                .map(|player| {
                    format!(
                        "{} {}",
                        escaped(&player.name),
                        rating_string(player.display_rating, player.old_rating),
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");
        } else {
            game += "\n";
            game += &score_teams
                .iter()
                .map(|x| x.1)
                .filter_map(|team| team)
                .map(|team| {
                    team.iter()
                        .map(|player| {
                            format!(
                                "{} {}",
                                escaped(&player.name),
                                rating_string(player.display_rating, player.old_rating)
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                })
                .collect::<Vec<String>>()
                .join("\n\n");
        }
    } else {
        summary = teams
            .iter()
            .map(|team| {
                team.iter()
                    .map(|player| {
                        format!(
                            "{} {}",
                            player.name,
                            rating_string(player.display_rating, player.old_rating),
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            })
            .collect::<Vec<String>>()
            .join(", ");

        let tie_teams: Vec<(&bool, &Vec<PlayerData>)> = teams
            .iter()
            .enumerate()
            .map(|(i, team)| {
                (
                    if i < 1 {
                        &false
                    } else {
                        ties.get(i - 1).unwrap_or(&false)
                    },
                    team,
                )
            })
            .collect();

        let mut last_placement = String::new();
        if teams.len() == 2 {
            if no_teams {
                if let Some(true) = ties.get(0) {
                    game += "Draw\n";
                }

                let winner = teams.get(0);
                let loser = teams.get(1);

                if let (Some(winner), Some(loser)) = (winner, loser) {
                    if let (Some(winner), Some(loser)) = (winner.get(0), loser.get(0)) {
                        if let Some(false) | None = ties.get(0) {
                            game += "Winner: "
                        }
                        game += &format!(
                            "{} {}\n",
                            escaped(&winner.name),
                            rating_string(winner.display_rating, winner.old_rating),
                        );
                        if let Some(false) | None = ties.get(0) {
                            game += "Loser: "
                        }
                        game += &format!(
                            "{} {}",
                            escaped(&loser.name),
                            rating_string(loser.display_rating, loser.old_rating),
                        );
                    }
                }
            } else {
                if let Some(true) = ties.get(0) {
                    game += "Draw\n\n";
                }

                let winners = teams.get(0);
                let losers = teams.get(1);

                if let (Some(winners), Some(losers)) = (winners, losers) {
                    if let Some(false) | None = ties.get(0) {
                        game += "Winner\n";
                    }
                    game += &winners
                        .iter()
                        .map(|player| {
                            format!(
                                "{} {}",
                                escaped(&player.name),
                                rating_string(player.display_rating, player.old_rating),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n");
                    game += "\n\n";

                    if let Some(false) | None = ties.get(0) {
                        game += "Loser\n";
                    }
                    game += &losers
                        .iter()
                        .map(|player| {
                            format!(
                                "{} {}",
                                escaped(&player.name),
                                rating_string(player.display_rating, player.old_rating),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n");
                }
            }
        } else {
            if no_teams {
                game += &tie_teams
                    .iter()
                    .enumerate()
                    .map(|(i, tie_team)| {
                        if let Some(player) = tie_team.1.get(0) {
                            let placement = if *tie_team.0 {
                                last_placement.clone()
                            } else {
                                to_ordinal(i + 1)
                            };
                            last_placement = placement.clone();

                            Some(format!(
                                "{}: {} {}",
                                placement,
                                escaped(&player.name),
                                rating_string(player.display_rating, player.old_rating),
                            ))
                        } else {
                            return None;
                        }
                    })
                    .filter_map(|str| str)
                    .collect::<Vec<String>>()
                    .join("\n");
            } else {
                game += &tie_teams
                    .iter()
                    .enumerate()
                    .map(|(i, tie_team)| {
                        let placement = if *tie_team.0 {
                            last_placement.clone()
                        } else {
                            to_ordinal(i + 1)
                        };
                        last_placement = placement.clone();

                        placement
                            + "\n"
                            + &tie_team
                                .1
                                .iter()
                                .map(|player| {
                                    format!(
                                        "{} {}",
                                        escaped(&player.name),
                                        rating_string(player.display_rating, player.old_rating),
                                    )
                                })
                                .collect::<Vec<String>>()
                                .join("\n")
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n");
            }
        }
    }

    (game, summary)
}

fn rating_string(new: f64, old: f64) -> String {
    let new = new.round();
    let old = old.round();

    format!(
        "({}, {}{})",
        new,
        if new >= old { "+" } else { "" },
        new - old
    )
}

fn escaped(str: &String) -> String {
    str.chars()
        .map(|c| {
            if DISCORD_MARKDOWN.contains(&c) {
                format!("\\{}", c)
            } else {
                c.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("")
}

fn to_ordinal(num: usize) -> String {
    let output = num.to_string();
    if num % 100 == 11 || num % 100 == 12 || num % 100 == 13 {
        return output + "th";
    }

    match num % 10 {
        1 => output + "st",
        2 => output + "nd",
        3 => output + "rd",
        _ => output + "th",
    }
}
