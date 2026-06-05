#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════
# Peerman Installation Script — DN42 Peer Manager
# https://github.com/peerman/peerman
# ═══════════════════════════════════════════════════════════

# ── Colors ──────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# ── Helpers ─────────────────────────────────────────────
info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

prompt() {
    local var_name="$1" prompt_text="$2" default="$3"
    local input
    if [[ -n "$default" ]]; then
        read -r -p "$prompt_text [$default]: " input
    else
        read -r -p "$prompt_text: " input
    fi
    eval "$var_name=\${input:-\$default}"
}

prompt_password() {
    local var_name="$1" prompt_text="$2"
    local pw1 pw2
    while true; do
        read -r -s -p "$prompt_text: " pw1; echo
        if [[ -z "$pw1" ]]; then
            warn "Password cannot be empty"
            continue
        fi
        read -r -s -p "Confirm password: " pw2; echo
        if [[ "$pw1" != "$pw2" ]]; then
            warn "Passwords do not match, try again"
            continue
        fi
        break
    done
    eval "$var_name=\$pw1"
}

prompt_yn() {
    local var_name="$1" prompt_text="$2" default="$3"
    local yn
    read -r -p "$prompt_text [$default]: " yn
    yn="${yn:-$default}"
    yn="${yn,,}"
    eval "$var_name=\$yn"
}

write_atomic() {
    local dest="$1" content="$2"
    printf '%s\n' "$content" > "${dest}.tmp"
    mv "${dest}.tmp" "$dest"
}

# ── Guards ──────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (needed for: system user creation, /etc writes, service install)"
fi

if [[ "$(uname -s)" != "Linux" ]]; then
    error "Peerman only supports Linux (WG + BIRD kernel interfaces required)"
fi

# ── Globals ─────────────────────────────────────────────
BINARY_PATH="/usr/local/bin/peerman"
CONFIG_DIR="/etc/peerman"
DATA_DIR="/var/lib/peerman"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
GITHUB_REPO="${PEERMAN_GITHUB_REPO:-zyxisme/peerman}"
GITHUB_RELEASES="https://github.com/${GITHUB_REPO}/releases"
INIT="none"
ARCH="x86_64-unknown-linux-musl"

