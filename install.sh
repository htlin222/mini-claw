#!/usr/bin/env bash
set -euo pipefail

# ── Mini-Claw Install & Deploy Script ────────────────────────────────────────
# Downloads pre-built binaries (or builds from source) and sets up the
# Mini-Claw Telegram bot as a system service on macOS or Linux.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/siygle/mini-claw/main/install.sh | bash
#   ./install.sh [OPTIONS]
#
# Options:
#   --from-source    Build from source instead of downloading pre-built binaries
#   --build-only     Build/download only, skip service installation
#   --service-only   Install/restart service only (binaries must already exist)
#   --uninstall      Stop and remove the service
#   --status         Show service status
#   --help           Show this help
# ─────────────────────────────────────────────────────────────────────────────

REPO="siygle/mini-claw"
INSTALL_DIR="${MINI_CLAW_INSTALL_DIR:-$HOME/.mini-claw}"
BIN_DIR="${INSTALL_DIR}/bin"
CONFIG_DIR="${INSTALL_DIR}"
SERVICE_NAME="mini-claw"
LAUNCHD_LABEL="com.mini-claw.bot"
LAUNCHD_PLIST="$HOME/Library/LaunchAgents/${LAUNCHD_LABEL}.plist"
SYSTEMD_SERVICE="$HOME/.config/systemd/user/${SERVICE_NAME}.service"

# ── Colors ───────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }
header()  { echo -e "\n${BOLD}$*${NC}"; }

command_exists() { command -v "$1" &>/dev/null; }

# ── Platform Detection ───────────────────────────────────────────────────────

detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux*)  OS="linux" ;;
        darwin*) OS="darwin" ;;
        *)       error "Unsupported OS: $os"; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)             error "Unsupported architecture: $arch"; exit 1 ;;
    esac

    case "${OS}-${ARCH}" in
        linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
        linux-aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
        darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
        darwin-aarch64) TARGET="aarch64-apple-darwin" ;;
    esac

    info "Platform: ${OS}/${ARCH} (${TARGET})"
}

# ── Usage ────────────────────────────────────────────────────────────────────

usage() {
    cat <<'EOF'
Mini-Claw Install & Deploy Script

Usage: ./install.sh [OPTIONS]

Options:
  --from-source    Build from source instead of downloading pre-built binaries
  --build-only     Build/download only, skip service installation
  --service-only   Install/restart service only (binaries must already exist)
  --uninstall      Stop and remove the service
  --status         Show service status
  --help           Show this help

Environment variables:
  MINI_CLAW_INSTALL_DIR   Install directory (default: ~/.mini-claw)
  MINI_CLAW_VERSION       Version to install (default: latest)
EOF
    exit 0
}

# ── Get Latest Version ───────────────────────────────────────────────────────

get_latest_version() {
    if [ -n "${MINI_CLAW_VERSION:-}" ]; then
        echo "$MINI_CLAW_VERSION"
        return
    fi

    local version
    version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')

    if [ -z "$version" ]; then
        error "Could not determine latest version. Set MINI_CLAW_VERSION manually."
        exit 1
    fi
    echo "$version"
}

# ── Download Pre-built Binaries ──────────────────────────────────────────────

download_binaries() {
    header "Downloading Mini-Claw..."

    local version archive_name url tmp_dir
    version=$(get_latest_version)
    archive_name="mini-claw-${version}-${TARGET}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

    info "Version: ${version}"
    info "URL: ${url}"

    tmp_dir=$(mktemp -d)
    trap "rm -rf ${tmp_dir}" EXIT

    if ! curl -fSL --progress-bar -o "${tmp_dir}/${archive_name}" "$url"; then
        error "Download failed. Check that version ${version} exists and has binaries for ${TARGET}."
        error "You can try: ./install.sh --from-source"
        exit 1
    fi

    tar xzf "${tmp_dir}/${archive_name}" -C "${tmp_dir}"

    mkdir -p "$BIN_DIR"

    # Find and copy binaries (they're inside a directory in the tarball)
    local extract_dir="${tmp_dir}/mini-claw-${version}-${TARGET}"
    cp "${extract_dir}/mini-claw" "${BIN_DIR}/mini-claw"
    cp "${extract_dir}/pw" "${BIN_DIR}/pw"
    chmod +x "${BIN_DIR}/mini-claw" "${BIN_DIR}/pw"

    # Copy .env.example if no .env exists
    if [ ! -f "${CONFIG_DIR}/.env" ] && [ -f "${extract_dir}/.env.example" ]; then
        cp "${extract_dir}/.env.example" "${CONFIG_DIR}/.env.example"
    fi

    trap - EXIT
    rm -rf "$tmp_dir"

    success "Binaries installed to ${BIN_DIR}/"
}

