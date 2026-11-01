-- The cookie carries a random token; only its SHA-256 is stored, so a leaked
-- database or backup can't be replayed as live session cookies.
CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Same UTC text format as poll_weeks.closes_at so comparisons against
    -- strftime('%Y-%m-%dT%H:%M:%SZ', 'now') sort correctly as text.
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        CHECK (created_at = strftime('%Y-%m-%dT%H:%M:%SZ', created_at)),
    expires_at TEXT NOT NULL
        CHECK (expires_at = strftime('%Y-%m-%dT%H:%M:%SZ', expires_at))
) WITHOUT ROWID, STRICT;

CREATE INDEX sessions_user_id ON sessions (user_id);
CREATE INDEX sessions_expires_at ON sessions (expires_at);