# ── Cleanup ─────────────────────────────────────────────
cleanup() {
    rm -f "${CONFIG_DIR}"/*.tmp 2>/dev/null || true
}
trap cleanup EXIT

# ═══════════════════════════════════════════════════════════
# Distro Detection & Package Manager
# ═══════════════════════════════════════════════════════════
detect_distro() {
    PKG_MGR=""
    DISTRO_ID=""

    if [[ -f /etc/os-release ]]; then
        # shellcheck source=/dev/null
        . /etc/os-release
        DISTRO_ID="${ID:-unknown}"
    fi

    if command -v apt-get &>/dev/null; then
        PKG_MGR="apt"
    elif command -v apk &>/dev/null; then
        PKG_MGR="apk"
    elif command -v dnf &>/dev/null; then
        PKG_MGR="dnf"
    elif command -v yum &>/dev/null; then
        PKG_MGR="yum"
    elif command -v pacman &>/dev/null; then
        PKG_MGR="pacman"
    elif command -v zypper &>/dev/null; then
        PKG_MGR="zypper"
    elif command -v xbps-install &>/dev/null; then
        PKG_MGR="xbps"
    fi

    if [[ -n "$PKG_MGR" ]]; then
        success "Detected package manager: $PKG_MGR (${DISTRO_ID:-unknown})"
    else
        warn "No supported package manager detected"
    fi
}

# Map generic dep name → distro-specific package name
# Args: $1=generic_name, returns via stdout
dep_to_pkg() {
    local name="$1"
    case "$name" in
        wg)
            echo "wireguard-tools" ;;
        birdc)
            case "$PKG_MGR" in
                apt)   echo "bird2" ;;
                *)     echo "bird" ;;
            esac ;;
        ping)
            case "$PKG_MGR" in
                apt)        echo "iputils-ping" ;;
                pacman)     echo "iputils" ;;
                *)          echo "iputils-ping" ;;
            esac ;;
        traceroute)
            echo "traceroute" ;;
        *)
            echo "$name" ;;
    esac
}

install_pkg() {
    local pkg="$1"
    info "Installing $pkg..."
    case "$PKG_MGR" in
        apt)
            DEBIAN_FRONTEND=noninteractive apt-get update -qq && apt-get install -y -qq "$pkg" ;;
        apk)
            apk add --no-cache "$pkg" ;;
        dnf)
            dnf install -y -q "$pkg" ;;
        yum)
            yum install -y -q "$pkg" ;;
        pacman)
            pacman -Sy --noconfirm --needed "$pkg" ;;
        zypper)
            zypper --non-interactive install -y "$pkg" ;;
        xbps)
            xbps-install -Sy "$pkg" ;;
        *)
            error "No package manager available — install '$pkg' manually" ;;
    esac
}

# ═══════════════════════════════════════════════════════════
# Dependency Checker
# ═══════════════════════════════════════════════════════════
check_cmd() {
    local name="$1" pkg_hint="$2"
    if command -v "$name" &>/dev/null; then
        success "Found: $name"
        return 0
    else
        warn "Missing: $name ${pkg_hint:+— try: $pkg_hint}"
        return 1
    fi
}

detect_init_system() {
    if command -v systemctl &>/dev/null && [[ -d /run/systemd/system ]]; then
        INIT="systemd"
        success "Detected init system: systemd"
    elif command -v rc-service &>/dev/null; then
        INIT="openrc"
        success "Detected init system: OpenRC"
    else
        warn "No supported init system detected (systemd or OpenRC)"
        warn "Daemon will NOT be set up — you'll need to start peerman manually"
        INIT="none"
    fi
}

check_arch() {
    local raw
    raw=$(uname -m)
    case "$raw" in
        x86_64)  ARCH="x86_64-unknown-linux-musl" ;;
        aarch64) ARCH="aarch64-unknown-linux-musl" ;;
        *)       ARCH="$raw"
                 warn "Unsupported architecture: $raw (download method may not work)" ;;
    esac
}

install_runtime_deps() {
    local missing=()
    for dep in wg birdc ping traceroute; do
        if ! command -v "$dep" &>/dev/null; then
            missing+=("$dep")
        fi
    done

    if [[ ${#missing[@]} -eq 0 ]]; then
        success "All runtime dependencies satisfied"
        return 0
    fi

    warn "Missing runtime dependencies: ${missing[*]}"

    if [[ -z "$PKG_MGR" ]]; then
        warn "No package manager detected — install manually:"
        for dep in "${missing[@]}"; do
            echo "  $dep → $(dep_to_pkg "$dep")"
        done
        prompt_yn CONTINUE "Continue anyway?" "y"
        [[ "$CONTINUE" != "y" ]] && exit 0
        return 1
    fi

    prompt_yn AUTO_INSTALL "Auto-install missing packages via $PKG_MGR?" "y"
    if [[ "$AUTO_INSTALL" != "y" ]]; then
        warn "Skipping auto-install. Install manually before running peerman."
        return 1
    fi

    local failed=0
    for dep in "${missing[@]}"; do
        local pkg
        pkg=$(dep_to_pkg "$dep")
        if install_pkg "$pkg"; then
            if command -v "$dep" &>/dev/null; then
                success "Installed: $dep (package: $pkg)"
            else
                warn "Package $pkg installed but $dep still not found in PATH"
                failed=$((failed + 1))
            fi
        else
            warn "Failed to install $pkg"
            failed=$((failed + 1))
        fi
    done

    if [[ $failed -gt 0 ]]; then
        warn "$failed package(s) could not be installed"
        prompt_yn CONTINUE "Continue anyway?" "y"
        [[ "$CONTINUE" != "y" ]] && exit 0
    fi
}

run_dependency_check() {
    echo ""
    echo "=== Dependency Check ==="
    echo ""

    detect_init_system
    detect_distro
    check_arch

    echo ""
    info "Runtime dependencies (wg, bird, ping, traceroute):"
    install_runtime_deps

    echo ""
    info "Install-method tools (informational):"
    check_cmd curl "apt install curl / apk add curl" || true
    check_cmd git  "apt install git / apk add git"   || true
    check_cmd cargo "" || true
    check_cmd pnpm ""  || true
}

# ═══════════════════════════════════════════════════════════
# Install Method Selector
# ═══════════════════════════════════════════════════════════
select_install_method() {
    echo ""
    echo "=== Install Method ==="
    echo ""
    echo "  1) Compile from source — cargo build --release (recommended)"
    echo "  2) Download from GitHub Releases"
    echo ""
    prompt METHOD "Choose install method" "1"

    case "${METHOD:-1}" in
        1) INSTALL_METHOD="compile" ;;
        2) INSTALL_METHOD="download" ;;
        *) error "Invalid choice: $METHOD" ;;
    esac
    info "Selected: $INSTALL_METHOD"
}

# ═══════════════════════════════════════════════════════════
# User & Directory Setup
# ═══════════════════════════════════════════════════════════
create_user_and_dirs() {
    echo ""
    echo "=== System Setup ==="

    if id peerman &>/dev/null; then
        info "User 'peerman' already exists"
    else
        useradd --system --shell /usr/sbin/nologin --no-create-home peerman
        success "Created system user: peerman"
    fi

    mkdir -p "$CONFIG_DIR"
    chown root:peerman "$CONFIG_DIR"
    chmod 0750 "$CONFIG_DIR"
    success "Created config directory: $CONFIG_DIR"

    mkdir -p "$DATA_DIR"
    chown peerman:peerman "$DATA_DIR"
    chmod 0750 "$DATA_DIR"
    success "Created data directory: $DATA_DIR"
}

# ═══════════════════════════════════════════════════════════
# Binary Installer
# ═══════════════════════════════════════════════════════════
install_from_release() {
    local version
    info "Fetching latest release version..."
    version=$(curl -sL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
        | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)
    if [[ -z "$version" ]]; then
        error "No releases found at ${GITHUB_REPO}.\n\
       Publish a release first, or use compile-from-source instead."
    fi
    info "Latest version: $version"

    # Release assets are bare binaries named peerman-<triple>
    local binary_name="peerman-${ARCH}"
    local url="${GITHUB_RELEASES}/download/${version}/${binary_name}"

    info "Downloading: $url"
    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf '$tmpdir'" RETURN

    curl -fSL --progress-bar -o "${tmpdir}/peerman" "$url" || {
        error "Download failed. Check:\n\
         - The release asset exists: $url\n\
         - Binary naming: peerman-<triple> (e.g. peerman-x86_64-unknown-linux-musl)\n\
         - Your network connectivity"
    }

    install -o peerman -g peerman -m 0755 "${tmpdir}/peerman" "$BINARY_PATH"
    success "Installed peerman $version to $BINARY_PATH"
}

install_from_source() {
    info "Building from source (cargo build --release)..."
    info "This may take several minutes..."

    local src_dir
    src_dir=$(pwd)
    if [[ ! -f "$src_dir/Cargo.toml" ]]; then
        prompt src_dir "Path to peerman source directory" "$(pwd)"
        if [[ ! -f "$src_dir/Cargo.toml" ]]; then
            error "Cargo.toml not found in $src_dir — not a peerman source tree"
        fi
    fi

    cd "$src_dir"
    cargo build --release || error "Build failed. Check output above."
    install -o peerman -g peerman -m 0755 target/release/peerman "$BINARY_PATH"
    success "Installed peerman (from source) to $BINARY_PATH"
}

install_binary() {
    echo ""
    echo "=== Install Binary ==="

    if [[ "$INSTALL_METHOD" == "download" ]]; then
        install_from_release
    else
        install_from_source
    fi
}

# ═══════════════════════════════════════════════════════════
# Config Generator
# ═══════════════════════════════════════════════════════════
generate_config() {
    echo ""
    echo "=== Configuration ==="
    echo ""
    info "Press Enter to accept defaults (shown in brackets)."
    echo ""

    echo "--- Server ---"
    prompt LISTEN_ADDR "Listen address" "0.0.0.0:3000"
    echo ""

    echo "--- Storage ---"
    prompt DB_PATH "Database path" "/var/lib/peerman/peerman.db"
    echo ""

    echo "--- Logging ---"
    echo "  Options: trace, debug, info, warn, error"
    prompt LOG_LEVEL "Log level" "info"
    echo ""

    echo "--- Auth ---"
    prompt AUTH_USERNAME "Admin username" "admin"
    prompt_password AUTH_PASSWORD "Admin password"
    echo ""
    prompt JWT_SECRET "JWT signing secret (empty = auto-generate on startup)" ""
    echo ""

    echo "--- Cluster (leave node_name empty to run in standalone mode) ---"
    prompt NODE_NAME "Node name (non-empty enables cluster mode)" ""
    echo ""

    if [[ -n "$NODE_NAME" ]]; then
        echo "Cluster mode enabled. Configuring cluster settings..."
        prompt CLUSTER_KEY "Cluster shared secret key (empty = auto-generate 32-char hex)" ""
        if [[ -z "$CLUSTER_KEY" ]]; then
            CLUSTER_KEY=$(head -c 16 /dev/urandom | xxd -p)
            info "Generated cluster key: $CLUSTER_KEY"
        fi
        prompt PEER_NODES "Bootstrap peer nodes (comma-separated host:port)" ""
        prompt TUNNEL_IP_RANGE "Inter-node tunnel IP range" "10.255.0.0/24"
        prompt PROBE_INTERVAL "Probe interval (seconds)" "60"
        prompt SYNC_INTERVAL "Sync interval (seconds)" "30"
    else
        CLUSTER_KEY=""
        PEER_NODES=""
        TUNNEL_IP_RANGE="10.255.0.0/24"
        PROBE_INTERVAL="60"
        SYNC_INTERVAL="30"
    fi
}

format_peer_nodes() {
    # Convert "a:1, b:2" → '"a:1", "b:2"'
    if [[ -z "$PEER_NODES" ]]; then
        echo ""
    else
        local formatted
        formatted=$(echo "$PEER_NODES" | sed 's/[[:space:]]*,[[:space:]]*/", "/g')
        echo "\"$formatted\""
    fi
}

