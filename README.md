# StaticHub

StaticHub is a static hosting platform for front-end projects. It supports fast anonymous deploys, named project deploys with auth, deployment history, and rollback.

## Quick Start

Install CLI:

```bash
curl -sSL https://raw.githubusercontent.com/Patrick0308/statichub/main/scripts/install.sh | sh
```

Deploy a directory:

```bash
statichub deploy ./dist
```

Deploy a single HTML file:

```bash
statichub deploy ~/Downloads/page.html
```

Output:

```text
📦 Collecting files from /Users/patrick/Downloads/page.html...
   Found 1 files
🚀 Deploying to https://statichub.dev...
✅ Deploy successful!
   URL: http://b7kr7b.statichub.dev
   Subdomain: b7kr7b
```

Login and deploy to a named project:

```bash
statichub login
statichub deploy ./dist --name my-app
```

## CLI Commands

```bash
statichub deploy <path> [--name <project>] [--config statichub.yaml]
statichub login
statichub logout
statichub list
statichub info <project>
statichub rollback <project> <version>
```

## `statichub` Skill (for AI Agents)

This repository includes a reusable skill at `skills/statichub` for agents that need to deploy AI-generated static output safely.

Install with `npx skills`:

```bash
npx skills add Patrick0308/statichub --skill statichub
```

Global install:

```bash
npx skills add Patrick0308/statichub --skill statichub -g
```

Check installed skills:

```bash
npx skills ls
```

Skill behavior summary:

1. Requires explicit deploy path: `statichub deploy <path>`
2. Accepts only a non-empty directory or non-empty `.html` file
3. Stops on validation failure and returns repair commands
4. Uses named deploy when `--name` is present, anonymous otherwise

Real output example:

```text
✅ Deploy successful!
   URL: http://b7kr7b.statichub.dev
   Subdomain: b7kr7b
```

The `URL` value is the live page URL.

## Optional Config (`statichub.yaml`)

```yaml
clean_urls: true
spa: true
redirects:
  - from: /old-path
    to: /new-path
    status: 301
headers:
  - path: /*
    headers:
      X-Frame-Options: DENY
directory_index:
  - index.html
```

## Run Server Locally

Create `.env` values and start server:

```bash
cp server/.env.example server/.env
statichub-server db init
statichub-server serve
```

Important env vars:

- `STATICHUB_PORT`
- `STATICHUB_ALLOWED_DOMAINS`
- `STATICHUB_DATABASE_URL`
- `STATICHUB_STORAGE_PATH`
- `STATICHUB_JWT_SECRET`
- `STATICHUB_GOOGLE_CLIENT_ID`
- `STATICHUB_GOOGLE_CLIENT_SECRET`
- `STATICHUB_GOOGLE_REDIRECT_URL`

## Routing Behavior

- Base domain (for example `statichub.io`) serves the built-in product homepage.
- Subdomains serve deployed static files for projects (for example `my-app.statichub.io`).
- Built-in homepage assets are reserved under `/__home/*`.

## Development

Run tests:

```bash
cargo test --workspace
```

Project layout:

```text
cli/      # CLI binary
server/   # API server and static serving
shared/   # shared types
skills/   # reusable agent skills
docs/     # docs and plans
```
