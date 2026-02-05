use rand::{seq::SliceRandom, Rng};
use serde::Deserialize;
use serde_json::{from_value, json};
use std::time::Duration;
use tokio::{sync::oneshot, time};

use crate::{
    bonk_bot::{
        bonk_commands,
        bonk_room::{GamePlayers, State},
        room_maker::{Mode, Queue},
    },
    leaderboard::LeaderboardMessage,
};

use super::bonk_room::{BonkRoom, Player};

pub async fn on_transition_timer_expired(room: &mut BonkRoom) {
    match room.state {
        State::Idle => transition_idle(room).await,
        State::Pick => transition_pick(room).await,
        State::MapSelection => {
            room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                room.room_parameters.ready_time,
            )));
            room.warning_step = 0;
            room.state = State::Ready;
            room.chat("The current map has been selected. Use !r to start.".to_string())
                .await;
        }
        State::Ready => {
            let not_ready = room
                .get_in_game()
                .drain(..)
                .filter(|p| !p.ready && !p.ready_cmd)
                .collect::<Vec<Player>>();

            for player in not_ready {
                let _ = room
                    .client
                    .execute(
                        "sgrAPI.toolFunctions.networkEngine.kickPlayer(arguments[0]);",
                        vec![json!(player.id)],
                    )
                    .await;

                let idx = room.queue.iter().position(|p| p.1.id == player.id);
                if let Some(idx) = idx {
                    room.queue.remove(idx);
                }
            }

            room.reset().await;
        }
        State::GameStarting => room.transition_timer = Box::pin(time::sleep(Duration::MAX)),
        State::InGame => on_game_end(room, None, false).await,
    }
}

async fn transition_idle(room: &mut BonkRoom) {
    match &mut room.room_parameters.queue {
        Queue::Singles => {
            let queue = room.get_queue_cloned();
            let picker;
            if queue.len() > 1 {
                picker = queue.first().clone();
            } else {
                picker = None;
            }

            if let Some(picker) = picker {
                if let Mode::Football = room.room_parameters.mode {
                    room.team_flip = rand::rng().random();
                    let _ = room.client.execute(
                        "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(picker.id), json!(if room.team_flip {3} else {2})]
                    ).await;
                } else {
                    let _ = room.client.execute(
                        "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                        vec![json!(picker.id), json!(1)]
                    ).await;
                }

                if queue.len() == 2 {
                    let mut picked = None;

                    if let Some(player) = queue.get(1) {
                        picked = Some(player.clone());
                        if let Mode::Football = room.room_parameters.mode {
                            let _ = room.client.execute(
                                "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                                vec![json!(player.id), json!(if room.team_flip {2} else {3})]
                            ).await;
                        } else {
                            let _ = room.client.execute(
                                "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                                vec![json!(player.id), json!(1)]
                            ).await;
                        }
                    }

                    room.game_players = GamePlayers::Singles {
                        picker: Some(picker.clone()),
                        picked,
                    };

                    room.start_map_selection().await;
                    return;
                }

                room.game_players = GamePlayers::Singles {
                    picker: Some(picker.clone()),
                    picked: None,
                };

                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.pick_time,
                )));
                room.warning_step = 0;
                room.state = State::Pick;
                room.chat(format!(
                    "{}, pick an opponent with !p <abbreviation>",
                    picker.name
                ))
                .await;
            } else {
                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.idle_time,
                )));
            }
        }
        Queue::Teams => {
            let queue = room.get_queue_cloned();
            let mut captains = vec![];
            let mut team_num = if let Mode::Football = room.room_parameters.mode {
                2
            } else {
                room.room_parameters.team_num
            };
            if team_num > 4 {
                team_num = 4;
            }
            room.room_parameters.team_num = team_num;
            if queue.len() >= room.room_parameters.team_size * room.room_parameters.team_num {
                for i in 0..room.room_parameters.team_num {
                    if let Some(player) = queue.get(i) {
                        captains.push(player.clone());
                    }
                }
            }

            if captains.len() == room.room_parameters.team_num {
                if let Mode::Football = room.room_parameters.mode {
                    room.team_flip = rand::rng().random();
                    if let Some(captain1) = captains.get(0) {
                        let _ = room.client.execute(
                            "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                            vec![json!(captain1.id), json!(if room.team_flip {3} else {2})]
                        ).await;
                    }
                    if let Some(captain2) = captains.get(1) {
                        let _ = room.client.execute(
                            "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                            vec![json!(captain2.id), json!(if room.team_flip {2} else {3})]
                        ).await;
                    }
                } else {
                    for (i, captain) in captains.iter().enumerate() {
                        let _ = room.client.execute(
                            "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                            vec![json!(captain.id), json!(2 + i)]
                        ).await;
                    }
                }
                room.game_players = GamePlayers::Teams {
                    teams: captains.iter().map(|p| vec![p.clone()]).collect(),
                    picker_idx: 0,
                };

                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.pick_time,
                )));
                room.warning_step = 0;
                room.state = State::Pick;
                if let Some(player) = captains.get(0) {
                    room.chat(format!(
                        "{}, pick a teammate with !p <abbreviation>",
                        player.name
                    ))
                    .await;
                }
            } else {
                room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                    room.room_parameters.idle_time,
                )));
            }
        }
        Queue::FFA => {
            if let Mode::Football = room.room_parameters.mode {
                room.room_parameters.queue = Queue::Singles;
                room.transition_timer = Box::pin(time::sleep(Duration::ZERO));
            } else {
                let queue = room.get_queue_cloned();
                if queue.len() >= room.room_parameters.ffa_min {
                    let mut in_game: Vec<Player> = vec![];
                    let player_num = room.room_parameters.ffa_max.min(queue.len());

                    for i in 0..player_num {
                        if let Some(player) = queue.get(i) {
                            in_game.push(player.clone());

                            let _ = room.client.execute(
                                "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);",
                                vec![json!(player.id), json!(1)]
                            ).await;
                        }
                    }

                    room.game_players = GamePlayers::FFA { in_game };

                    room.start_map_selection().await;
                } else {
                    room.transition_timer = Box::pin(time::sleep(Duration::from_secs(
                        room.room_parameters.idle_time,
                    )));
                }
            }
        }
    }

    let State::Idle = room.state else {
        let _ = room
            .client
            .execute(
                "sgrAPI.toolFunctions.networkEngine.sendStartCountdown(1);",
                vec![],
            )
            .await;
        return;
    };
}

