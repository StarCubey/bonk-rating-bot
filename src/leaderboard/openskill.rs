use std::f64;

use anyhow::Result;
use sqlx::{types::time::OffsetDateTime, Postgres, Transaction};

use crate::leaderboard::{self, Leaderboard};

use super::PlayerData;

///Update ratings with Weng-Lin Bradley-Terry Plackett-Luce where the Plackette-Luce pairings are reversed.
pub async fn update(
    lb: &Leaderboard,
    teams: Vec<Vec<String>>,
    ties: Vec<bool>,
) -> Result<(String, String)> {
    let mut trans = lb.db.begin().await?;

    let mut teams = teams;
    //Placements reversed for reverse Plackett-Luce
    teams.reverse();

    let mut teams_data = get_teams(lb, &mut trans, teams).await?;

    let today = OffsetDateTime::now_utc().date();
    for team in &mut teams_data {
        for player in team {
            if player.last_updated < today {
                player.last_updated = today;
                let day_num = today.to_julian_day() - player.last_updated.to_julian_day();

                player.rating_deviation = (player.rating_deviation.powi(2)
                    + (lb.settings.deviation_per_day * lb.settings.rating_scale).powi(2)
                        * f64::from(day_num))
                .sqrt();
            }
        }
    }

    struct Rating {
        r: f64,
        v: f64,
    }
    let mut team_ratings: Vec<Rating> = Vec::new();
    for team in &teams_data {
        let mut team_rating = Rating { r: 0., v: 0. };
        for player in team {
            team_rating.r += player.rating;
            team_rating.v += player.rating_deviation.powi(2);
        }

        team_ratings.push(team_rating);
    }

    let mut c_2 = 0.;
    let beta_2 = lb.settings.rating_scale;
    for team in &team_ratings {
        c_2 += team.v + beta_2
    }
    let c = c_2.sqrt().max(f64::EPSILON);

    //Count how many teams a team is tied with plus 1.
    let mut tie_nums: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < ties.len() + 1 {
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
    for team in &team_ratings {
        //Ratings are divided by the deviation of a logistic distribution when calculating win probability.
        //Ratings are negative in order to calculate the probability of losing
        //instead of the probability of winning for reverse Plackett-Luce.
        let exp_r = (-team.r / c * f64::consts::PI / 3f64.sqrt()).exp();
        let c_q = last_c_q + exp_r;

        exp_rs.push(exp_r);
        c_qs.push(c_q);
        last_c_q = c_q;
    }

    let mut delta_rs = Vec::new();
    let mut delta_vs = Vec::new();
    for (i, exp_r) in exp_rs.iter().enumerate() {
        let mut delta_r = 0.;
        let mut delta_v = 0.;

        if let Some(team) = team_ratings.get(i) {
            for j in i..c_qs.len() {
                if let (Some(c_q), Some(tie_num)) = (c_qs.get(j), tie_nums.get(j)) {
                    let p = exp_r / c_q;

                    if i == j {
                        delta_r += (1. - p) * team.v / c / *tie_num as f64;
                    } else {
                        delta_r += -p * team.v / c / *tie_num as f64;
                    }
                    delta_v += p * (1. - p) * team.v.powf(1.5) / c.powi(3) / *tie_num as f64;
                }
            }
        }

        delta_rs.push(delta_r);
        delta_vs.push(delta_v);
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
                    player.rating - player.rating_deviation * lb.settings.cre.unwrap_or(0.);
                player.last_updated = today;
            }
        }
    }

    apply_ratings(&mut trans, &teams_data).await?;
    lb.save_game(&mut trans, &teams_data, today, None, Some(&ties))
        .await?;

    trans.commit().await?;

    Ok(leaderboard::match_string(&teams_data, None, Some(&ties)))
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
