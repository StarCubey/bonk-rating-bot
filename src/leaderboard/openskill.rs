use std::f64;

use anyhow::Result;
use sqlx::{types::time::OffsetDateTime, Postgres, Transaction};

use crate::leaderboard::Leaderboard;

use super::{LeaderboardSettings, PlayerData};

///Update ratings with Weng-Lin
pub async fn update(
    lb: &mut Leaderboard,
    teams: Vec<Vec<String>>,
    ties: Vec<bool>,
) -> Result<String> {
    let mut trans = lb.db.begin().await?;

    let mut teams_data = get_teams(lb, &mut trans, teams).await?;

    let today = OffsetDateTime::now_utc().date();

    reverse_pl(&lb.settings, &ties, &mut teams_data);

    apply_ratings(&mut trans, &teams_data).await?;
    let match_string = lb
        .save_game(&mut trans, &teams_data, today, None, Some(&ties))
        .await?;

    trans.commit().await?;

    Ok(match_string)
}

///Rating update for Weng-Lin Bradley-Terry reverse Plackett-Luce (rankings and rating updates are reversed)
pub fn reverse_pl(
    settings: &LeaderboardSettings,
    ties: &Vec<bool>,
    teams_data: &mut Vec<Vec<PlayerData>>,
) {
    let today = OffsetDateTime::now_utc().date();
    for team in &mut *teams_data {
        for player in team {
            if player.last_updated < today {
                player.last_updated = today;
                let day_num = today.to_julian_day() - player.last_updated.to_julian_day();

                player.rating_deviation = (player.rating_deviation.powi(2)
                    + (settings.deviation_per_day * settings.rating_scale).powi(2)
                        * f64::from(day_num))
                .sqrt();
            }
        }
    }

    #[derive(Debug)]
    struct Rating {
        r: f64,
        v: f64,
    }
    let mut team_ratings: Vec<Rating> = Vec::new();
    for team in &*teams_data {
        let mut team_rating = Rating { r: 0., v: 0. };
        for player in team {
            team_rating.r += player.rating;
            team_rating.v += player.rating_deviation.powi(2);
        }

        team_ratings.push(team_rating);
    }

    let mut c_2 = 0.;
    let beta_2 = settings.rating_scale.powi(2);
    for team in &team_ratings {
        c_2 += team.v + beta_2
    }
    let c = c_2.sqrt().max(f64::EPSILON);

    //Count how many teams a team is tied with plus 1.
    let mut tie_nums: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < teams_data.len() {
        let mut count = 1usize;
        let mut j = i;
        while let Some(true) = ties.get(j) {
            count += 1;
            j += 1;
        }

        for _ in 0..count {
            tie_nums.push(count);
        }

        i += count;
    }

    let mut exp_rs = Vec::new();
    let mut c_qs = Vec::new();
    let mut last_c_q = 0f64;
    let mut i = 0;
    while i < team_ratings.len() {
        let Some(tie_num) = tie_nums.get(i) else {
            break;
        };
        let mut tie_group = vec![];
        for _ in 0..*tie_num {
            let Some(team) = team_ratings.get(i) else {
                continue;
            };
            //Ratings are divided by the deviation of a logistic distribution when calculating win probability.
            //Ratings are negative in order to calculate the probability of losing
            //instead of the probability of winning for reverse Plackett-Luce.
            let exp_r = (-team.r / c * f64::consts::PI / 3f64.sqrt()).exp();
            tie_group.push(exp_r);

            last_c_q += exp_r;
        }
        c_qs.push(last_c_q);
        exp_rs.push(tie_group);

        i += tie_num;
    }

    let mut delta_rs = Vec::new();
    let mut delta_vs = Vec::new();
    //Index of team
    let mut i = 0;
    for (tie_idx, tie_group) in exp_rs.iter().enumerate() {
        for exp_r in tie_group {
            let mut delta_r = 0.;
            let mut delta_v = 0.;

            let Some(team) = team_ratings.get(i) else {
                continue;
            };

            //Index of opposing teams grouped by ties
            let mut j = tie_idx;
            while j < exp_rs.len() {
                let Some(tie_group) = exp_rs.get(j) else {
                    continue;
                };
                let Some(c_q) = c_qs.get(j) else { continue };

                //p is 1 if exp_r == c_q which has no effect on
                //ratings assuming tie_idx == j and tie_group.len() == 1.
                let p = exp_r / c_q;
                //Update for win/loss
                if j == tie_idx {
                    delta_r += (1. - p) * team.v / c / tie_group.len() as f64;
                } else {
                    delta_r += -p * team.v / c / tie_group.len() as f64;
                }
                //Additional losses for ties
                for _ in 1..tie_group.len() {
                    delta_r += -p * team.v / c / tie_group.len() as f64;
                }

                delta_v += p * (1. - p) * team.v.powf(1.5) / c.powi(3);

                j += 1;
            }

            delta_rs.push(delta_r);
            delta_vs.push(delta_v);
            i += 1;
        }
    }
    for (i, team) in team_ratings.iter().enumerate() {
        if let (Some(players), Some(delta_r), Some(delta_v)) =
            (teams_data.get_mut(i), delta_rs.get(i), delta_vs.get(i))
        {
            for player in players {
                //Subtracted for reverse Plackett-Luce
                player.rating -= player.rating_deviation.powi(2) / team.v * delta_r;
                player.rating_deviation = (player.rating_deviation.powi(2)
                    * (1. - player.rating_deviation.powi(2) / team.v * delta_v).max(0.0001))
                .sqrt();
                player.display_rating =
                    player.rating - player.rating_deviation * settings.cre.unwrap_or(0.);
            }
        }
    }
}