# ── Build From Source ────────────────────────────────────────────────────────

build_from_source() {
    header "Building from source..."

    if ! command_exists cargo; then
        warn "Rust toolchain not found."
        read -rp "Install Rust via rustup? [Y/n] " answer
        if [[ "${answer:-Y}" =~ ^[Yy]$ ]]; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source "$HOME/.cargo/env"
        else
            error "Rust is required to build from source. Install from https://rustup.rs"
            exit 1
        fi
    fi

    # Must be in project directory for cargo build
    local project_dir
    project_dir="$(cd "$(dirname "$0")" && pwd)"

    if [ ! -f "${project_dir}/Cargo.toml" ]; then
        error "Cargo.toml not found. Run this script from the project directory or use download mode."
        exit 1
    fi

    info "Building release binaries (this may take a few minutes)..."
    (cd "$project_dir" && cargo build --release --workspace)

    mkdir -p "$BIN_DIR"
    cp "${project_dir}/target/release/mini-claw" "${BIN_DIR}/mini-claw"
    cp "${project_dir}/target/release/pw" "${BIN_DIR}/pw"
    chmod +x "${BIN_DIR}/mini-claw" "${BIN_DIR}/pw"

    # Copy .env.example if available and no .env exists yet
    if [ ! -f "${CONFIG_DIR}/.env" ] && [ -f "${project_dir}/.env.example" ]; then
        cp "${project_dir}/.env.example" "${CONFIG_DIR}/.env.example"
    fi

    success "Binaries installed to ${BIN_DIR}/"
}

# ── Configure ────────────────────────────────────────────────────────────────

configure() {
    header "Configuring..."

    mkdir -p "${CONFIG_DIR}/sessions"
    mkdir -p "${HOME}/mini-claw-workspace"

    # Set up .env
    if [ ! -f "${CONFIG_DIR}/.env" ]; then
        if [ -f "${CONFIG_DIR}/.env.example" ]; then
            cp "${CONFIG_DIR}/.env.example" "${CONFIG_DIR}/.env"
        else
            cat > "${CONFIG_DIR}/.env" <<'ENVEOF'
# Telegram Bot Token (get from @BotFather)
TELEGRAM_BOT_TOKEN=

# Optional settings (uncomment to customize)
# MINI_CLAW_WORKSPACE=~/mini-claw-workspace
# MINI_CLAW_SESSION_DIR=~/.mini-claw/sessions
# PI_THINKING_LEVEL=low
# ALLOWED_USERS=
ENVEOF
        fi
        info "Created ${CONFIG_DIR}/.env"
    fi

    # Check if TELEGRAM_BOT_TOKEN is set
    local token
    token=$(grep -E '^TELEGRAM_BOT_TOKEN=' "${CONFIG_DIR}/.env" | cut -d= -f2- | tr -d '[:space:]' || true)

    if [ -z "$token" ]; then
        warn "TELEGRAM_BOT_TOKEN is not set."
        echo ""
        echo "Get a bot token from @BotFather on Telegram, then enter it below."
        read -rp "Telegram Bot Token (or press Enter to skip): " token
        if [ -n "$token" ]; then
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' "s|^TELEGRAM_BOT_TOKEN=.*|TELEGRAM_BOT_TOKEN=${token}|" "${CONFIG_DIR}/.env"
            else
                sed -i "s|^TELEGRAM_BOT_TOKEN=.*|TELEGRAM_BOT_TOKEN=${token}|" "${CONFIG_DIR}/.env"
            fi
            success "Token saved to ${CONFIG_DIR}/.env"
        else
            warn "Skipped. Edit ${CONFIG_DIR}/.env before starting the service."
        fi
    else
        success "Telegram token configured"
    fi

    # Check pi-coding-agent
    if ! command_exists pi; then
        warn "pi-coding-agent not found."
        if command_exists npm; then
            read -rp "Install pi-coding-agent via npm? [Y/n] " answer
            if [[ "${answer:-Y}" =~ ^[Yy]$ ]]; then
                npm install -g @mariozechner/pi-coding-agent
                success "pi-coding-agent installed"
            fi
        else
            warn "npm not found. Install Node.js, then: npm install -g @mariozechner/pi-coding-agent"
        fi
    else
        success "pi-coding-agent found"
    fi

    # Check pi auth
    if command_exists pi; then
        if pi --version &>/dev/null; then
            success "Pi authenticated"
        else
            warn "Pi not authenticated."
            read -rp "Run 'pi /login' now? [Y/n] " answer
            if [[ "${answer:-Y}" =~ ^[Yy]$ ]]; then
                pi /login
            else
                warn "Run 'pi /login' before starting the bot."
            fi
        fi
    fi
}

