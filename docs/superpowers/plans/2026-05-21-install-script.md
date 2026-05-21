# Peerman Install Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-file interactive bash install script (`install.sh`) that deploys Peerman with daemon setup, privilege minimization, and guided config generation.

**Architecture:** One self-contained bash script with embedded heredoc templates for systemd unit, OpenRC init, and sudoers config. Sequential flow: OS/root check → dep check → install method → user/dirs → binary → interactive config → daemon → sudoers → start → verify.

**Tech Stack:** Bash 4.0+, systemd, OpenRC, sudoers

---

## File Structure

| File | Purpose |
|------|---------|
| `install.sh` (create) | Single self-contained install script, ~450 lines |

The script is organized into clearly delimited sections:
1. Header & preamble (shebang, colors, helpers)
2. OS / privilege guards
3. Dependency checker
4. Install method selector
5. System user & directory setup
6. Binary installer (download + compile branches)
7. Interactive config wizard
8. Daemon installer (systemd + OpenRC templates)
9. sudoers writer
10. Service start & verify
11. `main()` — wires everything together

---

### Task 1: Script skeleton — header, helpers, guards

**Files:**
- Create: `install.sh`

- [ ] **Step 1: Write the preamble section**

```bash
#!/usr/bin/env bash
set -euo pipefail

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

# Prompt with default value (returns user input or default)
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

# Prompt for password (no echo, confirm twice, required)
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

# Prompt yes/no (returns "y" or "n")
prompt_yn() {
    local var_name="$1" prompt_text="$2" default="$3"
    local yn
    read -r -p "$prompt_text [$default]: " yn
    yn="${yn:-$default}"
    yn="${yn,,}"  # lowercase
    eval "$var_name=\$yn"
}

# Atomic write: write to .tmp then mv
write_atomic() {
    local dest="$1" content="$2"
    printf '%s\n' "$content" > "${dest}.tmp"
    mv "${dest}.tmp" "$dest"
}
```

- [ ] **Step 2: Add OS and privilege guards**

```bash
# ── Guards ──────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (needed for: system user creation, /etc writes, service install)"
fi

if [[ "$(uname -s)" != "Linux" ]]; then
    error "Peerman only supports Linux (WG + BIRD kernel interfaces required)"
fi
```

- [ ] **Step 3: Commit**

```bash
git add install.sh
git commit -m "feat(install): add script skeleton with helpers and guards"
```

---

### Task 2: Dependency checker

**Files:**
- Modify: `install.sh` — append after guards section

- [ ] **Step 1: Add dependency check functions**

```bash
# ── Dependency Checker ──────────────────────────────────
DEP_WARNINGS=0

check_cmd() {
    local name="$1" pkg_hint="$2"
    if command -v "$name" &>/dev/null; then
        success "Found: $name"
        return 0
    else
        warn "Missing: $name ${pkg_hint:+— try: $pkg_hint}"
        DEP_WARNINGS=$((DEP_WARNINGS + 1))
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
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64)  ARCH="amd64" ;;
        aarch64) ARCH="arm64" ;;
        *)       ARCH="unsupported"
                 warn "Unsupported architecture: $arch (download method may not work)" ;;
    esac
}

run_dependency_check() {
    echo ""
    echo "=== Dependency Check ==="
    echo ""

    detect_init_system

    echo ""
    info "Checking required runtime tools (warnings only — install manually if missing):"
    check_cmd wg "apt install wireguard-tools / apk add wireguard-tools"
    check_cmd birdc "apt install bird2 / apk add bird"
    check_cmd ping ""  # almost always present
    check_cmd traceroute "apt install traceroute / apk add traceroute"

    echo ""
    info "Checking install-option tools:"
    check_cmd curl ""  # needed for download method
    check_cmd git ""   # needed for compile method
    check_cmd cargo "" # needed for compile method
    check_cmd pnpm ""  # needed for compile method

    check_arch

    if [[ $DEP_WARNINGS -gt 0 ]]; then
        echo ""
        warn "$DEP_WARNINGS tool(s) missing — install them before proceeding."
        prompt_yn CONTINUE "Continue anyway?" "y"
        if [[ "$CONTINUE" != "y" ]]; then
            exit 0
        fi
    fi
}
```

- [ ] **Step 2: Commit**

```bash
git add install.sh && git commit -m "feat(install): add dependency checker"
```

---

### Task 3: Install method selection and system user/directory creation

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add install method selector**

