# API Key Management Design (StaticHub)

Date: 2026-05-25
Status: Draft for review

## 1. Background and Goals

StaticHub currently uses Google OAuth login and short-lived JWT for authenticated operations.
JWT expires (currently 7 days), which is inconvenient for CI and long-running automation.

This design adds user-managed API Keys so CLI users can set `STATICHUB_API_KEY` and operate without repeated `statichub login`.

Goals:
- Allow persistent auth for automation via `STATICHUB_API_KEY`.
- Keep existing OAuth/JWT flow fully compatible.
- Require interactive login (JWT) for API key management actions.
- Keep scope minimal: full-permission keys, no expiration, create/list/revoke only.

Non-goals (for this iteration):
- Scoped permissions per key.
- Per-key expiration policies.
- Rotate/rename endpoints.

## 2. Requirements

### 2.1 Functional requirements

- `deploy/list/info/rollback` must accept either JWT or API Key.
- CLI must prefer `STATICHUB_API_KEY` when present for those commands.
- API key management endpoints must be JWT-only.
- Users can create, list, and revoke their own keys.
- API key plaintext is returned only once at creation time.

### 2.2 Security requirements

- Server never stores plaintext key.
- Server stores key hash and non-sensitive metadata.
- Revoked keys must fail immediately.
- API key cannot call API key management endpoints.

## 3. High-Level Design

### 3.1 Dual authentication channels

- Business operations (`/api/projects*` and related authenticated routes):
  - Accept `Authorization: Bearer <jwt>` or `Authorization: Bearer <api_key>`.
- API key management (`/api/apikeys*`):
  - Accept only JWT.
  - Reject API key credentials with `403 Forbidden`.

This ensures API keys are operational credentials, not identity-escalation credentials.

### 3.2 CLI auth source selection

For `deploy/list/info/rollback`:
1. If `STATICHUB_API_KEY` is set and non-empty, use it as Bearer credential.
2. Else, use local `~/.statichub/credentials.json` JWT.
3. Else, prompt user to login or set env var.

For `apikey create/list/revoke`:
- Must use local JWT login credential.
- If not logged in, command fails with clear guidance to run `statichub login`.

## 4. Data Model

Add new table: `api_keys`

- `id` INTEGER PRIMARY KEY AUTOINCREMENT
- `user_id` INTEGER NOT NULL
- `name` TEXT NOT NULL
- `key_prefix` TEXT NOT NULL
- `key_hash` TEXT NOT NULL
- `last_used_at` TIMESTAMP NULL
- `revoked_at` TIMESTAMP NULL
- `created_at` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP

Indexes:
- `idx_api_keys_user_id` on `(user_id)`
- `idx_api_keys_prefix` on `(key_prefix)`

Optional future hardening (not required now):
- Uniqueness on `(user_id, name)`.

## 5. API Key Format and Storage

Plaintext key format:
- Prefix: `shk_`
- Body: cryptographically secure random bytes encoded for CLI-safe copy/paste.

Example shape: `shk_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`

Storage policy:
- Persist only hash (`key_hash`) plus metadata.
- `key_prefix` stores short visible prefix for identification in list output.
- Plaintext key appears only in create API response once.

## 6. API Contract

All routes below are under authenticated API namespace.

### 6.1 Create key (JWT-only)

`POST /api/apikeys`

Request:
```json
{ "name": "ci-key" }
```

Response:
```json
{
  "id": 12,
  "name": "ci-key",
  "prefix": "shk_ab12cd",
  "api_key": "shk_...plaintext..."
}
```

Notes:
- `api_key` is returned only once.

### 6.2 List keys (JWT-only)

`GET /api/apikeys`

Response:
```json
[
  {
    "id": 12,
    "name": "ci-key",
    "prefix": "shk_ab12cd",
    "created_at": "2026-05-25T10:00:00Z",
    "last_used_at": "2026-05-25T12:30:00Z",
    "revoked": false
  }
]
```

Notes:
- No plaintext key in list response.

### 6.3 Revoke key (JWT-only)

`POST /api/apikeys/:id/revoke`

Response:
```json
{ "ok": true }
```

Behavior:
- Idempotent: revoking an already-revoked key still returns success for owner.
- Non-owner/non-existent key returns `404`.

## 7. CLI Contract

Add command group:

- `statichub apikey create <name>`
- `statichub apikey list`
- `statichub apikey revoke <id>`

Behavior:
- All `apikey` commands require local login (JWT in credentials file).
- `STATICHUB_API_KEY` is ignored for `apikey` command group.
- For successful create, CLI prints clear warning that key is shown once and should be stored securely.

## 8. Error Handling

- `401 Unauthorized`:
  - Missing or invalid bearer credential for business endpoints.
  - Revoked API key.
- `403 Forbidden`:
  - API key attempts to access JWT-only `/api/apikeys*` endpoints.
- `404 Not Found`:
  - Revoke target does not exist or is not owned by caller.
- `400 Bad Request`:
  - Invalid API key create payload (e.g., empty name).

## 9. Testing Strategy

### 9.1 Server tests

- API key authenticates business endpoints.
- API key denied on key-management endpoints (`403`).
- JWT continues to authenticate all existing authenticated endpoints.
- Create returns plaintext key once; DB stores only hash.
- List never exposes plaintext key.
- Revoke invalidates key immediately.
- Cross-user isolation: cannot view/revoke other users' keys.

### 9.2 CLI tests

- With `STATICHUB_API_KEY`, `deploy/list/info/rollback` work without local login.
- `apikey *` commands fail when not logged in (clear message).
- Auth precedence behaves as designed.

## 10. Backward Compatibility and Risks

Compatibility:
- Existing OAuth login UX unchanged.
- Existing JWT-based commands remain supported.

Risks:
- Mistaking API key for JWT in middleware parsing.
- Increased auth complexity in one middleware.

Mitigations:
- Keep credential parsing explicit and deterministic.
- Add focused unit/integration tests for both credential types and route-level policy.

## 11. Rollout Notes

- Requires DB migration for `api_keys` table.
- No data migration from existing auth records.
- Documentation updates needed in README and env var section.

## 12. Success Criteria

- User can run:
  - `statichub login`
  - `statichub apikey create ci-key`
  - Export `STATICHUB_API_KEY`
  - Run authenticated project commands later without re-login.
- User cannot manage keys unless logged in with JWT.
