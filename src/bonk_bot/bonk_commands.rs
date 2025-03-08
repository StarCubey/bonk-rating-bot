use serde_json::json;
use tokio::time::Instant;

use super::{
    bonk_room::{BonkRoom, Player, RoomState},
    room_maker::Mode,
};

pub async fn pick(arguments: &Vec<&str>, name: &String, bonk_room: &mut BonkRoom) {
    let my_name = dotenv::var("BONK_USERNAME").unwrap_or("".to_string());
    let captain_name = &bonk_room.player_data.captain.1.name;

    if let RoomState::Idle | RoomState::DuringGame = bonk_room.state {
        bonk_room
            .chat_queue
            .push_back("You can't pick an opponent right now.".to_string());
        return;
    }
    if *name != *captain_name {
        bonk_room
            .chat_queue
            .push_back(format!("It's {}'s turn to pick.", captain_name));
        return;
    }

    let keys: Vec<String> = bonk_room
        .player_data
        .players
        .iter()
        .map(|p| p.1.name.clone())
        .filter(|p| p != captain_name && *p != my_name)
        .collect();
    let pick_name = arguments.join(" ");
    if pick_name != "".to_string() {
        let matches = fuzzy_finder(&pick_name, &keys);

        if matches.len() == 0 {
            bonk_room
                .chat_queue
                .push_back("No matches found. Please try again.".to_string());
            return;
        } else if matches.len() >= 2 {
            bonk_room.chat_queue.push_back(format!(
                "I couldn't find a match. \
                Here are the matches I considered: {}. Please try again.",
                matches.join(", ")
            ));
            return;
        }
        let default = &"".to_string();
        let match_ = matches.get(0).unwrap_or(default);

        let default = &(0, Player::new());
        let match_ = bonk_room
            .player_data
            .players
            .iter()
            .find(|p| p.1.name == *match_)
            .unwrap_or(default);

        let mut team = 1;
        if let Mode::Football = bonk_room.room_parameters.mode {
            match bonk_room.player_data.team_flip {
                false => team = 2,
                true => team = 3,
            }
        }

        match bonk_room.player_data.pick_progress {
            0 => {
                let _ = bonk_room
                    .client
                    .execute(
                        "window\
                        .bonkHost\
                        .toolFunctions\
                        .networkEngine\
                        .changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(match_.0), json!(team)],
                    )
                    .await;

                bonk_room.chat_queue.push_back(format!(
                    "{} Has been selected! Type !r to start the game.",
                    match_.1.name
                ));

                bonk_room.player_data.other_player = match_.clone();

                bonk_room.player_data.pick_progress = 1;
                bonk_room.state_changed = Instant::now();
            }
            1..=8 => {
                let _ = bonk_room
                    .client
                    .execute(
                        "window\
                        .bonkHost\
                        .toolFunctions\
                        .networkEngine\
                        .changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(bonk_room.player_data.other_player.0), json!(0)],
                    )
                    .await;

                let _ = bonk_room
                    .client
                    .execute(
                        "window\
                        .bonkHost\
                        .toolFunctions\
                        .networkEngine\
                        .changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(match_.0), json!(team)],
                    )
                    .await;

                bonk_room.chat_queue.push_back(format!(
                    "{} Has been selected! Type \"!r\" to start the game.",
                    match_.1.name
                ));

                bonk_room.player_data.other_player = match_.clone();

                bonk_room.player_data.pick_progress += 1;
                bonk_room.state_changed = Instant::now();
            }
            _ => bonk_room.chat_queue.push_back(
                "Allowed repicks have been exhausted. Type \"!r\" to start the game.".to_string(),
            ),
        }
    } else {
        bonk_room.chat_queue.push_back(
            "Error: Player argument missing. Please specify a player to pick.".to_string(),
        );
    }
}

pub async fn ready(name: &String, bonk_room: &mut BonkRoom) {
    if let RoomState::Idle | RoomState::DuringGame = bonk_room.state {
        return;
    } else if bonk_room.player_data.pick_progress == 0 {
        return;
    } else {
        if *name == bonk_room.player_data.captain.1.name {
            bonk_room.player_data.captain.1.ready = true;
        } else if *name == bonk_room.player_data.other_player.1.name {
            bonk_room.player_data.other_player.1.ready = true;
        }
    }
}

pub fn fuzzy_finder(query: &str, keys: &[String]) -> Vec<String> {
    let query_lower: Vec<u8> = query
        .as_bytes()
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let keys_lower: Vec<Vec<u8>> = keys
        .iter()
        .map(|s| {
            s.as_bytes()
                .iter()
                .map(|c| c.to_ascii_lowercase())
                .collect()
        })
        .collect();

    let scores: Vec<(&String, f32, f32)> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let default = &Vec::new();
            let key_lower = keys_lower.get(i).unwrap_or(default);

            return (
                key,
                fuzzy_score(query.as_bytes(), key.as_bytes(), 0, 0),
                fuzzy_score(&query_lower, key_lower, 0, 0),
            );
        })
        .collect();

    let mut best: Vec<(&String, f32, f32)> = Vec::new();
    for entry in scores {
        if let Some(best_0) = best.get(0) {
            if entry.2 > best_0.2 {
                best = vec![entry];
            } else if entry.2 == best_0.2 {
                if entry.1 > best_0.1 {
                    best = vec![entry];
                } else if entry.1 == best_0.1 {
                    if entry.0.len() < best_0.0.len() {
                        best = vec![entry];
                    } else if entry.0.len() == best_0.0.len() {
                        best.push(entry);
                    }
                }
            }
        } else {
            if entry.2 != 0.0 {
                best.push(entry);
            }
        }
    }

    return best.iter().map(|x| x.0.clone()).collect();
}

fn fuzzy_score(query: &[u8], key: &[u8], query_index: usize, key_index: usize) -> f32 {
    if query_index >= query.len() {
        return 0.0;
    }
    if key_index >= key.len() {
        return 0.0;
    }

    let query_char = query.get(query_index);
    let key_slice = key.get(key_index..key.len());
    if let (Some(query_char), Some(key_slice)) = (query_char, key_slice) {
        return key_slice
            .iter()
            .enumerate()
            .map(|(i, c)| (i + key_index, c))
            .map(|(i, c)| {
                if c == query_char {
                    if i == key_index {
                        return fuzzy_score(query, key, query_index + 1, i + 1) + 1.0;
                    } else {
                        return fuzzy_score(query, key, query_index + 1, i + 1) + 0.9;
                    }
                } else {
                    return 0.0;
                }
            })
            .max_by(|num1, num2| num1.partial_cmp(num2).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
    } else {
        return 0.0;
    }
}
