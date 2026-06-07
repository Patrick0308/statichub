CREATE TABLE device_login_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_code_hash TEXT NOT NULL UNIQUE,
    user_code TEXT NOT NULL UNIQUE,
    oauth_state TEXT UNIQUE,
    status TEXT NOT NULL CHECK(status IN ('pending', 'verified', 'approved', 'denied', 'expired', 'consumed')),
    token TEXT,
    poll_interval_seconds INTEGER NOT NULL DEFAULT 5,
    last_polled_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP
);

CREATE INDEX idx_device_login_sessions_user_code ON device_login_sessions(user_code);
CREATE INDEX idx_device_login_sessions_oauth_state ON device_login_sessions(oauth_state);
CREATE INDEX idx_device_login_sessions_expires_at ON device_login_sessions(expires_at);