```bash
# ── Install Method Selector ─────────────────────────────
select_install_method() {
    echo ""
    echo "=== Install Method ==="
    echo ""
    echo "  1) Download from GitHub Releases (recommended)"
    echo "  2) Compile from source (cargo build --release)"
    echo ""
    prompt INSTALL_METHOD "Choose install method" "1"

    case "${INSTALL_METHOD:-1}" in
        1) INSTALL_METHOD="download" ;;
        2) INSTALL_METHOD="compile" ;;
        *) error "Invalid choice: $INSTALL_METHOD" ;;
    esac
    info "Selected: $INSTALL_METHOD"
}
```

- [ ] **Step 2: Add user and directory creation**

```bash
# ── User & Directory Setup ──────────────────────────────
BINARY_PATH="/usr/local/bin/peerman"
CONFIG_DIR="/etc/peerman"
DATA_DIR="/var/lib/peerman"
CONFIG_FILE="${CONFIG_DIR}/config.toml"

create_user_and_dirs() {
    echo ""
    echo "=== System Setup ==="

    # Create peerman user if not exists
    if id peerman &>/dev/null; then
        info "User 'peerman' already exists"
    else
        useradd --system --shell /usr/sbin/nologin --no-create-home peerman
        success "Created system user: peerman"
    fi

    # Create config directory
    mkdir -p "$CONFIG_DIR"
    chown root:peerman "$CONFIG_DIR"
    chmod 0750 "$CONFIG_DIR"
    success "Created config directory: $CONFIG_DIR"

    # Create data directory
    mkdir -p "$DATA_DIR"
    chown peerman:peerman "$DATA_DIR"
    chmod 0750 "$DATA_DIR"
    success "Created data directory: $DATA_DIR"
}
```

- [ ] **Step 3: Commit**

```bash
git add install.sh && git commit -m "feat(install): add method selector and user/dir setup"
```

---

### Task 4: Binary installer (download + compile)

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add download install function**

```bash
# ── Binary Installer ─────────────────────────────────────
GITHUB_REPO="${PEERMAN_GITHUB_REPO:-peerman/peerman}"  # override via env var or edit script
GITHUB_RELEASES="https://github.com/${GITHUB_REPO}/releases"

install_from_release() {
    local version="$1"

    if [[ -z "$version" ]]; then
        info "Fetching latest release version..."
        version=$(curl -sL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
            | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)
        if [[ -z "$version" ]]; then
            error "Failed to fetch latest version from GitHub. Try specifying a version manually."
        fi
        info "Latest version: $version"
    fi

    local tarball="peerman-${version}-${ARCH}-unknown-linux-gnu.tar.gz"
    local url="${GITHUB_RELEASES}/download/${version}/${tarball}"

    info "Downloading: $url"
    cd /tmp
    curl -fSL --progress-bar -o "$tarball" "$url" || {
        error "Download failed. Check:"
        echo "  - The release exists at: $url"
        echo "  - Your network connectivity"
        echo "  - Replace GITHUB_REPO in this script with the actual repo name"
    }

    info "Extracting..."
    tar xzf "$tarball"
    install -o peerman -g peerman -m 0755 peerman "$BINARY_PATH"
    rm -f "$tarball" peerman
    cd - > /dev/null
    success "Installed peerman $version to $BINARY_PATH"
}

install_from_source() {
    info "Building from source (cargo build --release)..."
    info "This may take several minutes..."

    # Find source directory
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
        install_from_release ""
    else
        install_from_source
    fi
}
```

- [ ] **Step 2: Commit**

```bash
git add install.sh && git commit -m "feat(install): add binary installer (download + compile)"
```

---

### Task 5: Interactive config wizard

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add config generation function**

```bash
# ── Config Generator ────────────────────────────────────
generate_config() {
    echo ""
    echo "=== Configuration ==="
    echo ""
    info "Press Enter to accept defaults (shown in brackets)."
    echo ""

    # ── Server ──
    echo "--- Server ---"
    prompt LISTEN_ADDR "Listen address" "0.0.0.0:3000"
    echo ""

    # ── Storage ──
    echo "--- Storage ---"
    prompt DB_PATH "Database path" "/var/lib/peerman/peerman.db"
    echo ""

    # ── Logging ──
    echo "--- Logging ---"
    echo "  Options: trace, debug, info, warn, error"
    prompt LOG_LEVEL "Log level" "info"
    echo ""

    # ── Auth ──
    echo "--- Auth ---"
    prompt AUTH_USERNAME "Admin username" "admin"
    prompt_password AUTH_PASSWORD "Admin password"
    echo ""
    prompt JWT_SECRET "JWT signing secret (empty = auto-generate on startup)" ""
    echo ""

    # ── Cluster ──
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
    else
        CLUSTER_KEY=""
        PEER_NODES=""
        TUNNEL_IP_RANGE="10.255.0.0/24"
    fi
}
```

- [ ] **Step 2: Add config file writer with atomic write**

