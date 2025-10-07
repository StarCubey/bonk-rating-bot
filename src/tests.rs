use time::{Date, Month};

use crate::{
    bonk_bot::bonk_commands::fuzzy_finder,
    leaderboard::{self, PlayerData},
};

#[test]
fn fuzzy_finder_test() {
    let output = fuzzy_finder(
        "RDDuwu",
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
    let match_string = leaderboard::match_string(&teams, Some(vec![5., 1., 3.]), None);

    println!("{}\n{}", match_string.0, match_string.1);
}