# ── Service: systemd (Linux) ────────────────────────────────────────────────

install_systemd() {
    header "Installing systemd service..."

    mkdir -p "$(dirname "$SYSTEMD_SERVICE")"

    cat > "$SYSTEMD_SERVICE" <<EOF
[Unit]
Description=Mini-Claw Telegram Bot
After=network.target

[Service]
Type=simple
ExecStart=${BIN_DIR}/mini-claw
WorkingDirectory=${HOME}/mini-claw-workspace
EnvironmentFile=${CONFIG_DIR}/.env
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload
    systemctl --user enable "$SERVICE_NAME"
    systemctl --user restart "$SERVICE_NAME"

    success "Service installed and started"
    echo ""
    info "Useful commands:"
    echo "  systemctl --user status ${SERVICE_NAME}    # Check status"
    echo "  systemctl --user restart ${SERVICE_NAME}   # Restart"
    echo "  systemctl --user stop ${SERVICE_NAME}      # Stop"
    echo "  journalctl --user -u ${SERVICE_NAME} -f    # View logs"
}

uninstall_systemd() {
    header "Removing systemd service..."

    if systemctl --user is-active "$SERVICE_NAME" &>/dev/null; then
        systemctl --user stop "$SERVICE_NAME"
        success "Service stopped"
    fi

    if [ -f "$SYSTEMD_SERVICE" ]; then
        systemctl --user disable "$SERVICE_NAME" 2>/dev/null || true
        rm -f "$SYSTEMD_SERVICE"
        systemctl --user daemon-reload
        success "Service file removed"
    else
        info "Service file not found, nothing to remove"
    fi
}

status_systemd() {
    systemctl --user status "$SERVICE_NAME" 2>/dev/null || info "Service not installed"
}

# ── Service: launchd (macOS) ─────────────────────────────────────────────────

install_launchd() {
    header "Installing launchd service..."

    mkdir -p "$(dirname "$LAUNCHD_PLIST")"
    mkdir -p "${CONFIG_DIR}"

    # Parse .env file into launchd EnvironmentVariables
    local env_dict=""
    if [ -f "${CONFIG_DIR}/.env" ]; then
        while IFS='=' read -r key value; do
            # Skip comments and empty lines
            [[ "$key" =~ ^#.*$ || -z "$key" ]] && continue
            # Strip leading/trailing whitespace
            key=$(echo "$key" | xargs)
            value=$(echo "$value" | xargs)
            [ -z "$key" ] && continue
            env_dict="${env_dict}
      <key>${key}</key>
      <string>${value}</string>"
        done < "${CONFIG_DIR}/.env"
    fi

    cat > "$LAUNCHD_PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${LAUNCHD_LABEL}</string>

    <key>ProgramArguments</key>
    <array>
        <string>${BIN_DIR}/mini-claw</string>
    </array>

    <key>WorkingDirectory</key>
    <string>${HOME}/mini-claw-workspace</string>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>${CONFIG_DIR}/mini-claw.log</string>

    <key>StandardErrorPath</key>
    <string>${CONFIG_DIR}/mini-claw.err.log</string>

    <key>EnvironmentVariables</key>
    <dict>
      <key>HOME</key>
      <string>${HOME}</string>
      <key>PATH</key>
      <string>/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin:${HOME}/.cargo/bin:${HOME}/.nvm/versions/node/$(node --version 2>/dev/null || echo v0)/bin</string>${env_dict}
    </dict>
</dict>
</plist>
EOF

    # Unload first if already loaded
    launchctl bootout "gui/$(id -u)/${LAUNCHD_LABEL}" 2>/dev/null || true

    launchctl bootstrap "gui/$(id -u)" "$LAUNCHD_PLIST"

    success "Service installed and started"
    echo ""
    info "Useful commands:"
    echo "  launchctl print gui/$(id -u)/${LAUNCHD_LABEL}                  # Check status"
    echo "  launchctl kickstart -k gui/$(id -u)/${LAUNCHD_LABEL}           # Restart"
    echo "  launchctl bootout gui/$(id -u)/${LAUNCHD_LABEL}                # Stop"
    echo "  tail -f ${CONFIG_DIR}/mini-claw.log                            # View logs"
    echo "  tail -f ${CONFIG_DIR}/mini-claw.err.log                        # View error logs"
}

uninstall_launchd() {
    header "Removing launchd service..."

    launchctl bootout "gui/$(id -u)/${LAUNCHD_LABEL}" 2>/dev/null && success "Service stopped" || true

    if [ -f "$LAUNCHD_PLIST" ]; then
        rm -f "$LAUNCHD_PLIST"
        success "Plist removed"
    else
        info "Plist not found, nothing to remove"
    fi
}

status_launchd() {
    launchctl print "gui/$(id -u)/${LAUNCHD_LABEL}" 2>/dev/null || info "Service not installed"
}

# ── Service Router ───────────────────────────────────────────────────────────

install_service() {
    case "$OS" in
        linux)  install_systemd ;;
        darwin) install_launchd ;;
    esac
}