write_config_file() {
    echo ""
    if [[ -f "$CONFIG_FILE" ]]; then
        local backup="${CONFIG_FILE}.bak.$(date +%Y%m%d%H%M%S)"
        cp "$CONFIG_FILE" "$backup"
        warn "Existing config backed up to $backup"
    fi
    info "Writing config to $CONFIG_FILE..."

    local config_content
    config_content=$(cat <<TOML
# Peerman configuration — generated by install script

[server]
listen_addr = "${LISTEN_ADDR}"

[storage]
db_path = "${DB_PATH}"

[logging]
level = "${LOG_LEVEL}"

[auth]
username = "${AUTH_USERNAME}"
password = "${AUTH_PASSWORD}"
jwt_secret = "${JWT_SECRET}"
TOML
)

    if [[ -n "$NODE_NAME" ]]; then
        local peers_toml
        peers_toml=$(format_peer_nodes)
        config_content+=$(cat <<TOML

[cluster]
node_name = "${NODE_NAME}"
cluster_key = "${CLUSTER_KEY}"
peer_nodes = [$peers_toml]
tunnel_ip_range = "${TUNNEL_IP_RANGE}"
probe_interval_secs = ${PROBE_INTERVAL:-60}
sync_interval_secs = ${SYNC_INTERVAL:-30}
TOML
)
    else
        config_content+=$(cat <<'TOML'

# [cluster] — disabled (standalone mode). Set node_name to enable.
# node_name = ""
# cluster_key = ""
# peer_nodes = []
# tunnel_ip_range = "10.255.0.0/24"
# probe_interval_secs = 60
# sync_interval_secs = 30
TOML
)
    fi

    write_atomic "$CONFIG_FILE" "$config_content"
    chown root:peerman "$CONFIG_FILE"
    chmod 0640 "$CONFIG_FILE"
    success "Config written to $CONFIG_FILE"
}

