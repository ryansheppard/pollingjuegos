-- Foreign keys are only enforced when each connection runs
-- `PRAGMA foreign_keys = ON`; the pragma is a no-op inside refinery's transaction.

CREATE TABLE teams (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 100),
    abbreviation TEXT NOT NULL UNIQUE
        CHECK (length(abbreviation) BETWEEN 2 AND 3 AND abbreviation = upper(abbreviation))
) STRICT;

CREATE TABLE quarterbacks (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    -- Current team only; existing votes stay tied to the QB after a trade.
    team_id INTEGER REFERENCES teams (id) ON DELETE SET NULL
) STRICT;

CREATE INDEX quarterbacks_team_id ON quarterbacks (team_id);

-- Holds the Discord snowflake ID, not the mutable display name.
CREATE TABLE approved_discord_users (
    id INTEGER PRIMARY KEY,
    discord_id TEXT NOT NULL UNIQUE
        CHECK (length(discord_id) BETWEEN 1 AND 20 AND discord_id NOT GLOB '*[^0-9]*'),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 100)
) STRICT;

-- Deliberately not a foreign key to approved_discord_users: revoking someone
-- (disabling or deleting the allowlist row) must not touch their ballots.
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    discord_id TEXT NOT NULL UNIQUE
        CHECK (length(discord_id) BETWEEN 1 AND 20 AND discord_id NOT GLOB '*[^0-9]*'),
    username TEXT NOT NULL
) STRICT;

CREATE TABLE poll_weeks (
    id INTEGER PRIMARY KEY,
    season INTEGER NOT NULL CHECK (season >= 1),
    week INTEGER NOT NULL CHECK (week >= 1),
    -- UTC as 'YYYY-MM-DDTHH:MM:SSZ'; the check stops hand-written SQL from storing
    -- a format that would compare wrongly as text. NULL means no deadline.
    closes_at TEXT CHECK (closes_at IS NULL OR closes_at = strftime('%Y-%m-%dT%H:%M:%SZ', closes_at)),
    UNIQUE (season, week)
) STRICT;

CREATE TABLE ballots (
    id INTEGER PRIMARY KEY,
    poll_week_id INTEGER NOT NULL REFERENCES poll_weeks (id) ON DELETE CASCADE,
    voter_id INTEGER NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    UNIQUE (poll_week_id, voter_id)
) STRICT;

CREATE INDEX ballots_voter_id ON ballots (voter_id);

CREATE TABLE votes (
    id INTEGER PRIMARY KEY,
    ballot_id INTEGER NOT NULL REFERENCES ballots (id) ON DELETE CASCADE,
    quarterback_id INTEGER NOT NULL REFERENCES quarterbacks (id) ON DELETE RESTRICT,
    rank INTEGER NOT NULL CHECK (rank >= 1),
    UNIQUE (ballot_id, rank),
    UNIQUE (ballot_id, quarterback_id)
) STRICT;

CREATE INDEX votes_quarterback_id ON votes (quarterback_id);