async fn get_teams(
    lb: &Leaderboard,
    trans: &mut Transaction<'static, Postgres>,
    teams: Vec<Vec<String>>,
) -> Result<Vec<Vec<PlayerData>>> {
    let today = OffsetDateTime::now_utc().date();

    let mut teams_data: Vec<Vec<PlayerData>> = Vec::new();
    for team in teams {
        let mut team_data: Vec<PlayerData> = Vec::new();
        for player in team {
            let player_data_option: Option<PlayerData> = sqlx::query_as(
                "SELECT id, name, rating, rating_deviation, display_rating, last_updated FROM lb_players WHERE name = $1",
            )
            .bind(&player)
            .fetch_optional(&mut **trans)
            .await?;

            let player_data;
            if let Some(mut data) = player_data_option {
                data.old_rating = data.display_rating;
                player_data = data;
            } else {
                let rating = lb.settings.mean_rating;
                let rating_deviation = lb.settings.unrated_deviation * lb.settings.rating_scale;
                let display_rating = rating - rating_deviation * lb.settings.cre.unwrap_or(0.);

                let player_id: i64 = sqlx::query_scalar(
                    "INSERT INTO lb_players \
                    (lb_id, rating, rating_deviation, name, display_rating, last_updated) \
                    VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
                )
                .bind(lb.id)
                .bind(rating)
                .bind(rating_deviation)
                .bind(&player)
                .bind(display_rating)
                .bind(today)
                .fetch_one(&mut **trans)
                .await?;

                player_data = PlayerData {
                    id: player_id,
                    name: player,
                    rating,
                    display_rating,
                    old_rating: display_rating,
                    rating_deviation,
                    last_updated: today,
                }
            }
            team_data.push(player_data);
        }
        teams_data.push(team_data);
    }

    Ok(teams_data)
}

async fn apply_ratings(
    trans: &mut Transaction<'static, Postgres>,
    teams_data: &Vec<Vec<PlayerData>>,
) -> Result<()> {
    for team in teams_data {
        for player in team {
            sqlx::query(
                "UPDATE lb_players \
                SET rating = $1, rating_deviation = $2, display_rating = $3, last_updated = $4 \
                WHERE id = $5",
            )
            .bind(player.rating)
            .bind(player.rating_deviation)
            .bind(player.display_rating)
            .bind(player.last_updated)
            .bind(player.id)
            .execute(&mut **trans)
            .await?;
        }
    }

    Ok(())
}
