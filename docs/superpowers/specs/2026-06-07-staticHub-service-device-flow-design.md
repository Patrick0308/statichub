# StaticHub Service Device Flow Design

Date: 2026-06-07
Status: Draft for review

## 1. Background and Goals

StaticHub currently supports CLI login by having `statichub login` create a random session id, ask the server for a Google OAuth authorization URL, open or print that URL, and poll `/auth/status/:session_id` until the server stores a JWT in memory. This works on a local desktop, but it is awkward on SSH hosts and exposes an imprecise boundary: the same session id is used as OAuth state and as the polling lookup key for retrieving the resulting StaticHub JWT.

This design replaces the CLI login UX with a StaticHub-owned device flow. The server still uses Google OAuth as the identity provider, but StaticHub becomes the login broker: the CLI displays a short user code and a verification URL, the user completes Google login in any browser, and the CLI polls with a separate high-entropy device code until the StaticHub JWT is ready.

Goals:
- Improve `statichub login` for SSH and remote terminal use.
- Keep Google OAuth as the underlying identity provider.
- Separate human-entered user codes from machine polling secrets.
- Persist login sessions so server restarts do not orphan active login attempts.
- Return the final JWT only once.
- Keep API key behavior unchanged.

Non-goals:
- Implement Google's OAuth device authorization grant.
- Replace Google OAuth with password or email login.
- Add scoped permissions, refresh tokens, or account management.
- Change existing API key authentication behavior.

## 2. Requirements

### 2.1 Functional requirements

- `statichub login` starts a StaticHub device login session.
- CLI output shows:
  - A verification URL, such as `https://statichub.example.com/auth/device`.
  - A short user code, such as `ABCD-EFGH`.
  - A fallback direct verification URL that includes the code.
- The user can open the verification page on any browser-capable device.
- The verification page accepts the user code, validates it, and starts Google OAuth.
- Google OAuth callback completes the StaticHub session by associating the authenticated user with the pending device login.
- CLI polling returns clear pending, approved, denied, expired, and slow-down states.
- On approval, CLI saves the returned StaticHub JWT using the existing credentials file path.
- The approved JWT is returned once; later polling for the same device code must not return the token again.

### 2.2 Security requirements

- `device_code` must be high entropy and never shown as the human code.
- `user_code` must be short-lived and safe to display.
- `user_code` lookup must not expose the final JWT.
- Polling must use `device_code`, not `user_code`.
- OAuth callback must validate server-generated state before approving a device session.
- Sessions must expire automatically.
- The server must avoid logging full `device_code`, `user_code`, Google access tokens, or StaticHub JWTs.
- Polling must enforce the advertised interval or return `slow_down`.

## 3. High-Level Flow

### 3.1 CLI session creation

`statichub login` calls:

```text
POST /auth/device
```

The server creates a device login session and returns:

```json
{
  "device_code": "high-entropy-secret",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://statichub.example.com/auth/device",
  "verification_uri_complete": "https://statichub.example.com/auth/device?code=ABCD-EFGH",
  "expires_in": 600,
  "interval": 5
}
```

The CLI prints the user-facing URL and code, then polls with `device_code`.

### 3.2 Browser verification

The user opens `/auth/device`. If the URL includes `?code=...`, the page pre-fills the code. After submission, the server validates that the code exists, is pending, and has not expired. The server then redirects the browser to Google OAuth with an opaque StaticHub OAuth state.

### 3.3 Google callback

Google redirects to the existing callback route. The server exchanges the Google authorization code, fetches user info, creates or finds the StaticHub user, generates the existing StaticHub JWT, and stores it on the matching device session as an approved, unconsumed token result.

### 3.4 CLI polling

The CLI calls:

```text
POST /auth/device/token
```

with:

```json
{
  "device_code": "high-entropy-secret"
}
```

Responses:
- Pending: `authorization_pending`
- Polling too fast: `slow_down`
- Expired: `expired_token`
- Denied: `access_denied`
- Approved: returns the StaticHub JWT and marks the session consumed

## 4. API Contract

### 4.1 Create device session

`POST /auth/device`

Response status: `200 OK`

```json
{
  "device_code": "sdc_...",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://statichub.example.com/auth/device",
  "verification_uri_complete": "https://statichub.example.com/auth/device?code=ABCD-EFGH",
  "expires_in": 600,
  "interval": 5
}
```

### 4.2 Verification page

