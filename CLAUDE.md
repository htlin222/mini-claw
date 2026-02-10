# Mini-Claw

Lightweight Telegram bot for persistent AI conversations using Pi coding agent.

## Project Goals

- **Simple**: Minimal dependencies, single-purpose
- **Persistent**: Long-running conversations with session management
- **Subscription-friendly**: Use Claude Pro/Max or ChatGPT Plus via OAuth (no API costs)

## Tech Stack

- **Language**: Rust (2021 edition)
- **Build**: Cargo workspace (root + `skills/playwright`)
- **AI Backend**: [@mariozechner/pi-coding-agent](https://github.com/badlogic/pi-mono)
- **Telegram**: [teloxide](https://github.com/teloxide/teloxide) (mature Rust Telegram framework)
- **Browser Automation**: [chromiumoxide](https://github.com/nickel-project/chromiumoxide) (pure Rust CDP client)
- **Process**: Single long-running binary (systemd/tmux)

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

**Two Pi interaction modes:**
- **One-shot** (`--print`): Default. Spawns a new Pi process per message.
- **Live** (`--mode rpc`): Toggle with `/live`. Persistent Pi process per chat with mid-conversation interaction.

## Directory Structure

```
tehran/
├── CLAUDE.md              # This file
├── Cargo.toml             # Workspace root + mini-claw binary
├── Makefile               # Quick commands
├── .env.example           # Environment template
├── src/
│   ├── main.rs            # Entry point
│   ├── config.rs          # Configuration (env vars)
│   ├── error.rs           # MiniClawError enum
│   ├── rate_limiter.rs    # Per-chat rate limiting
│   ├── markdown.rs        # Markdown → Telegram HTML
│   ├── file_detector.rs   # Detect files in Pi output
│   ├── workspace.rs       # Per-chat working directory
│   ├── pi_runner.rs       # One-shot Pi (--print mode)
│   ├── pi_rpc.rs          # Persistent Pi (--mode rpc)
│   ├── sessions.rs        # Session management
│   └── bot/
│       ├── mod.rs          # AppState, dispatcher setup
│       ├── commands.rs     # /start, /help, /cd, /shell, /live, etc.
│       ├── handlers.rs     # Text & photo message handlers
│       ├── callbacks.rs    # Inline keyboard callbacks
│       └── util.rs         # split_message, run_shell
└── skills/playwright/
    ├── Cargo.toml          # pw CLI crate
    └── src/
        ├── main.rs         # Clap CLI entry point
        ├── browser.rs      # chromiumoxide session
        └── commands/
            ├── mod.rs
            ├── navigate.rs
            ├── screenshot.rs
            ├── interact.rs
            ├── content.rs
            └── wait.rs
```

## Quick Start

```bash
# 1. Build & install
make install

# 2. Login to AI provider (Claude/ChatGPT)
make login

# 3. Configure Telegram bot token
cp .env.example .env
# Edit .env with your TELEGRAM_BOT_TOKEN

# 4. Start the bot
make dev
```

## Makefile Commands

| Command              | Description                        |
| -------------------- | ---------------------------------- |
| `make install`       | Build workspace + install Pi agent |
| `make login`         | Run `pi /login` to authenticate    |
| `make dev`           | Build & run bot (debug)            |
| `make start`         | Build & run bot (release)          |
| `make build`         | Compile all workspace crates       |
| `make test`          | Run all tests                      |
| `make clippy`        | Run clippy lints                   |
| `make check`         | Run clippy + tests                 |
| `make status`        | Check Pi auth status               |
| `make clean`         | Remove build artifacts             |
| `make pw-build`      | Build pw CLI binary                |
| `make pw-install`    | Install pw CLI to ~/.cargo/bin     |

## Environment Variables

```bash
# Required
TELEGRAM_BOT_TOKEN=your_telegram_bot_token

# Optional
MINI_CLAW_WORKSPACE=/path/to/workspace  # Default: ~/mini-claw-workspace
MINI_CLAW_SESSION_DIR=~/.mini-claw/sessions
PI_THINKING_LEVEL=low                   # low | medium | high
ALLOWED_USERS=123,456                   # Comma-separated user IDs (empty = allow all)

# Rate Limiting & Timeouts (all in milliseconds)
RATE_LIMIT_COOLDOWN_MS=5000             # Default: 5 seconds between messages
PI_TIMEOUT_MS=300000                    # Default: 5 minutes
SHELL_TIMEOUT_MS=60000                  # Default: 60 seconds
SESSION_TITLE_TIMEOUT_MS=10000          # Default: 10 seconds
```

## Session Management

- Each Telegram chat gets its own Pi session file
- Session file: `~/.mini-claw/sessions/telegram-<chat_id>.jsonl`
- Pi handles auto-compaction when context window fills
- Full history preserved in JSONL, compacted context for AI

## Bot Commands

| Command        | Description                       |
| -------------- | --------------------------------- |
| `/start`       | Welcome message                   |
| `/help`        | Show all commands                 |
| `/pwd`         | Show current working directory    |
| `/cd <path>`   | Change working directory          |
| `/home`        | Go to home directory              |
| `/shell <cmd>` | Run shell command directly        |
| `/session`     | List sessions with inline buttons |
| `/new`         | Start fresh session (archive old) |
| `/status`      | Show current session info         |
| `/live`        | Toggle persistent Pi session      |

Note: The bot registers these commands with Telegram, so they appear in the "/" menu.

## Concurrency Handling

- Per-chat `Mutex` locking prevents concurrent Pi executions
- Rate limiter with configurable cooldown
- Typing indicator while processing

## Development

```bash
# Build workspace
cargo build --workspace

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace

# Run bot in debug mode
cargo run
```

## Deployment

### Option 1: systemd (Linux)

```bash
make install-service  # Creates systemd user service
systemctl --user start mini-claw
systemctl --user enable mini-claw
```

### Option 2: tmux (manual)

```bash
tmux new -s mini-claw
make start
# Ctrl+B, D to detach
```

## Key Crates

| Purpose | Crate |
|---------|-------|
| Telegram bot | `teloxide 0.17` |
| Async runtime | `tokio` (full) |
| Serialization | `serde` + `serde_json` |
| HTTP client | `reqwest 0.12` |
| Browser automation | `chromiumoxide 0.8` |
| CLI parsing | `clap 4` (derive) |
| Errors | `thiserror` + `anyhow` |
| Logging | `tracing` + `tracing-subscriber` |

## License

MIT
