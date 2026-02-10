# Mini-Claw

Lightweight Telegram bot for persistent AI conversations using [Pi coding agent](https://github.com/badlogic/pi-mono).

A minimalist alternative to OpenClaw - use your Claude Pro/Max or ChatGPT Plus subscription directly in Telegram, no API costs.

## Features

- **Persistent Sessions** - Conversations are saved and auto-compacted
- **Workspace Navigation** - Change directories with `/cd`, run shell commands with `/shell`
- **Session Management** - Archive, switch, and clean up old sessions
- **File Attachments** - Automatically sends files created by Pi (PDF, images, documents)
- **Image Support** - Send photos to Pi for vision analysis
- **Live Mode** - Interactive `/live` sessions with real-time Pi RPC
- **Rate Limiting** - Prevents message spam (configurable cooldown)
- **Access Control** - Optional allowlist for authorized users
- **Playwright Skill** - Built-in `pw` CLI for browser automation via Pi

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Telegram   │────►│  Mini-Claw  │────►│  Pi Agent   │
│   (User)    │◄────│   (Bot)     │◄────│  (Session)  │
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                           ▼
                    ~/.mini-claw/
                    └── sessions/
                        └── telegram-<chat_id>.jsonl
```

## Quick Start

### One-line Install (pre-built binary)

```bash
curl -fsSL https://raw.githubusercontent.com/siygle/mini-claw/main/install.sh | bash
```

This downloads the correct binary for your platform, walks you through configuration, and installs a system service.

### Build from Source

Requires [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/siygle/mini-claw
cd mini-claw

# Build and install as service
make deploy

# Or step by step:
make install        # Build + check pi-coding-agent
make login          # Authenticate with AI provider
cp .env.example .env  # Configure
make dev            # Run in debug mode
```

### Prerequisites

- [Pi coding agent](https://github.com/badlogic/pi-mono) (`npm install -g @mariozechner/pi-coding-agent`)
- A Telegram bot token from [@BotFather](https://t.me/BotFather)

## Bot Commands

| Command        | Description                        |
| -------------- | ---------------------------------- |
| `/start`       | Welcome message                    |
| `/help`        | Show all commands                  |
| `/pwd`         | Show current working directory     |
| `/cd <path>`   | Change working directory           |
| `/home`        | Go to home directory               |
| `/shell <cmd>` | Run shell command directly         |
| `/session`     | List and manage sessions           |
| `/new`         | Start fresh session (archives old) |
| `/status`      | Show bot status                    |
| `/live`        | Start interactive RPC session      |

## Configuration

All settings go in `.env` (or `~/.mini-claw/.env` when deployed via install script):

```bash
# Required
TELEGRAM_BOT_TOKEN=your_bot_token

# Optional
MINI_CLAW_WORKSPACE=~/mini-claw-workspace    # Default: ~/mini-claw-workspace
MINI_CLAW_SESSION_DIR=~/.mini-claw/sessions
PI_THINKING_LEVEL=low                         # low | medium | high
ALLOWED_USERS=123456,789012                   # Comma-separated user IDs

# Rate limiting & timeouts (milliseconds)
RATE_LIMIT_COOLDOWN_MS=5000                   # Default: 5 seconds
PI_TIMEOUT_MS=300000                          # Default: 5 minutes
SHELL_TIMEOUT_MS=60000                        # Default: 60 seconds
SESSION_TITLE_TIMEOUT_MS=10000                # Default: 10 seconds

# Web search (optional)
BRAVE_API_KEY=your_brave_api_key              # For Pi web search skill
```

## Deployment

### Install Script (recommended)

The install script handles binary download, configuration, and service setup for both macOS and Linux.

```bash
# Download binary + configure + install service
./install.sh

# Build from source instead of downloading
./install.sh --from-source

# Just download/build, skip service
./install.sh --build-only

# Install/restart service only (after manual config)
./install.sh --service-only

# Check service status
./install.sh --status

# Remove service
./install.sh --uninstall
```

Files are installed to `~/.mini-claw/`:

```
~/.mini-claw/
├── bin/
│   ├── mini-claw       # Bot binary
│   └── pw              # Playwright CLI
├── .env                # Configuration
├── sessions/           # Session storage
├── mini-claw.log       # Stdout log (macOS)
└── mini-claw.err.log   # Stderr log (macOS)
```

### macOS (launchd)

The install script creates a launchd user agent that starts on login.

```bash
# Manage service
launchctl print gui/$(id -u)/com.mini-claw.bot           # Status
launchctl kickstart -k gui/$(id -u)/com.mini-claw.bot    # Restart
launchctl bootout gui/$(id -u)/com.mini-claw.bot         # Stop
tail -f ~/.mini-claw/mini-claw.log                       # Logs
```

### Linux (systemd)

The install script creates a systemd user service.

```bash
# Manage service
systemctl --user status mini-claw      # Status
systemctl --user restart mini-claw     # Restart
systemctl --user stop mini-claw        # Stop
journalctl --user -u mini-claw -f      # Logs
```

## Development

```bash
make dev        # Build & run (debug mode)
make start      # Build & run (release mode)
make test       # Run tests
make clippy     # Run clippy lints
make check      # Run clippy + tests
make clean      # Remove build artifacts
```

### Playwright CLI

The `pw` binary is a standalone CLI for browser automation, used by Pi as a skill.

```bash
make pw-build     # Build pw binary
make pw-install   # Install pw to ~/.cargo/bin
```

### Project Structure

```
mini-claw/
├── Cargo.toml              # Workspace root
├── src/
│   ├── main.rs             # Entry point
│   ├── config.rs           # Configuration
│   ├── error.rs            # Error types
│   ├── markdown.rs         # Telegram markdown formatting
│   ├── file_detector.rs    # File detection in Pi output
│   ├── workspace.rs        # Workspace snapshots
│   ├── sessions.rs         # Session management
│   ├── pi_runner.rs        # Pi one-shot execution
│   ├── pi_rpc.rs           # Pi RPC protocol (live mode)
│   ├── rate_limiter.rs     # Per-chat rate limiting
│   └── bot/
│       ├── mod.rs          # Bot setup & dispatcher
│       ├── commands.rs     # Telegram command handlers
│       ├── callbacks.rs    # Inline keyboard callbacks
│       ├── handlers.rs     # Message handlers
│       └── util.rs         # Message splitting & helpers
├── skills/playwright/      # pw CLI crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── browser.rs
│       └── commands/
├── install.sh              # Cross-platform installer
├── Makefile
├── .env.example
└── .github/workflows/
    ├── ci.yml              # Build + test + clippy on PRs
    └── release.yml         # Cross-compile on tag push
```

## Tech Stack

- **Language**: Rust
- **Telegram**: [teloxide](https://github.com/teloxide/teloxide)
- **Browser Automation**: [chromiumoxide](https://github.com/mattsse/chromiumoxide)
- **AI Backend**: [Pi coding agent](https://github.com/badlogic/pi-mono)
- **CI/CD**: GitHub Actions (cross-compile for Linux & macOS, x86_64 & aarch64)

## Troubleshooting

### "Pi not authenticated"

```bash
pi /login
```

### "Session file locked"

Check for running Pi processes:

```bash
ps aux | grep pi
```

### Service not starting

Check logs:

```bash
# macOS
tail -f ~/.mini-claw/mini-claw.err.log

# Linux
journalctl --user -u mini-claw -f
```

Verify your token is set:

```bash
grep TELEGRAM_BOT_TOKEN ~/.mini-claw/.env
```

## License

MIT
