# StaticHub

> Static web publishing for front-end developers

StaticHub is a static site hosting platform similar to Surge and GitHub Pages. Deploy your static sites with a single command, manage custom domains, and track deployment versions.

## Features

- **🚀 Instant Deploys** - Deploy static sites with one command
- **🔓 Anonymous Deploys** - Quick deployments without login (free tier)
- **🔐 Authenticated Projects** - Manage named projects with Google OAuth
- **🌐 Custom Domains** - Map your own domains with file-based verification
- **📦 Version Management** - Keep deployment history and rollback instantly
- **⚙️ Project Configuration** - Clean URLs, SPA mode, redirects, custom headers
- **📝 Deploy History** - Track all deployments with metadata

## Quick Start

### 1. Deploy Anonymously (No Login Required)

```bash
# Deploy current directory
statichub deploy .

# Deploy specific directory
statichub deploy ./dist

# Deploy with config file
statichub deploy ./build --config statichub.yaml
```

You'll get a unique URL like `https://x7k2m9.statichub.dev` that expires after 24 hours.

### 2. Deploy to a Named Project (Requires Login)

```bash
# Login with Google
statichub login

# Deploy to a named project
statichub deploy ./dist --name my-app

# Your site is live at https://my-app.statichub.dev
```

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/Patrick0308/statichub/releases).

Each release package includes both the **CLI tool** (`statichub`) and the **server** (`statichub-server`).

**macOS (Intel)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-x86_64-apple-darwin.tar.gz | tar xz
# Install CLI only
sudo mv statichub /usr/local/bin/
# Or install both CLI and server
sudo mv statichub statichub-server /usr/local/bin/
```

**macOS (Apple Silicon)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-aarch64-apple-darwin.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
# Optionally install server: sudo mv statichub-server /usr/local/bin/
```

**Linux (x86_64)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-x86_64-linux-musl.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
# Optionally install server: sudo mv statichub-server /usr/local/bin/
```

**Windows**:
1. Download `statichub-x86_64-windows.zip` from the releases page
2. Extract the archive (contains `statichub.exe` and `statichub-server.exe`)
3. Add the extracted directory to your PATH

**Verify Installation**:
```bash
statichub --version
# If server is installed:
statichub-server --version
```

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/statichub.git
cd statichub

# Build the CLI
cargo build --release -p statichub

# The binary is at target/release/statichub
# Add it to your PATH or copy to /usr/local/bin
```

### Server Setup

```bash
# Set up environment variables
cp server/.env.example server/.env
# Edit .env with your configuration
```

**Configure environment variables in `.env`:**

