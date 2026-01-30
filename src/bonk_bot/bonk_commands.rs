use std::time::Duration;

use rand::Rng;
use serde_json::json;
use tokio::time::{self, Instant};

use crate::bonk_bot::bonk_room::{GamePlayers, Player, State};

use super::{bonk_room::BonkRoom, room_maker::Mode};

pub async fn pick(room: &mut BonkRoom, id: i32, name: String) {
    let State::Pick = room.state else {
        return;
    };

    let keys = room
        .queue
        .iter()
        .filter(|p| p.1.in_room && p.1.team == 0)
        .map(|p| p.1.name.clone())
        .collect::<Vec<String>>();

    room.game_players = match room.game_players.clone() {
        GamePlayers::Singles { picker, picked } => {
            if let Some(_) = picked {
                return;
            }
            let mut picked = None;

            if let Some(picker) = &picker {
                if id == picker.id {
                    let matches = fuzzy_finder(&name, &keys);
                    if matches.len() != 1 {
                        room.chat("I couldn't find a match.".to_string()).await;
                        return;
                    }
                    if let Some(matched) = matches.get(0) {
                        let matched = room.queue.iter().find(|p| p.1.name == *matched);
                        if let Some(matched) = matched {
                            picked = Some(matched.1.clone());
                            if let Mode::Football = room.room_parameters.mode {
                                let _ = room.client.execute(
                                    "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                                    vec![json!(matched.1.id), json!(if room.team_flip {2} else {3})]
                                ).await;
                            } else {
                                let _ = room.client.execute(
                                    "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                                    vec![json!(matched.1.id), json!(1)]
                                ).await;
                            }

                            room.start_map_selection().await;
                        }
                    }
                } else {
                    room.chat(format!("It's {}'s turn to pick.", picker.name))
                        .await;
                    return;
                }
            }

            GamePlayers::Singles { picker, picked }
        }
        GamePlayers::Teams { teams, picker_idx } => {
            let mut teams = teams.clone();
            let mut picker_idx = picker_idx;

            if let Some(false) = teams
                .iter()
                .map(|t| t.len() >= room.room_parameters.team_size)
                .reduce(|a, b| a && b)
            {
                let Some(team) = teams.get_mut(picker_idx) else {
                    return;
                };
                let Some(picker) = team.get(0) else {
                    return;
                };
                if id != picker.id {
                    room.chat(format!("It's {}'s turn to pick.", picker.name))
                        .await;
                    return;
                }

                let matches = fuzzy_finder(&name, &keys);
                if matches.len() != 1 {
                    room.chat("I couldn't find a match.".to_string()).await;
                    return;
                }
                let Some(matched) = matches.get(0) else {
                    return;
                };
                let Some(matched) = room.queue.iter().find(|p| p.1.name == *matched) else {
                    return;
                };

                if let Mode::Football = room.room_parameters.mode {
                    if picker_idx == 0 {
                        let _ = room.client.execute(
                            "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                            vec![json!(matched.1.id), json!(if room.team_flip {3} else {2})]
                        ).await;
                    } else {
                        let _ = room.client.execute(
                            "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                            vec![json!(matched.1.id), json!(if room.team_flip {2} else {3})]
                        ).await;
                    }
                } else {
                    let _ = room.client.execute(
                        "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(matched.1.id), json!(2 + picker_idx)]
                    ).await;
                }

                team.push(matched.1.clone());

                match teams
                    .iter()
                    .map(|t| t.len() >= room.room_parameters.team_size)
                    .reduce(|a, b| a && b)
                {
                    Some(true) => room.start_map_selection().await,
                    Some(false) => loop {
                        picker_idx = (picker_idx + 1) % room.room_parameters.team_num;

                        let Some(team) = teams.get(picker_idx) else {
                            return;
                        };
                        if team.len() >= room.room_parameters.team_size {
                            continue;
                        }
                        let Some(picker) = team.get(0) else {
                            return;
                        };

                        room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                            room.room_parameters.pick_time,
                        )));
                        room.warning_step = 0;
                        room.chat(format!("{}, pick a teammate.", picker.name))
                            .await;
                        break;
                    },
                    _ => (),
                }
            }

            GamePlayers::Teams { teams, picker_idx }
        }
        GamePlayers::FFA { in_game } => GamePlayers::FFA { in_game },
    }
}

