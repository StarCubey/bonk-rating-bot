CREATE TABLE players (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE leaderboard (
    id SERIAL PRIMARY KEY
);

CREATE TABLE lb_settings (
    lb_id INTEGER REFERENCES leaderboard ON DELETE CASCADE,
    parameter TEXT,
    data JSONB NOT NULL,
    PRIMARY KEY (lb_id, parameter)
);

CREATE TABLE lb_seasons (
    lb_id INTEGER REFERENCES leaderboard ON DELETE CASCADE,
    season_num INTEGER,
    start DATE NOT NULL,
    hard_reset BOOLEAN NOT NULL,
    PRIMARY KEY (lb_id, season_num)
);

CREATE TABLE lb_players (
    id SERIAL PRIMARY KEY,
    lb_id INTEGER NOT NULL REFERENCES leaderboard ON DELETE CASCADE,
    player_id INTEGER NOT NULL REFERENCES players ON DELETE CASCADE,
    rating DOUBLE PRECISION NOT NULL,
    rating_deviation DOUBLE PRECISION NOT NULL
);

CREATE TABLE lb_players_whr (
    lb_player_id INTEGER PRIMARY KEY REFERENCES lb_players ON DELETE CASCADE,
    late_prior_variance DOUBLE PRECISION NOT NULL
);

CREATE TABLE lb_player_days_whr (
    lb_player_id INTEGER REFERENCES lb_players ON DELETE CASCADE,
    day DATE,
    rating DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (lb_player_id, day)
);

CREATE TABLE lb_games (
    id SERIAL PRIMARY KEY,
    lb_id INTEGER NOT NULL REFERENCES leaderboard ON DELETE CASCADE,
    season_num INTEGER NOT NULL,
    day DATE NOT NULL
);

CREATE TABLE lb_game_teams (
    game_id INTEGER REFERENCES lb_games ON DELETE CASCADE,
    team INTEGER,
    score DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (game_id, team)
);

CREATE TABLE lb_game_players (
    game_id INTEGER REFERENCES lb_games ON DELETE CASCADE,
    player_id INTEGER REFERENCES lb_players ON DELETE CASCADE,
    team INTEGER NOT NULL,
    old_rating DOUBLE PRECISION NOT NULL,
    new_rating DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (game_id, player_id)
);
