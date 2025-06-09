CREATE TABLE lb_settings (
    lb_id INTEGER REFERENCES leaderboard ON DELETE CASCADE,
    parameter TEXT,
    data JSONB NOT NULL,
    PRIMARY KEY (lb_id, parameter)
);

ALTER TABLE leaderboard
DROP COLUMN settings,
DROP COLUMN name,
DROP COLUMN abbreviation;
