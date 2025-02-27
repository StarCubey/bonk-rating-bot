use std::ascii::AsciiExt;

use super::bonk_room::{BonkRoom, RoomState};

pub fn pick(arguments: &Vec<&str>, name: &String, bonk_room: &mut BonkRoom) {
    let captain_name = &bonk_room.player_data.captain.1.name;

    if let RoomState::Idle | RoomState::DuringGame = bonk_room.state {
        bonk_room
            .chat_queue
            .push_back("You can't pick an opponent right now.".to_string());
    }
    if *name != *captain_name {
        bonk_room
            .chat_queue
            .push_back(format!("It's {}'s turn to pick.", captain_name));
    }

    let pick_name = arguments.join(" ");
    if pick_name != "".to_string() {
        match bonk_room.player_data.pick_progress {
            0 => {}
            1..=2 => {}
            _ => {}
        }
    } else {
        bonk_room.chat_queue.push_back(
            "Error: Player argument missing. Please specify a player to pick.".to_string(),
        );
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