# ═══════════════════════════════════════════════════════════
# Daemon Installer
# ═══════════════════════════════════════════════════════════
install_systemd_service() {
    info "Installing systemd service..."

    local unit_content
    unit_content=$(cat <<'UNIT'
[Unit]
Description=Peerman - DN42 Peer Manager
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=peerman
Group=peerman
ExecStart=/usr/local/bin/peerman -c /etc/peerman/config.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNIT
)

    write_atomic "/etc/systemd/system/peerman.service" "$unit_content"
    systemctl daemon-reload
    success "systemd unit installed: /etc/systemd/system/peerman.service"
}

install_openrc_service() {
    info "Installing OpenRC init script..."

    local init_content
    init_content=$(cat <<'INIT'
#!/sbin/openrc-run
name="peerman"
command="/usr/local/bin/peerman"
command_args="-c /etc/peerman/config.toml"
command_user="peerman"
command_background=false
pidfile="/run/${RC_SVCNAME}.pid"

depend() {
    need net
    after firewall
}
INIT
)

    write_atomic "/etc/init.d/peerman" "$init_content"
    chmod +x /etc/init.d/peerman
    success "OpenRC init installed: /etc/init.d/peerman"
}

install_daemon() {
    echo ""
    echo "=== Daemon Setup ==="

    case "$INIT" in
        systemd)
            install_systemd_service
            ;;
        openrc)
            install_openrc_service
            ;;
        none)
            warn "Skipping daemon installation (no supported init system)"
            warn "Start peerman manually: $BINARY_PATH -c $CONFIG_FILE"
            return
            ;;
    esac
}

