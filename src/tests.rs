use time::{Date, Month, OffsetDateTime};

use crate::{
    bonk_bot::bonk_commands::fuzzy_finder,
    leaderboard::{self, openskill, LeaderboardSettings, PlayerData},
};

#[test]
fn fuzzy_finder_test() {
    let output = fuzzy_finder(
        "s",
        &vec![
            "StarCubey".to_string(),
            "Arrrd God".to_string(),
            "Arow Godd".to_string(),
        ],
    );

    dbg!(output);
}

#[test]
fn match_string() {
    let player = PlayerData {
        id: 0,
        name: "StarCubey".to_string(),
        rating: 1600.,
        display_rating: 1600.,
        old_rating: 1500.,
        rating_deviation: 0.,
        last_updated: Date::from_calendar_date(2025, Month::January, 1).unwrap(),
    };
    let mut player2 = player.clone();
    player2.name = "F A C T S 2".to_string();
    let mut player3 = player.clone();
    player3.name = "F A C T S 3".to_string();

    let teams = vec![
        vec![player.clone(), player],
        vec![player2.clone(), player2],
        vec![player3.clone()],
    ];
    let match_string = leaderboard::match_string(&teams, Some(&vec![5., 1., 3.]), None);

    println!("{}\n{}", match_string.0, match_string.1);
}

#[test]
fn openskill() {
    let today = OffsetDateTime::now_utc().date();

    let settings = LeaderboardSettings {
        name: "".to_string(),
        abbreviation: "".to_string(),
        algorithm: leaderboard::RatingAlgorithm::OpenSkill,
        mean_rating: 5000.,
        rating_scale: 1000.,
        unrated_deviation: 2.,
        deviation_per_day: 0.0523,
        cre: Some(1.),
    };

    let player = PlayerData {
        id: 0,
        name: "StarCubey".to_string(),
        rating: 5000.,
        display_rating: 1500.,
        old_rating: 5000.,
        rating_deviation: 2000.,
        last_updated: today,
    };

    let mut player2 = player.clone();
    player2.name = "StarCubey2".to_string();
    player2.rating = 6000.;

    let mut teams_data = vec![vec![player.clone()], vec![player2.clone()]];

    openskill::reverse_pl(&settings, &vec![true], &mut teams_data);

    dbg!(teams_data);
}

#[test]
fn test() {
    let ties = vec![false, true, true, false];
    //Count how many teams a team is tied with plus 1.
    let mut tie_nums: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < tie_nums.len() + 1 {
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
}