`GET /auth/device`

Returns a small HTML page with a code input. This page is intentionally server-rendered and dependency-free.

### 4.3 Submit user code

`POST /auth/device/verify`

Request:

```json
{
  "user_code": "ABCD-EFGH"
}
```

Response:
- `302 Found` to Google OAuth when the code is valid.
- `400 Bad Request` for malformed code.
- `404 Not Found` for unknown code.
- `410 Gone` for expired code.
- `409 Conflict` for already approved or consumed sessions.

### 4.4 Poll device token

`POST /auth/device/token`

Request:

```json
{
  "device_code": "sdc_..."
}
```

Pending response:

```json
{
  "status": "authorization_pending",
  "interval": 5
}
```

Slow-down response:

```json
{
  "status": "slow_down",
  "interval": 10
}
```

Approved response:

```json
{
  "status": "approved",
  "token": "staticHub-jwt"
}
```

Expired response:

```json
{
  "status": "expired_token"
}
```

Denied response:

```json
{
  "status": "access_denied"
}
```

## 5. Data Model

Replace or repurpose the existing unused `oauth_sessions` table into a device login sessions table.

Recommended table:

```sql
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
```

Notes:
- Store a hash of `device_code`, not the plaintext value.
- Store `user_code` plaintext because it must be looked up from browser input. It is short-lived and not sufficient to retrieve the JWT.
- Store the JWT only after Google OAuth succeeds and delete or null it after consumption.

## 6. CLI Behavior

`statichub login` should become:

```text
Logging in to StaticHub...

Open this URL:
  https://statichub.example.com/auth/device

Enter this code:
  ABCD-EFGH

Or open:
  https://statichub.example.com/auth/device?code=ABCD-EFGH

Waiting for authentication...
```

If `open::that` is available, the CLI may open `verification_uri_complete`. The command should still be excellent when auto-open fails or when running over SSH.

Polling rules:
- Start with the server-provided interval.
- If the server returns `slow_down`, use the returned interval.
- Stop on approval, denial, expiration, network failure, or local timeout.
- Save credentials through the existing `auth::save_credentials`.

## 7. Compatibility and Migration

Compatibility:
- Existing saved credentials remain valid until their JWT expiry.
- `STATICHUB_API_KEY` behavior remains unchanged.
- Authenticated project and API key endpoints continue accepting the existing JWT format.

Migration:
- Add a migration for `device_login_sessions`.
- The old `oauth_sessions` table can remain unused for one release, then be removed in a later cleanup, or be dropped now if there are no deployed databases relying on it.
- Existing `/auth/login/google` and `/auth/status/:session_id` can be kept temporarily for compatibility or replaced in one release if no external clients depend on them.

## 8. Testing Strategy

Server tests:
- Creating a device session returns a unique `device_code`, unique `user_code`, complete verification URL, expiry, and interval.
- Polling a new session returns `authorization_pending`.
- Polling faster than allowed returns `slow_down`.
- Submitting an invalid, unknown, expired, approved, or consumed user code returns the expected status.
- Valid user code submission stores an OAuth state and redirects to Google.
- Callback with valid state approves the matching device session.
- Approved polling returns the JWT once and marks the session consumed.
- Subsequent polling does not return the JWT.

CLI tests:
- Login prints verification URL and user code.
- Login polls until approved and saves credentials.
- Login handles pending, slow-down, denied, expired, and timeout responses.

Regression tests:
- API key auth still works for project operations.
- API key management remains JWT-only.
- Auth-disabled local mode keeps returning 503 for auth and protected APIs.

## 9. Risks and Mitigations

Risk: Device flow expands auth surface area.
Mitigation: Keep endpoints small, explicit, and covered by focused integration tests.

Risk: Storing JWT in the database creates a short-lived secret at rest.
Mitigation: Store only until first successful poll, expire quickly, and avoid logging token fields.

Risk: User code guessing.
Mitigation: Use enough entropy for the short validity window, rate-limit verification attempts, and ensure user code alone cannot retrieve a token.

Risk: Breaking current login behavior.
Mitigation: Keep the old login endpoints during rollout or add compatibility tests if replacing them immediately.

## 10. Recommendation

Make StaticHub service device flow the default `statichub login` experience. Keep Google OAuth underneath it and keep API keys as the long-lived automation credential. This gives the CLI a more mature remote-login UX while also cleaning up the current polling security boundary.