# ═══════════════════════════════════════════════════════════
# Sudoers
# ═══════════════════════════════════════════════════════════
install_sudoers() {
    info "Configuring sudoers for peerman user..."

    local sudoers_content
    sudoers_content=$(cat <<'SUDOERS'
# Peerman — minimal privilege escalation for WireGuard & BIRD management
# Managed by install.sh — do not edit by hand
peerman ALL=(root) NOPASSWD: /usr/bin/wg, /usr/sbin/birdc, /usr/bin/ping, /usr/sbin/traceroute
SUDOERS
)

    write_atomic "/etc/sudoers.d/peerman" "$sudoers_content"
    chmod 0440 /etc/sudoers.d/peerman
    success "sudoers installed: /etc/sudoers.d/peerman"
}

# ═══════════════════════════════════════════════════════════
# Service Start & Verify
# ═══════════════════════════════════════════════════════════
start_service() {
    echo ""
    echo "=== Start Service ==="

    case "$INIT" in
        systemd)
            systemctl enable --now peerman
            info "Service enabled and started (systemctl enable --now peerman)"
            echo ""
            info "Waiting for service to start..."
            sleep 2
            if systemctl is-active --quiet peerman; then
                success "Peerman is running"
            else
                warn "Service may not have started correctly"
                echo "  Check logs: journalctl -u peerman -n 50"
            fi
            ;;
        openrc)
            rc-update add peerman
            rc-service peerman start
            info "Service added to runlevel and started"
            echo ""
            info "Waiting for service to start..."
            sleep 2
            if rc-service peerman status &>/dev/null; then
                success "Peerman is running"
            else
                warn "Service may not have started correctly"
                echo "  Check logs: tail /var/log/messages | grep peerman"
            fi
            ;;
        none)
            warn "Skipping service start."
            echo "  Start manually: $BINARY_PATH -c $CONFIG_FILE &"
            return
            ;;
    esac

    local listen_port
    listen_port=$(echo "$LISTEN_ADDR" | sed 's/.*://' || echo "3000")
    if ss -tlnp 2>/dev/null | grep -q ":$listen_port " || \
       netstat -tlnp 2>/dev/null | grep -q ":$listen_port "; then
        success "Port $listen_port is listening"
    else
        warn "Port $listen_port not detected yet (may need a few more seconds)"
    fi
}

# ═══════════════════════════════════════════════════════════
# Success Banner
# ═══════════════════════════════════════════════════════════
print_success_banner() {
    local listen_port
    listen_port=$(echo "$LISTEN_ADDR" | sed 's/.*://' || echo "3000")

    echo ""
    echo "================================================"
    echo ""
    echo -e "  ${GREEN}Peerman installation complete!${NC}"
    echo ""
    echo "  Web UI:  http://localhost:${listen_port}"
    echo "  Config:  $CONFIG_FILE"
    echo "  Binary:  $BINARY_PATH"
    echo "  DB:      $DB_PATH"
    echo ""
    echo "  Service management:"
    case "$INIT" in
        systemd)
            echo "    systemctl status peerman"
            echo "    journalctl -u peerman -f"
            ;;
        openrc)
            echo "    rc-service peerman status"
            echo "    tail -f /var/log/messages | grep peerman"
            ;;
    esac
    echo ""
    echo "================================================"
}

# ═══════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════
main() {
    echo ""
    echo "========================================"
    echo "   Peerman Installation Script"
    echo "   DN42 Peer Manager"
    echo "========================================"

    run_dependency_check
    select_install_method
    create_user_and_dirs
    install_binary
    generate_config
    write_config_file
    install_daemon
    install_sudoers
    start_service
    print_success_banner
}

if [[ -z "${BASH_SOURCE[0]:-}" ]] || [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
