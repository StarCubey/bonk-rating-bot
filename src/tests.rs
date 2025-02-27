use crate::bonk_bot::bonk_commands::fuzzy_finder;

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