uninstall_service() {
    detect_platform
    case "$OS" in
        linux)  uninstall_systemd ;;
        darwin) uninstall_launchd ;;
    esac
    echo ""
    info "Binaries left in ${BIN_DIR}/. Remove manually if desired:"
    echo "  rm -rf ${INSTALL_DIR}"
}

show_status() {
    detect_platform
    case "$OS" in
        linux)  status_systemd ;;
        darwin) status_launchd ;;
    esac
}

# ── Summary ──────────────────────────────────────────────────────────────────

print_summary() {
    header "Installation complete!"
    echo ""
    echo "  Binaries:  ${BIN_DIR}/mini-claw"
    echo "             ${BIN_DIR}/pw"
    echo "  Config:    ${CONFIG_DIR}/.env"
    echo "  Sessions:  ${CONFIG_DIR}/sessions/"
    echo "  Workspace: ${HOME}/mini-claw-workspace/"
    echo ""

    if [ "$OS" = "darwin" ]; then
        echo "  Logs:      tail -f ${CONFIG_DIR}/mini-claw.log"
        echo "  Restart:   launchctl kickstart -k gui/$(id -u)/${LAUNCHD_LABEL}"
        echo "  Stop:      launchctl bootout gui/$(id -u)/${LAUNCHD_LABEL}"
    else
        echo "  Logs:      journalctl --user -u ${SERVICE_NAME} -f"
        echo "  Restart:   systemctl --user restart ${SERVICE_NAME}"
        echo "  Stop:      systemctl --user stop ${SERVICE_NAME}"
    fi
    echo "  Uninstall: ./install.sh --uninstall"
    echo ""
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
    local from_source=false
    local build_only=false
    local service_only=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --from-source)  from_source=true; shift ;;
            --build-only)   build_only=true; shift ;;
            --service-only) service_only=true; shift ;;
            --uninstall)    uninstall_service; exit 0 ;;
            --status)       show_status; exit 0 ;;
            --help|-h)      usage ;;
            *) error "Unknown option: $1"; usage ;;
        esac
    done

    echo ""
    echo -e "${BOLD}  Mini-Claw Installer${NC}"
    echo "  ─────────────────────"
    echo ""

    detect_platform

    if [ "$service_only" = false ]; then
        if [ "$from_source" = true ]; then
            build_from_source
        else
            download_binaries
        fi

        configure
    fi

    if [ "$build_only" = false ]; then
        # Verify binary exists before installing service
        if [ ! -x "${BIN_DIR}/mini-claw" ]; then
            error "Binary not found at ${BIN_DIR}/mini-claw"
            error "Run without --service-only first, or use --from-source."
            exit 1
        fi

        # Verify token is set
        local token
        token=$(grep -E '^TELEGRAM_BOT_TOKEN=' "${CONFIG_DIR}/.env" 2>/dev/null | cut -d= -f2- | tr -d '[:space:]' || true)
        if [ -z "$token" ]; then
            warn "TELEGRAM_BOT_TOKEN not set in ${CONFIG_DIR}/.env"
            warn "The service will fail to start until you set it."
            read -rp "Install service anyway? [y/N] " answer
            if [[ ! "${answer:-N}" =~ ^[Yy]$ ]]; then
                info "Skipped service installation. Set the token, then run: ./install.sh --service-only"
                exit 0
            fi
        fi

        install_service
    fi

    print_summary
}

main "$@"
