# Auth-Disabled Local Start Design (2026-05-28)

## Summary
Enable `statichub-server` to start quickly in local environments without requiring Google OAuth environment variables (`STATICHUB_GOOGLE_CLIENT_ID`, `STATICHUB_GOOGLE_CLIENT_SECRET`) and related auth setup. When auth prerequisites are missing, the server enters an explicit `AuthDisabled` mode where authentication and protected management APIs are unavailable, while anonymous deploy/static serving remain available.

## Goals
- Allow local server startup without Google OAuth env vars.
- Keep security posture explicit: no accidental auth bypass.
- Preserve existing behavior in fully configured environments.
- Keep changes focused and backward-compatible.

## Non-Goals
- No change to production auth behavior when env is fully configured.
- No change to database schema, host routing logic, rollback semantics, or TLS behavior.
- No fake/dev user auth flow.

## Requirements
- If both `STATICHUB_GOOGLE_CLIENT_ID` and `STATICHUB_GOOGLE_CLIENT_SECRET` are present, auth mode is `Enabled` (current behavior).
- Otherwise auth mode is `Disabled`.
- In `Disabled` mode:
  - `/auth/*` endpoints return `503 Service Unavailable` with consistent machine-readable payload.
  - protected authenticated management endpoints return `503 Service Unavailable` with same payload.
  - anonymous deploy, health, and static serving continue to work.
- `STATICHUB_JWT_SECRET` is only required when auth mode is `Enabled`.

## Proposed Design

### 1) Auth mode determination
Add explicit runtime auth mode decision in config/startup path.

- New enum:
  - `AuthMode::Enabled`
  - `AuthMode::Disabled`
- Resolution logic:
  - `Enabled` when both Google OAuth env vars exist and are non-empty.
  - Else `Disabled`.
- Emit startup log indicating selected mode and reason.

### 2) Startup wiring (`server/src/main.rs`)
- Resolve auth mode before auth state creation.
- In `Enabled` mode:
  - Create `AuthState` as today (including JWT secret requirement).
- In `Disabled` mode:
  - Do not create `AuthState`.
  - Continue server startup without panicking on missing auth vars.

### 3) Router composition (`server/src/lib.rs`)
Refactor router assembly to be auth-mode aware.

- Always mounted:
  - `/health`
  - anonymous deploy endpoint(s)
  - static serving fallback
- `Enabled` mode mounts:
  - `/auth/login/google`
  - `/auth/callback/google`
  - `/auth/status/:session_id`
  - authenticated management endpoints (project deploy/list/info/rollback, apikey management)
- `Disabled` mode mounts same auth/protected paths as explicit disabled handlers returning `503`.

Rationale for mounting disabled handlers instead of removing paths:
- clearer client behavior than `404`
- easier diagnostics (“feature unavailable in current mode”)
- more stable API shape across modes

### 4) Disabled response contract
Use a consistent JSON response for disabled auth/protected endpoints:

```json
{
  "error": "authentication is disabled in local mode"
}
```

Status code: `503 Service Unavailable`.

## Data Flow

### Enabled mode
1. Startup resolves `AuthMode::Enabled`.
2. `AuthState` initialized.
3. Auth + protected routes use existing middleware and handlers.
4. Requests behave exactly as current implementation.

### Disabled mode
1. Startup resolves `AuthMode::Disabled`.
2. No `AuthState` initialization.
3. Requests to auth/protected endpoints are short-circuited by disabled handlers with `503`.
4. Anonymous deploy and static serving proceed unchanged.

## Error Handling
- No startup panic for missing Google OAuth env vars in local/degraded mode.
- Explicit runtime errors for disabled features:
  - status: `503`
  - body: stable error message above
- Startup logs include mode and missing-variable hint for troubleshooting.

## Testing Plan

### Unit tests
- Auth mode resolution tests:
  - both Google vars present -> `Enabled`
  - one/both missing -> `Disabled`
  - empty string treated as missing

### Integration tests
- In disabled mode:
  - `/auth/login/google` returns `503` + expected error payload
  - protected management endpoint returns `503` + expected payload
  - anonymous deploy endpoint remains reachable (non-503; existing semantics)
- In enabled mode:
  - existing auth and middleware tests continue to pass (regression coverage)

### Verification commands
- Minimal targeted validation first:
  - `cargo test -p statichub-server`
- If behavior-affecting changes are substantial, run:
  - `cargo test --workspace`
  - `cargo check --workspace`

## Documentation Updates
- `README.md` local run section:
  - clarify that Google OAuth vars are optional for quick local startup
  - document that auth/protected routes return `503` when auth is disabled
- `.env.example` comments:
  - separate base startup vars from auth-enabled vars

## Risks and Mitigations
- Risk: Clients may assume auth endpoints should always work.
  - Mitigation: stable `503` + explicit message instead of silent route removal.
- Risk: routing duplication for enabled/disabled modes increases maintenance burden.
  - Mitigation: keep shared route definitions centralized and add targeted tests.

## Rollout Strategy
- Ship as backward-compatible behavior change.
- No migration needed.
- Existing fully configured deployments remain unchanged.

## Open Questions
- None. Behavior and scope were confirmed during brainstorming.