```bash
write_config_file() {
    echo ""
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
        config_content+=$(cat <<TOML

[cluster]
node_name = "${NODE_NAME}"
cluster_key = "${CLUSTER_KEY}"
peer_nodes = [${PEER_NODES:+"$(echo "$PEER_NODES" | sed 's/, */", "/g' | sed 's/^/"/; s/$/"/')"}]
tunnel_ip_range = "${TUNNEL_IP_RANGE}"
TOML
)
    else
        config_content+=$(cat <<TOML

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
```

- [ ] **Step 3: Commit**

```bash
git add install.sh && git commit -m "feat(install): add interactive config wizard"
```

---

### Task 6: Daemon installer — systemd + OpenRC

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add systemd unit installer**

```bash
# ── Daemon Installer ────────────────────────────────────
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
```

- [ ] **Step 2: Add OpenRC init installer**

```bash
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
```

- [ ] **Step 3: Commit**

```bash
git add install.sh && git commit -m "feat(install): add daemon installer (systemd + OpenRC)"
```

---

### Task 7: sudoers, service start, and verification

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add sudoers writer**

```bash
# ── Sudoers ─────────────────────────────────────────────
install_sudoers() {
    info "Configuring sudoers for peerman user..."

    local sudoers_content
    sudoers_content=$(cat <<'SUDOERS'
# Peerman — minimal privilege escalation for WireGuard & BIRD management
# Managed by install.sh — do not edit by hand
peerman ALL=(root) NOPASSWD: /usr/bin/wg
peerman ALL=(root) NOPASSWD: /usr/sbin/birdc
peerman ALL=(root) NOPASSWD: /usr/bin/ping
peerman ALL=(root) NOPASSWD: /usr/sbin/traceroute
SUDOERS
)

    write_atomic "/etc/sudoers.d/peerman" "$sudoers_content"
    chmod 0440 /etc/sudoers.d/peerman
    success "sudoers installed: /etc/sudoers.d/peerman"
}
```

- [ ] **Step 2: Add service start and verification**

```bash
# ── Service Start & Verify ──────────────────────────────
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

    # Verify port is listening
    local listen_port
    listen_port=$(echo "$LISTEN_ADDR" | grep -oP ':\K\d+$' || echo "3000")
    if ss -tlnp | grep -q ":$listen_port "; then
        success "Port $listen_port is listening"
    else
        warn "Port $listen_port not detected yet (may need a few more seconds)"
    fi
}

print_success_banner() {
    local listen_port
    listen_port=$(echo "$LISTEN_ADDR" | grep -oP ':\K\d+$' || echo "3000")

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
```

- [ ] **Step 3: Commit**

```bash
git add install.sh && git commit -m "feat(install): add sudoers, service start, and verification"
```

---

### Task 8: Main function — wire everything together

**Files:**
- Modify: `install.sh` — append

- [ ] **Step 1: Add main() function**

```bash
# ── Main ────────────────────────────────────────────────
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
```

- [ ] **Step 2: Add the invocation at the end of the script**

```bash
# Run unless sourced
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
```

- [ ] **Step 3: Commit**

```bash
git add install.sh && git commit -m "feat(install): wire up main() function"
```

---

### Task 9: Final review — shellcheck, permissions, dry-run

**Files:**
- Modify: `install.sh` — final review pass

- [ ] **Step 1: Run shellcheck**

Run: `shellcheck install.sh`
Expected: zero warnings/errors. Fix any issues found.

- [ ] **Step 2: Syntax check**

Run: `bash -n install.sh`
Expected: no output (no syntax errors).

- [ ] **Step 3: Verify atomic writes and error handling are consistent**

Check each section:
- All `write_atomic` calls use `.tmp` → `mv` pattern ✓
- All heredoc templates use `<<'DELIM'` (quoted) to prevent variable expansion ✓
- `set -euo pipefail` propagates failures ✓
- User is warned before destructive operations ✓

- [ ] **Step 4: Final commit**

```bash
git add install.sh && git commit -m "chore(install): final review pass"
```

---

## Test Plan

Manual test scenarios (executed in a VM or container):

| Scenario | What to verify |
|----------|---------------|
| Fresh install, download method, systemd, standalone | Script completes, service runs, login at `:3000` |
| Fresh install, download method, systemd, cluster mode | Cluster config generated, service runs |
| Fresh install, compile method, OpenRC, standalone | Build succeeds, OpenRC service starts |
| Missing deps (no curl) | Warning shown, doesn't block |
| peerman user already exists | Script handles gracefully |
| Config file already exists | Currently overwrites — add backup step? (future enhancement) |
| Run as non-root | Error message, exit 1 |
| Run on macOS | Error message, exit 1 |