async fn transition_pick(room: &mut BonkRoom) {
    match &room.game_players {
        GamePlayers::Singles { picker, picked: _ } => {
            let Some(picker) = picker else {
                room.reset().await;
                return;
            };
            let _ = room
                .client
                .execute(
                    "sgrAPI.toolFunctions.networkEngine.kickPlayer(arguments[0]);",
                    vec![json!(picker.id)],
                )
                .await;

            let idx = room.queue.iter().position(|p| p.1.id == picker.id);
            if let Some(idx) = idx {
                room.queue.remove(idx);
            }

            room.reset().await;
        }
        GamePlayers::Teams { teams, picker_idx } => {
            let Some(picker_team) = teams.get(*picker_idx) else {
                room.reset().await;
                return;
            };
            let Some(picker) = picker_team.get(0) else {
                room.reset().await;
                return;
            };
            let _ = room
                .client
                .execute(
                    "sgrAPI.toolFunctions.networkEngine.kickPlayer(arguments[0]);",
                    vec![json!(picker.id)],
                )
                .await;

            let idx = room.queue.iter().position(|p| p.1.id == picker.id);
            if let Some(idx) = idx {
                room.queue.remove(idx);
            }

            room.reset().await;
        }
        GamePlayers::FFA { in_game: _ } => {
            room.transition_timer = Box::pin(time::sleep(Duration::MAX))
        }
    }
}

pub async fn on_message(room: &mut BonkRoom, message: String) {
    if let Some(message) = message.strip_prefix("42[20,") {
        let num_string: String = message.chars().take_while(|c| c.is_digit(10)).collect();
        let message = &message[num_string.len()..];

        if let Some(id) = num_string.parse::<i32>().ok() {
            if id == 0 {
                return;
            }

            let message = message.strip_prefix(&num_string).unwrap_or(message);
            let message = message.strip_prefix(",\"").unwrap_or(message);
            let chat_message = message.strip_suffix("\"]").unwrap_or(message);

            if let Some(command) = chat_message.strip_prefix("!") {
                let mut command: Vec<&str> = command.split(' ').collect();

                let help_string = "!queue (lists the queue)".to_string();
                if command.len() == 0 {
                    return;
                }
                match command.remove(0) {
                    "help" | "h" | "?" => room.chat(help_string).await,
                    "ping" => room.chat("Pong!".to_string()).await,
                    "discord" | "d" => bonk_commands::discord(room).await,
                    "queue" | "q" => {
                        room.chat(format!(
                            "{}",
                            room.queue
                                .iter()
                                .filter(|p| p.1.in_room)
                                .map(|p| p.1.name.clone())
                                .collect::<Vec<String>>()
                                .join(", ")
                        ))
                        .await
                    }
                    "pick" | "p" => bonk_commands::pick(room, id, command.join(" ")).await,
                    "strike" | "s" => bonk_commands::strike(room, id).await,
                    "ready" | "r" => bonk_commands::ready(room, id).await,
                    "reset" | "re" => bonk_commands::reset(room, id).await,
                    "cancel" | "c" => bonk_commands::cancel(room, id).await,
                    _ => room.chat(help_string).await,
                }
            }
        }
    }
}