pub async fn strike(room: &mut BonkRoom, id: i32) {
    if let State::MapSelection = room.state {
        let start = room
            .transition_timer
            .deadline()
            .checked_sub(Duration::from_secs(room.room_parameters.strike_time));
        if let Some(start) = start {
            if Instant::now().duration_since(start) == Duration::ZERO {
                return;
            }
        }

        let queue_clone = room.queue.clone();
        let Some((_, player)) = queue_clone.iter().find(|p| p.1.id == id) else {
            return;
        };
        if player.team == 0 {
            return;
        }

        let strikes_option = room.player_strikes.iter_mut().find(|p| p.0 == id);
        let strikes;
        if let Some(strikes_value) = strikes_option {
            strikes = strikes_value;
        } else {
            let strikes_value = (id, 0);
            room.player_strikes.push(strikes_value);
            let strikes_option = room.player_strikes.last_mut();
            let Some(strikes_value) = strikes_option else {
                return;
            };
            strikes = strikes_value;
        }

        if strikes.1 >= room.room_parameters.strike_num {
            room.chat("You've used all of you're strikes".to_string())
                .await;
        } else {
            strikes.1 += 1;
            let strikes = strikes.clone();

            for player in &mut room.queue {
                player.1.ready_cmd = false;
            }
            let _ = room
                .client
                .execute(
                    "sgrAPI.toolFunctions.networkEngine.allReadyReset();",
                    vec![],
                )
                .await;

            let remaining_maps = room
                .room_parameters
                .maps
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    let Some(map_strike) = room.map_strikes.get(*i) else {
                        return false;
                    };
                    !*map_strike
                })
                .collect::<Vec<(usize, &String)>>();
            let map_idx = rand::rng().random_range(0..remaining_maps.len());
            let Some(new_map) = remaining_maps.get(map_idx) else {
                return;
            };

            if let Some(map_strike) = room.map_strikes.get_mut(new_map.0) {
                *map_strike = true;
            }
            let _ = room
                .client
                .execute(
                    "sgrAPI.loadMap(JSON.parse(arguments[0]));",
                    vec![json!(new_map.1)],
                )
                .await;

            let all_strikes_used = room
                .queue
                .iter()
                .filter(|p| p.1.in_room && p.1.team != 0)
                .map(|p| {
                    let strikes = room
                        .player_strikes
                        .iter()
                        .find(|strike_player| strike_player.0 == p.1.id);
                    if let Some(strikes) = strikes {
                        return strikes.1;
                    } else {
                        return 0;
                    }
                })
                .map(|p| p >= room.room_parameters.strike_num)
                .reduce(|p1, p2| p1 && p2);

            if all_strikes_used == Some(true) {
                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.ready_time,
                )));
                room.warning_step = 0;
                room.state = State::Ready;
                room.chat("All strikes have been used. Use !r to start.".to_string())
                    .await;
            } else if remaining_maps.len() < 2 {
                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.ready_time,
                )));
                room.warning_step = 0;
                room.state = State::Ready;
                room.chat("All other maps have been struck. Use !r to start.".to_string())
                    .await;
            } else {
                //2 second double strike prevention.
                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.strike_time + 2,
                )));
                room.warning_step = 0;

                let remaining = room.room_parameters.strike_num - strikes.1;
                room.chat(format!(
                    "{} struck a map. They have {} strike{} remaining.",
                    player.name,
                    remaining,
                    if remaining == 1 { "" } else { "s" },
                ))
                .await;
            }
        }
    }
}

pub async fn ready(room: &mut BonkRoom, id: i32) {
    if room.state == State::MapSelection || room.state == State::Ready {
        let player = room.queue.iter_mut().find(|p| p.1.id == id);
        let Some(player) = player else { return };
        player.1.ready_cmd = true;
        room.check_ready(true).await;
    }
}

pub async fn reset(room: &mut BonkRoom, id: i32) {
    if room.state != State::GameStarting && room.state != State::InGame {
        return;
    }

    let in_game = room.get_in_game();
    let in_game = in_game
        .iter()
        .filter(|p| p.in_room)
        .collect::<Vec<&Player>>();
    let is_in_game = in_game.iter().find(|p| p.id == id).is_some();
    let is_in_vote = room.vote_reset.iter().find(|p| **p == id).is_some();

    if is_in_game && !is_in_vote {
        room.vote_reset.push(id);
    }

    if room.vote_reset.len() < in_game.len() {
        room.chat(format!(
            "{}/{} players voted for reset.",
            room.vote_reset.len(),
            in_game.len()
        ))
        .await;
    } else {
        if let Mode::Football = room.room_parameters.mode {
            let _ = room
                .client
                .execute(
                    "sgrAPI.nextScores = sgrAPI.footballState.scores;\
                    sgrAPI.startGame();",
                    vec![],
                )
                .await;
        } else {
            let _ = room
                .client
                .execute(
                    "sgrAPI.nextScores = sgrAPI.state.scores;\
                    sgrAPI.startGame();",
                    vec![],
                )
                .await;
        }
    }
}

pub async fn cancel(room: &mut BonkRoom, id: i32) {
    if room.state != State::GameStarting && room.state != State::InGame {
        return;
    }

    let in_game = room.get_in_game();
    let in_game = in_game
        .iter()
        .filter(|p| p.in_room)
        .collect::<Vec<&Player>>();
    let is_in_game = in_game.iter().find(|p| p.id == id).is_some();
    let is_in_vote = room.vote_cancel.iter().find(|p| **p == id).is_some();

    if is_in_game && !is_in_vote {
        room.vote_cancel.push(id);
    }

    if room.vote_cancel.len() < in_game.len() {
        room.chat(format!(
            "{}/{} players voted for cancelling the game.",
            room.vote_cancel.len(),
            in_game.len()
        ))
        .await;
    } else {
        room.reset().await;
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
