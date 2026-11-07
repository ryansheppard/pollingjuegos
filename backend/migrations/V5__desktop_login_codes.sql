-- Short-lived codes handed to a desktop app over its loopback redirect and
-- exchanged for a bearer session. Hashed like sessions.token_hash; `challenge`
-- is the app's PKCE S256 challenge, so a code intercepted on the loopback
-- port is useless without the verifier only the app holds.
CREATE TABLE desktop_login_codes (
    code_hash BLOB PRIMARY KEY CHECK (length(code_hash) = 32),
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    challenge TEXT NOT NULL,
    expires_at TEXT NOT NULL
        CHECK (expires_at = strftime('%Y-%m-%dT%H:%M:%SZ', expires_at))
) WITHOUT ROWID, STRICT;

CREATE INDEX desktop_login_codes_expires_at ON desktop_login_codes (expires_at);