pub async fn on_player_join(room: &mut BonkRoom, player: Player) {
    match &mut room.game_players {
        GamePlayers::Singles { picker, picked } => {
            let mut game_player = None;
            'gp: {
                let Some(picker) = picker else { break 'gp };
                if player.name == picker.name {
                    game_player = Some(picker)
                }
                let Some(picked) = picked else { break 'gp };
                if player.name == picked.name {
                    game_player = Some(picked)
                }
            }
            if let Some(game_player) = game_player {
                game_player.id = player.id;
            }
        }
        GamePlayers::Teams {
            teams,
            picker_idx: _,
        } => {
            for team in teams {
                for p in team {
                    if p.name == player.name {
                        p.id = player.id;
                    }
                }
            }
        }
        GamePlayers::FFA { in_game } => {
            for p in in_game {
                if p.name == player.name {
                    p.id = player.id;
                }
            }
        }
    }

    match room.state {
        State::Idle => match room.room_parameters.queue {
            Queue::Singles => {
                if room.get_queue_mut().len() == 2 {
                    room.transition_timer = Box::pin(time::sleep(Duration::ZERO));
                }
            }
            Queue::Teams => {
                if room.get_queue_mut().len()
                    == room.room_parameters.team_size * room.room_parameters.team_num
                {
                    room.transition_timer = Box::pin(time::sleep(Duration::ZERO));
                }
            }
            Queue::FFA => {
                if room.get_queue_mut().len() == room.room_parameters.ffa_min {
                    room.transition_timer = Box::pin(time::sleep(Duration::ZERO));
                }
            }
        },
        State::Pick | State::MapSelection | State::Ready => (),
        State::GameStarting | State::InGame => match &room.game_players {
            GamePlayers::Singles {
                picker: _,
                picked: _,
            } => (),
            GamePlayers::Teams {
                teams,
                picker_idx: _,
            } => {
                for (i, team) in teams.iter().enumerate() {
                    for p in team {
                        if player.id == p.id {
                            let _ = room.client.execute(
                                    "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);\
                                    sgrAPI.stateFunctions.hostHandlePlayerJoined(arguments[0], sgrAPI.players.length, arguments[1]);",
                                    vec![json!(player.id), json!(i + 2)]
                                ).await;
                        }
                    }
                }
            }
            GamePlayers::FFA { in_game } => {
                for p in in_game {
                    if player.id == p.id {
                        let _ = room.client.execute(
                                "sgrAPI.toolFunctions.networkEngine.changeOtherTeam(arguments[0], arguments[1]);\
                                sgrAPI.stateFunctions.hostHandlePlayerJoined(arguments[0], sgrAPI.players.length, arguments[1]);",
                                vec![json!(player.id), json!(1)]
                            ).await;
                    }
                }
            }
        },
    }
}

pub async fn on_player_leave(room: &mut BonkRoom, player: Player) {
    match room.state {
        State::Idle => (),
        State::Pick => match &mut room.game_players {
            GamePlayers::Singles { picker, picked: _ } => {
                let Some(picker) = picker else {
                    return;
                };

                if player.id == picker.id {
                    room.reset().await;
                }
            }
            GamePlayers::Teams {
                teams,
                picker_idx: _,
            } => {
                let mut reset = false;
                for team in teams {
                    for i in 0..team.len() {
                        let Some(team_player) = team.get(i) else {
                            return;
                        };

                        if player.id == team_player.id {
                            if i == 0 {
                                reset = true;
                                break;
                            } else {
                                team.remove(i);
                                break;
                            }
                        }
                    }
                }
                if reset {
                    room.reset().await;
                }
            }
            GamePlayers::FFA { in_game: _ } => (),
        },
        State::MapSelection | State::Ready => {
            if player.team != 0 {
                let player_idx = room.queue.iter().position(|p| p.1.id == player.id);
                if let Some(player_idx) = player_idx {
                    let queue_spot = room.queue.remove(player_idx);
                    room.queue.push(queue_spot);
                }
                room.reset().await;
            }
        }
        State::GameStarting | State::InGame => {
            let game_players = room.game_players.clone();
            match game_players {
                GamePlayers::Singles { picker, picked } => {
                    if let Mode::Football = room.room_parameters.mode {
                        if let Some(picker) = picker {
                            if picker.id == player.id {
                                on_game_end(room, Some(if room.team_flip { 2 } else { 3 }), false)
                                    .await;
                            }
                        }
                        if let Some(picked) = picked {
                            if picked.id == player.id {
                                on_game_end(room, Some(if room.team_flip { 3 } else { 2 }), false)
                                    .await;
                            }
                        }
                    } else {
                        let Some(picker) = picker else { return };
                        let Some(picked) = picked else { return };
                        if picker.id == player.id {
                            on_game_end(room, Some(picked.id as usize), false).await;
                        } else if picked.id == player.id {
                            on_game_end(room, Some(picker.id as usize), false).await;
                        }
                    }
                }
                GamePlayers::Teams {
                    teams,
                    picker_idx: _,
                } => {
                    let remaining_teams = teams
                        .iter()
                        .enumerate()
                        .map(|(i, team)| {
                            (
                                i,
                                team.iter()
                                    .filter_map(|p| room.queue.iter().find(|qp| qp.1.id == p.id))
                                    .map(|p| !p.1.in_room)
                                    .reduce(|x1, x2| x1 && x2)
                                    .unwrap_or(true),
                            )
                        })
                        .filter(|(_, not_in_room)| !not_in_room)
                        .map(|(i, _)| i)
                        .collect::<Vec<usize>>();

                    if remaining_teams.len() == 1 {
                        let Some(remaining_team) = remaining_teams.get(0) else {
                            return;
                        };
                        on_game_end(room, Some(*remaining_team), false).await;
                    } else if remaining_teams.len() == 0 {
                        room.reset().await;
                    }
                }
                GamePlayers::FFA { in_game } => {
                    let remaining_players = in_game
                        .iter()
                        .filter_map(|p| room.queue.iter().find(|qp| qp.1.id == p.id))
                        .map(|p| p.1.clone())
                        .filter(|p| p.in_room)
                        .collect::<Vec<Player>>();

                    if remaining_players.len() == 1 {
                        let Some(remaining_player) = remaining_players.get(0) else {
                            return;
                        };
                        on_game_end(room, Some(remaining_player.id as usize), false).await;
                    } else if remaining_players.len() == 0 {
                        room.reset().await;
                    }
                }
            }
        }
    }
}

///Winner and tie are only specified if the game ends prematurely and
///the remaining team/player automatically wins or there's an all-way tie.
pub async fn on_game_end(room: &mut BonkRoom, mut winner: Option<usize>, tie: bool) {
    #[derive(Deserialize, Debug)]
    struct Score {
        id: i32,
        score: i32,
    }

    match &room.game_players {
        GamePlayers::Singles { picker, picked } => {
            let Some(picked) = picked else {
                return;
            };
            let picked_idx = room.queue.iter().position(|p| p.1.id == picked.id);
            if let Some(picked_idx) = picked_idx {
                let queue_spot = room.queue.remove(picked_idx);
                room.queue.push(queue_spot);
            }

            let Some(picker) = picker else {
                return;
            };
            let picker_idx = room.queue.iter().position(|p| p.1.id == picker.id);
            if let Some(picker_idx) = picker_idx {
                let queue_spot = room.queue.remove(picker_idx);
                room.queue.push(queue_spot);
            }

            if let Some(leaderboard_tx) = &room.leaderboard_tx {
                if tie {
                    let (match_string_tx, match_string_rx) = oneshot::channel();
                    let _ = leaderboard_tx
                        .send(LeaderboardMessage::Update {
                            teams: vec![vec![picker.name.clone()], vec![picked.name.clone()]],
                            ties: vec![true],
                            match_str: match_string_tx,
                        })
                        .await;
                    if let Ok(Ok(match_string)) = match_string_rx.await {
                        room.chat(match_string).await;
                    }
                } else {
                    if let Mode::Football = room.room_parameters.mode {
                        if !winner.is_some() {
                            let value = room
                                .client
                                .execute(
                                    "\
                                if(sgrAPI.footballState.scores[3] > sgrAPI.footballState.scores[2]) return 3;\
                                if(sgrAPI.footballState.scores[2] < sgrAPI.footballState.scores[3]) return 2;\
                                return 0;\
                                ",
                                    vec![],
                                )
                                .await;
                            if let Ok(value) = value {
                                if let Ok(value) = from_value::<usize>(value) {
                                    winner = Some(value);
                                }
                            }
                        }
                        if let Some(winner) = winner {
                            let (match_string_tx, match_string_rx) = oneshot::channel();
                            let _ = leaderboard_tx
                                .send(LeaderboardMessage::Update {
                                    teams: vec![
                                        vec![if (winner == 2 || winner == 0) ^ room.team_flip {
                                            picker.name.clone()
                                        } else {
                                            picked.name.clone()
                                        }],
                                        vec![if (winner == 3) ^ room.team_flip {
                                            picker.name.clone()
                                        } else {
                                            picked.name.clone()
                                        }],
                                    ],
                                    ties: vec![winner == 0],
                                    match_str: match_string_tx,
                                })
                                .await;
                            if let Ok(Ok(match_string)) = match_string_rx.await {
                                room.chat(match_string).await;
                            }
                        }
                    } else {
                        let scores = room
                            .client
                            .execute(
                                "\
                                return Object.keys(sgrAPI.state.scores)\
                                    .map(id => {\
                                        let score = sgrAPI.state.scores[id];\
                                        if(score === null) return undefined;\
                                        return {id: Number(id), score};\
                                    }).filter(x => x !== undefined);\
                            ",
                                vec![],
                            )
                            .await;
                        if let Ok(scores) = scores {
                            if let Ok(scores) = from_value::<Vec<Score>>(scores) {
                                if !winner.is_some() {
                                    'winner: {
                                        if winner != None {
                                            break 'winner;
                                        };
                                        let Some(p1) = scores.get(0) else {
                                            break 'winner;
                                        };
                                        let Some(p2) = scores.get(1) else {
                                            break 'winner;
                                        };

                                        if p1.score > p2.score {
                                            winner = Some(p1.id as usize);
                                        } else if p2.score > p1.score {
                                            winner = Some(p2.id as usize)
                                        } else {
                                            winner = Some(0);
                                        }
                                    }
                                }
                                if let Some(winner) = winner {
                                    let picker_lostnt = winner as i32 == picker.id || winner == 0;
                                    let (match_string_tx, match_string_rx) = oneshot::channel();
                                    let _ = leaderboard_tx
                                        .send(LeaderboardMessage::Update {
                                            teams: vec![
                                                vec![if picker_lostnt {
                                                    picker.name.clone()
                                                } else {
                                                    picked.name.clone()
                                                }],
                                                vec![if !picker_lostnt {
                                                    picker.name.clone()
                                                } else {
                                                    picked.name.clone()
                                                }],
                                            ],
                                            ties: vec![winner == 0],
                                            match_str: match_string_tx,
                                        })
                                        .await;
                                    if let Ok(Ok(match_string)) = match_string_rx.await {
                                        room.chat(match_string).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        GamePlayers::Teams {
            teams,
            picker_idx: _,
        } => {
            let mut captains = teams
                .iter()
                .filter_map(|team| team.get(0))
                .collect::<Vec<&Player>>();
            captains.shuffle(&mut rand::rng());

            let mut others = teams
                .iter()
                .map(|team| {
                    team.iter()
                        .enumerate()
                        .filter(|(i, _)| *i != 0)
                        .map(|(_, player)| player)
                })
                .flatten()
                .collect::<Vec<&Player>>();
            others.shuffle(&mut rand::rng());

            for player in others {
                let idx = room.queue.iter().position(|p| p.1.id == player.id);
                if let Some(idx) = idx {
                    let queue_spot = room.queue.remove(idx);
                    room.queue.push(queue_spot);
                }
            }

            for player in captains {
                let idx = room.queue.iter().position(|p| p.1.id == player.id);
                if let Some(idx) = idx {
                    let queue_spot = room.queue.remove(idx);
                    room.queue.push(queue_spot);
                }
            }

            'lb: {
                let Some(leaderboard_tx) = &room.leaderboard_tx else {
                    break 'lb;
                };

                let scores;
                if let Mode::Football = room.room_parameters.mode {
                    scores = room
                        .client
                        .execute(
                            "return sgrAPI.footballState.scores.filter(x => x !== null);",
                            vec![],
                        )
                        .await;
                } else {
                    scores = room
                        .client
                        .execute(
                            "return sgrAPI.state.scores.filter(x => x !== null);",
                            vec![],
                        )
                        .await;
                }
                let Ok(scores) = scores else { break 'lb };
                let Ok(mut scores) = from_value::<Vec<i32>>(scores) else {
                    break 'lb;
                };

                if let Some(winner) = winner {
                    if let Some(score) = scores.get_mut(winner) {
                        *score = room.room_parameters.rounds;
                    }
                }

                let mut placements = teams
                    .iter()
                    .enumerate()
                    .map(|(i, team)| {
                        (
                            scores.get(i),
                            team.iter().map(|p| p.name.clone()).collect::<Vec<String>>(),
                        )
                    })
                    .collect::<Vec<(Option<&i32>, Vec<String>)>>();
                placements.sort_by(|team1, team2| team2.0.cmp(&team1.0));
                let mut ties;
                if tie {
                    ties = vec![true; placements.len() - 1];
                } else {
                    ties = vec![];
                    for i in 0..placements.len() - 1 {
                        let Some(team1) = placements.get(i) else {
                            ties.push(false);
                            continue;
                        };
                        let Some(team2) = placements.get(i + 1) else {
                            ties.push(false);
                            continue;
                        };
                        ties.push(team1.0 == team2.0);
                    }
                }
                let team_strings = placements
                    .iter()
                    .map(|team| team.1.clone())
                    .collect::<Vec<Vec<String>>>();

                let (match_string_tx, match_string_rx) = oneshot::channel();
                let _ = leaderboard_tx
                    .send(LeaderboardMessage::Update {
                        teams: team_strings,
                        ties,
                        match_str: match_string_tx,
                    })
                    .await;
                if let Ok(Ok(match_string)) = match_string_rx.await {
                    room.chat(match_string).await;
                }
            }
        }
        GamePlayers::FFA { in_game } => {
            for player in in_game {
                let idx = room.queue.iter().position(|p| p.1.id == player.id);
                if let Some(idx) = idx {
                    let queue_spot = room.queue.remove(idx);
                    room.queue.push(queue_spot);
                }
            }

            'lb: {
                let Some(leaderboard_tx) = &room.leaderboard_tx else {
                    break 'lb;
                };

                let scores = room
                    .client
                    .execute(
                        "\
                        return Object.keys(sgrAPI.state.scores)\
                            .map(id => {\
                                let score = sgrAPI.state.scores[id];\
                                if(score === null) return undefined;\
                                return {id: Number(id), score};\
                            }).filter(x => x !== undefined);\
                    ",
                        vec![json!(room.room_parameters.rounds)],
                    )
                    .await;
                let Ok(scores) = scores else { break 'lb };
                let Ok(mut scores) = from_value::<Vec<Score>>(scores) else {
                    break 'lb;
                };

                if let Some(winner) = winner {
                    if let Some(score) = scores.iter_mut().find(|score| score.id == winner as i32) {
                        score.score = room.room_parameters.rounds;
                    }
                }

                let mut placements = in_game
                    .iter()
                    .map(|player| {
                        (
                            scores.iter().find(|score| score.id == player.id),
                            vec![player.name.clone()],
                        )
                    })
                    .map(|placement| {
                        if let Some(score) = placement.0 {
                            return (score.score, placement.1);
                        } else {
                            return (0, placement.1);
                        }
                    })
                    .collect::<Vec<(i32, Vec<String>)>>();
                placements.sort_by(|p1, p2| p2.0.cmp(&p1.0));
                let mut ties;
                if tie {
                    ties = vec![true; placements.len() - 1];
                } else {
                    ties = vec![];
                    for i in 0..placements.len() - 1 {
                        let Some(team1) = placements.get(i) else {
                            ties.push(false);
                            continue;
                        };
                        let Some(team2) = placements.get(i + 1) else {
                            ties.push(false);
                            continue;
                        };
                        ties.push(team1.0 == team2.0);
                    }
                }
                let team_strings = placements
                    .iter()
                    .map(|player| player.1.clone())
                    .collect::<Vec<Vec<String>>>();

                let (match_string_tx, match_string_rx) = oneshot::channel();
                let _ = leaderboard_tx
                    .send(LeaderboardMessage::Update {
                        teams: team_strings,
                        ties,
                        match_str: match_string_tx,
                    })
                    .await;
                if let Ok(Ok(match_string)) = match_string_rx.await {
                    room.chat(match_string).await;
                }
            }
        }
    }

    room.reset().await;
}
