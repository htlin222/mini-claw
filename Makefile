.PHONY: install login dev start build status clean help test clippy check pw-build pw-install install-service deploy uninstall

# Default target
help:
	@echo "Mini-Claw - Lightweight Telegram AI Bot (Rust)"
	@echo ""
	@echo "Quick Start:"
	@echo "  make install    Install dependencies"
	@echo "  make login      Authenticate with AI provider (Claude/ChatGPT)"
	@echo "  make dev        Start in development mode"
	@echo ""
	@echo "Commands:"
	@echo "  make install    Install pi-coding-agent"
	@echo "  make login      Run 'pi /login' to authenticate"
	@echo "  make dev        Build & run bot (debug mode)"
	@echo "  make start      Build & run bot (release mode)"
	@echo "  make build      Compile all workspace crates"
	@echo "  make status     Check Pi auth status"
	@echo "  make clean      Remove build artifacts"
	@echo ""
	@echo "Quality:"
	@echo "  make test       Run tests"
	@echo "  make clippy     Run clippy lints"
	@echo "  make check      Run all checks (clippy + test)"
	@echo ""
	@echo "Deploy:"
	@echo "  make deploy     Build from source & install as service"
	@echo "  make uninstall  Stop service & remove service files"
	@echo ""
	@echo "Playwright Skill:"
	@echo "  make pw-build   Build pw CLI binary"
	@echo "  make pw-install Build and install pw CLI to ~/.cargo/bin"
	@echo ""
	@echo "Setup:"
	@echo "  1. make install"
	@echo "  2. make login"
	@echo "  3. cp .env.example .env && edit .env"
	@echo "  4. make dev"

# Install dependencies
install:
	@echo "Checking pi-coding-agent..."
	@which pi > /dev/null 2>&1 || (echo "Installing pi-coding-agent globally..." && npm install -g @mariozechner/pi-coding-agent)
	@echo ""
	@echo "Building project..."
	cargo build --workspace
	@echo ""
	@echo "Done! Next steps:"
	@echo "  1. Run 'make login' to authenticate with Claude/ChatGPT"
	@echo "  2. Copy .env.example to .env and add your Telegram bot token"
	@echo "  3. Run 'make dev' to start the bot"

# Login to AI provider
login:
	@echo "Starting Pi login..."
	@echo "Select your AI provider (Anthropic for Claude, OpenAI for ChatGPT)"
	@echo ""
	pi /login

# Development mode (debug build + run)
dev:
	@test -f .env || (echo "Error: .env file not found. Copy .env.example to .env first." && exit 1)
	cargo run

# Production start (release build + run)
start:
	@test -f .env || (echo "Error: .env file not found." && exit 1)
	cargo run --release

# Build all workspace crates
build:
	cargo build --workspace

# Check Pi status
status:
	@echo "Checking Pi installation..."
	@which pi > /dev/null 2>&1 && echo "Pi: installed at $$(which pi)" || echo "Pi: NOT INSTALLED"
	@echo ""
	@echo "Checking Pi auth..."
	@pi --version 2>/dev/null && echo "Pi: OK" || echo "Pi: not authenticated or not working"

# Run tests
test:
	cargo test --workspace

# Run clippy lints
clippy:
	cargo clippy --workspace

# Run all checks
check: clippy test

# Clean build artifacts
clean:
	cargo clean

# Install systemd service (Linux)
install-service:
	@echo "Building release binary..."
	cargo build --release
	@echo ""
	@echo "Creating systemd user service..."
	@mkdir -p ~/.config/systemd/user
	@echo "[Unit]" > ~/.config/systemd/user/mini-claw.service
	@echo "Description=Mini-Claw Telegram Bot" >> ~/.config/systemd/user/mini-claw.service
	@echo "After=network.target" >> ~/.config/systemd/user/mini-claw.service
	@echo "" >> ~/.config/systemd/user/mini-claw.service
	@echo "[Service]" >> ~/.config/systemd/user/mini-claw.service
	@echo "Type=simple" >> ~/.config/systemd/user/mini-claw.service
	@echo "WorkingDirectory=$$(pwd)" >> ~/.config/systemd/user/mini-claw.service
	@echo "ExecStart=$$(pwd)/target/release/mini-claw" >> ~/.config/systemd/user/mini-claw.service
	@echo "Restart=on-failure" >> ~/.config/systemd/user/mini-claw.service
	@echo "RestartSec=5" >> ~/.config/systemd/user/mini-claw.service
	@echo "" >> ~/.config/systemd/user/mini-claw.service
	@echo "[Install]" >> ~/.config/systemd/user/mini-claw.service
	@echo "WantedBy=default.target" >> ~/.config/systemd/user/mini-claw.service
	@echo ""
	@echo "Service created. Run:"
	@echo "  systemctl --user daemon-reload"
	@echo "  systemctl --user start mini-claw"
	@echo "  systemctl --user enable mini-claw"

# Playwright skill targets
pw-build:
	@echo "Building pw CLI..."
	cargo build -p pw

pw-install:
	@echo "Building and installing pw CLI..."
	cargo install --path skills/playwright
	@echo ""
	@echo "Done! Test with: pw --help"

# Deploy: build from source and install as service
deploy:
	./install.sh --from-source

# Uninstall service
uninstall:
	./install.sh --uninstall
