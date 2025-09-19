ALTER TABLE leaderboard
    ADD COLUMN channel BIGINT NOT NULL,
    ADD COLUMN messages BIGINT[] NOT NULL;
ALTER TABLE lb_players
    ADD COLUMN display_rating DOUBLE PRECISION NOT NULL,
    ADD COLUMN last_updated DATE NOT NULL;
ALTER TABLE lb_game_teams
    ADD COLUMN player_ids INTEGER[] NOT NULL,
    ADD COLUMN old_rating DOUBLE PRECISION[] NOT NULL,
    ADD COLUMN new_rating DOUBLE PRECISION[] NOT NULL;

DROP TABLE lb_players_whr;
DROP TABLE lb_player_days_whr;
DROP TABLE lb_game_players;