- `STATICHUB_PORT` - Server listening port (default: 3000)
- `STATICHUB_ALLOWED_DOMAINS` - Comma-separated list of domains the server accepts (default: localhost,statichub.dev)
- `STATICHUB_DATABASE_URL` - SQLite database path (default: sqlite:statichub.db)
- `STATICHUB_STORAGE_PATH` - Path for deployment storage (default: ~/.statichub/deploys)
- `STATICHUB_GOOGLE_CLIENT_ID` - Google OAuth client ID
- `STATICHUB_GOOGLE_CLIENT_SECRET` - Google OAuth client secret
- `STATICHUB_GOOGLE_REDIRECT_URL` - OAuth callback URL (must match PORT, e.g., http://localhost:3000/auth/callback/google)
- `STATICHUB_JWT_SECRET` - Secret key for JWT token signing (use a long random string)

**Multi-domain setup:**

The server can serve multiple domains. Each domain in `STATICHUB_ALLOWED_DOMAINS` will:
- Accept deployments via CLI (using `STATICHUB_SERVER` environment variable)
- Serve static content based on hostname routing
- Support custom domain mappings

Example for production:
```bash
STATICHUB_PORT=80
STATICHUB_ALLOWED_DOMAINS=statichub.dev,statichub.com,localhost
```

**Start the server:**

```bash
# Initialize database (first time only)
statichub-server db init

# Start the server
statichub-server serve

# The server listens on the STATICHUB_PORT specified in .env
# and accepts requests for all domains in STATICHUB_ALLOWED_DOMAINS
```

**Database management commands:**

```bash
# Initialize new database
statichub-server db init

# Run pending migrations
statichub-server db migrate

# Check migration status
statichub-server db status

# Reset database (WARNING: deletes all data)
statichub-server db reset
```

## CLI Commands

### Authentication

```bash
# Login with Google OAuth
statichub login

# Logout
statichub logout
```

### Deployment

```bash
# Deploy anonymously (no login)
statichub deploy <directory>

# Deploy to named project (requires login)
statichub deploy <directory> --name <project-name>

# Deploy with custom config
statichub deploy <directory> --config statichub.yaml
```

### Project Management

```bash
# List your projects
statichub list

# View project details and deploy history
statichub info <project>

# Rollback to previous version
statichub rollback <project> <version>
```

### Custom Domains

```bash
# Add a custom domain
statichub domain add <project> example.com

# List domains for a project
statichub domain list <project>

# Verify domain ownership
statichub domain verify <project> example.com

# Remove a domain
statichub domain remove <project> example.com
```

## Configuration

Create a `statichub.yaml` file in your project root:

```yaml
# Clean URLs - remove .html extensions
clean_urls: true

# Single Page Application mode
spa: true

# Custom redirects
redirects:
  - from: /old-path
    to: /new-path
    status: 301
  - from: /blog/*
    to: /posts/:splat
    status: 302

# Custom HTTP headers
headers:
  - path: /*
    headers:
      X-Frame-Options: DENY
      X-Content-Type-Options: nosniff
  - path: /assets/*
    headers:
      Cache-Control: public, max-age=31536000, immutable

# Directory index files (default: index.html)
directory_index:
  - index.html
  - index.htm
```

### Configuration Options

**clean_urls** (boolean, default: `false`)
- Remove `.html` extensions from URLs
- `/about.html` becomes accessible at `/about`

**spa** (boolean, default: `false`)
- Enable Single Page Application mode
- All non-file paths serve `index.html`
- Useful for React, Vue, Angular apps

**redirects** (array)
- `from` (string, required) - Source path
- `to` (string, required) - Destination path
- `status` (integer, optional, default: 301) - HTTP status code
- Use `:splat` for wildcard matching

**headers** (array)
- `path` (string, required) - Path pattern (supports wildcards)
- `headers` (object, required) - Key-value pairs of HTTP headers

**directory_index** (array, default: `["index.html"]`)
- Files to serve for directory requests

## Custom Domain Setup

1. **Add your domain:**
   ```bash
   statichub domain add my-app example.com
   ```

2. **Create verification file:**

   Add a file named `statichub-verify.txt` to your site root containing the verification token shown in step 1.

3. **Deploy with verification file:**
   ```bash
   statichub deploy ./dist --name my-app
   ```

4. **Verify domain ownership:**
   ```bash
   statichub domain verify my-app example.com
   ```

5. **Configure DNS:**

   Point your domain to StaticHub (exact configuration provided by your server admin).

## Architecture

StaticHub is built with:

- **CLI**: Rust with Clap for command-line interface
- **Server**: Rust with Axum web framework
- **Database**: SQLite with SQLx for data persistence
- **Storage**: Filesystem storage (S3-ready with trait abstraction)
- **Authentication**: Google OAuth 2.0 with JWT tokens

### Project Structure

```
statichub/
├── cli/              # Command-line interface
├── server/           # API server and static file serving
│   ├── src/
│   │   ├── api/      # REST API endpoints
│   │   ├── models/   # Database models
│   │   ├── storage/  # Storage abstraction
│   │   └── middleware/ # Auth middleware
│   ├── migrations/   # Database migrations
│   └── tests/        # Integration tests
├── shared/           # Shared types between CLI and server
└── docs/             # Documentation and plans
```

## Development

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run server tests only
cargo test -p statichub-server

# Run CLI tests only
cargo test -p statichub

# Run specific test
cargo test test_add_domain
```

### Environment Variables

**Server:**
- `STATICHUB_DATABASE_URL` - SQLite database path (default: `sqlite:statichub.db`)
- `STATICHUB_PORT` - Server listening port (default: `3000`)
- `STATICHUB_ALLOWED_DOMAINS` - Comma-separated list of allowed domains (default: `localhost,statichub.dev`)
- `STATICHUB_STORAGE_PATH` - Path for deployment storage (default: `~/.statichub/deploys`)
- `STATICHUB_GOOGLE_CLIENT_ID` - Google OAuth client ID
- `STATICHUB_GOOGLE_CLIENT_SECRET` - Google OAuth client secret
- `STATICHUB_GOOGLE_REDIRECT_URL` - OAuth callback URL (must match your PORT)
- `STATICHUB_JWT_SECRET` - Secret key for JWT token signing

**CLI:**
- `STATICHUB_SERVER` - Server URL (default: `http://statichub.dev`)

**Multi-domain Configuration:**

The server uses `STATICHUB_PORT` and `STATICHUB_ALLOWED_DOMAINS` instead of a single `BASE_URL`. This allows:
- Serving multiple domains from one server instance
- Flexible deployment across different environments
- Hostname-based routing for static content

Example configurations:

**Development:**
```bash
# Server .env
STATICHUB_PORT=3000
STATICHUB_ALLOWED_DOMAINS=localhost,statichub.dev

# CLI environment
export STATICHUB_SERVER=http://localhost:3000
```

**Production:**
```bash
# Server .env
STATICHUB_PORT=80
STATICHUB_ALLOWED_DOMAINS=statichub.dev,statichub.com

# CLI environment (users point to their preferred domain)
export STATICHUB_SERVER=http://statichub.dev
```

The CLI's `STATICHUB_SERVER` should point to any domain in the server's `STATICHUB_ALLOWED_DOMAINS` list.

### Database Migrations

```bash
# Create a new migration
sqlx migrate add <name>

# Run pending migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

## API Endpoints

### Anonymous Deploys

- `POST /api/deploys` - Create anonymous deployment

### Authentication

- `POST /auth/login/google` - Initiate Google OAuth flow
- `GET /auth/callback/google` - OAuth callback
- `GET /auth/status/:session_id` - Check auth status

### Projects (Authenticated)

- `POST /api/projects/:name/deploys` - Deploy to named project
- `GET /api/projects` - List user's projects
- `GET /api/projects/:name` - Get project details
- `POST /api/projects/:name/rollback` - Rollback to version

### Domains (Authenticated)

- `POST /api/projects/:name/domains` - Add custom domain
- `GET /api/projects/:name/domains` - List project domains
- `POST /api/projects/:name/domains/:domain/verify` - Verify domain
- `DELETE /api/projects/:name/domains/:domain` - Remove domain

### Static File Serving

- `GET /*` - Serve static files (hostname-based routing)

## Security

- **JWT Authentication** - Secure token-based auth with 7-day expiry
- **OAuth 2.0** - Industry-standard authentication via Google
- **Project Ownership** - All operations validate user ownership
- **Path Traversal Protection** - Canonical path validation prevents directory escaping
- **Domain Verification** - File-based verification prevents domain hijacking
- **SQL Injection Prevention** - All queries use parameterized statements

## Roadmap

- [ ] GitHub OAuth provider
- [ ] Deploy tokens for CI/CD
- [ ] S3 storage backend
- [ ] Let's Encrypt SSL automation
- [ ] Deployment webhooks
- [ ] Access logs and analytics
- [ ] Team collaboration features
- [ ] Custom error pages (404, 500)

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Write tests for your changes
4. Ensure all tests pass (`cargo test --workspace`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

## License

[MIT License](LICENSE)

## Credits

Built with [Claude Code](https://claude.com/claude-code) by Anthropic.

## Support

- **Issues**: [GitHub Issues](https://github.com/yourusername/statichub/issues)
- **Documentation**: See `docs/` directory
- **Email**: support@statichub.dev
