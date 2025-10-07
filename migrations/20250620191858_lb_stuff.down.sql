CREATE TABLE players (
    id SERIAL PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

ALTER TABLE leaderboard
    ALTER COLUMN id TYPE INTEGER,
    DROP COLUMN channel,
    DROP COLUMN messages;
ALTER TABLE lb_games
    ALTER COLUMN id TYPE INTEGER,
    ALTER COLUMN lb_id TYPE INTEGER,
    DROP COLUMN score,
    DROP COLUMN ties;
ALTER TABLE lb_players
    ALTER COLUMN id TYPE INTEGER,
    ALTER COLUMN lb_id TYPE INTEGER,
    ADD COLUMN player_id INTEGER NOT NULL REFERENCES players ON DELETE CASCADE,
    DROP COLUMN name,
    DROP COLUMN display_rating,
    DROP COLUMN last_updated;
ALTER TABLE lb_game_teams
    ALTER COLUMN game_id TYPE INTEGER,
    ADD COLUMN score DOUBLE PRECISION NOT NULL,
    DROP COLUMN player_ids,
    DROP COLUMN old_rating,
    DROP COLUMN new_rating;
ALTER TABLE lb_seasons
    ALTER COLUMN lb_id TYPE INTEGER;

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

CREATE TABLE lb_game_players (
    game_id INTEGER REFERENCES lb_games ON DELETE CASCADE,
    player_id INTEGER REFERENCES lb_players ON DELETE CASCADE,
    team INTEGER NOT NULL,
    old_rating DOUBLE PRECISION NOT NULL,
    new_rating DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (game_id, player_id)
);
