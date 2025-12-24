#!/usr/bin/env bash
#
# NixOS Configuration Tool
#
# Usage: ./install.sh [command] [options]
#
# Commands:
#   (none)              Interactive menu
#   install [hostname]  Fresh NixOS installation
#   update              Update flake inputs, smart rebuild, update CLI tools
#
# Run fresh installs from the official NixOS minimal ISO.
#
# Steps for fresh install:
#   1. Boot NixOS ISO
#   2. Connect to WiFi: nmtui
#   3. Clone and run:
#      nix-shell -p git --run "git clone https://github.com/DigitalPals/nixos-config.git"
#      cd nixos-config && sudo ./install.sh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Spinner for background processes
spin() {
    local pid=$1 msg=$2
    local spinstr='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    tput civis 2>/dev/null  # Hide cursor
    while kill -0 "$pid" 2>/dev/null; do
        for ((i=0; i<${#spinstr}; i++)); do
            printf "\r  ${BLUE}%s${NC} %s" "${spinstr:$i:1}" "$msg"
            sleep 0.1
        done
    done
    tput cnorm 2>/dev/null  # Show cursor
    printf "\r\033[K"  # Clear the line
}

# Run command quietly with spinner, log output
run_quiet() {
    local msg="$1" log_file="$2"
    shift 2

    "$@" >"$log_file" 2>&1 &
    local pid=$!
    spin "$pid" "$msg"
    wait "$pid"
    local status=$?

    # Always save to main log
    echo "" >> "$UPDATE_LOG"
    echo "=== $msg ===" >> "$UPDATE_LOG"
    cat "$log_file" >> "$UPDATE_LOG"

    if [[ $status -eq 0 ]]; then
        printf "  ${GREEN}✓${NC} %s\n" "$msg"
    else
        printf "  ${RED}✗${NC} %s\n" "$msg"
        echo ""
        cat "$log_file"
    fi
    return $status
}

# Parse flake.lock changes to show version updates
parse_flake_changes() {
    local old_lock="$1" new_lock="$2"
    if command -v jq &>/dev/null && [[ -f "$old_lock" ]] && [[ -f "$new_lock" ]]; then
        local inputs
        inputs=$(jq -r '.nodes | keys[]' "$new_lock" | grep -v root)
        for input in $inputs; do
            local old_rev new_rev
            old_rev=$(jq -r ".nodes.\"$input\".locked.rev // empty" "$old_lock" 2>/dev/null | head -c7)
            new_rev=$(jq -r ".nodes.\"$input\".locked.rev // empty" "$new_lock" 2>/dev/null | head -c7)
            if [[ -n "$old_rev" ]] && [[ -n "$new_rev" ]] && [[ "$old_rev" != "$new_rev" ]]; then
                echo "    $input: $old_rev → $new_rev"
            fi
        done
    fi
}

# Configuration
REPO_URL="https://github.com/DigitalPals/nixos-config.git"
TEMP_CONFIG="/tmp/nixos-config"
CONFIG_DIR="/mnt/home/john/nixos-config"
SYMLINK_PATH="/mnt/etc/nixos"

# Browser backup configuration file
BROWSER_BACKUP_CONFIG="$HOME/.config/browser-backup/config"

# Base hosts (hardware configurations)
BASE_HOSTS=("kraken" "G1a")


# Show usage
usage() {
    echo "NixOS Configuration Tool"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  (none)              Interactive menu"
    echo "  install [hostname]  Fresh NixOS installation"
    echo "  update              Update flake inputs, smart rebuild"
    echo "  browser backup      Backup browser profiles and push to GitHub"
    echo "  browser restore     Pull and restore browser profiles from GitHub"
    echo "  browser status      Check for browser profile updates"
    echo ""
    echo "Examples:"
    echo "  $0                  # Show interactive menu"
    echo "  $0 install          # Install with hostname selection"
    echo "  $0 install kraken   # Install directly to kraken"
    echo "  $0 update           # Update current system"
    echo "  $0 browser backup   # Backup + push browser profiles"
    echo "  $0 browser restore  # Pull + restore browser profiles"
    echo ""
    echo "Available hosts: ${BASE_HOSTS[*]}"
    echo ""
    echo "Desktop shells are selected via the boot menu (specialisations):"
    echo "  - Default (Noctalia), illogical, caelestia"
    echo ""
    exit 1
}

# Show main menu
show_menu() {
    echo ""
    echo "=============================================="
    echo "  NixOS Configuration Tool"
    echo "=============================================="
    echo ""
    echo -e "  ${GREEN}1)${NC} Install NixOS (fresh installation)"
    echo -e "  ${GREEN}2)${NC} Update system (flake inputs + rebuild + CLI tools)"
    echo -e "  ${GREEN}3)${NC} Browser profiles (backup/restore)"
    echo -e "  ${GREEN}4)${NC} Exit"
    echo ""

    while true; do
        read -p "Select option (1-4): " choice
        case $choice in
            1) do_install; break ;;
            2) do_update; break ;;
            3) do_browser_menu; break ;;
            4) echo "Goodbye!"; exit 0 ;;
            *) echo "Invalid option. Please enter 1-4." ;;
        esac
    done
}

# Interactive hostname selection (selects base host)
select_hostname() {
    echo ""
    log_info "Available hosts:"
    echo ""
    echo -e "  ${GREEN}1)${NC} kraken    - Desktop with NVIDIA RTX 5090"
    echo -e "  ${GREEN}2)${NC} G1a       - HP ZBook Ultra G1a (AMD Strix Halo)"
    echo ""

    while true; do
        read -p "Select host (1-2): " choice
        case $choice in
            1) BASE_HOST="kraken"; break ;;
            2) BASE_HOST="G1a"; break ;;
            *) echo "Invalid selection. Please enter 1 or 2." ;;
        esac
    done

    log_success "Selected host: $BASE_HOST"
}

# Resolve current host configuration (shared by update)
# With specialisations, config name is always just the hostname
resolve_current_config() {
    BASE_HOST=$(hostname)
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    if ! validate_base_host "$BASE_HOST"; then
        log_error "Unknown hostname: $BASE_HOST. Expected one of: ${BASE_HOSTS[*]}"
    fi

    # Config name is always the base hostname (specialisations handle shell variants)
    CONFIG_NAME="$BASE_HOST"
}

# Get list of available disks (excluding loop, ram, rom, zram devices)
get_available_disks() {
    lsblk -dno NAME,SIZE,MODEL,TYPE 2>/dev/null | \
        awk '$NF == "disk" {
            name = "/dev/" $1
            size = $2
            # Get model (all fields between size and type)
            model = ""
            for (i = 3; i < NF; i++) model = model (model ? " " : "") $i
            print name, size, model
        }' | \
        grep -v "^/dev/loop" | \
        grep -v "^/dev/ram" | \
        grep -v "^/dev/zram" | \
        grep -v "^/dev/sr" | \
        grep -v "^/dev/fd"
}

# Interactive disk selection
select_disk() {
    echo ""
    log_info "Available disks:"
    echo ""

    local disks=()
    local i=1

    while IFS= read -r line; do
        local disk=$(echo "$line" | awk '{print $1}')
        local size=$(echo "$line" | awk '{print $2}')
        local model=$(echo "$line" | cut -d' ' -f3-)

        disks+=("$disk")
        printf "  ${GREEN}%d)${NC} %-15s %8s  %s\n" "$i" "$disk" "$size" "$model"
        ((i++))
    done < <(get_available_disks)

    if [[ ${#disks[@]} -eq 0 ]]; then
        log_error "No suitable disks found"
    fi

    echo ""

    if [[ ${#disks[@]} -eq 1 ]]; then
        log_info "Only one disk available: ${disks[0]}"
        DISK_DEVICE="${disks[0]}"
    else
        while true; do
            read -p "Select disk (1-${#disks[@]}): " choice
            if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#disks[@]} )); then
                DISK_DEVICE="${disks[$((choice-1))]}"
                break
            else
                echo "Invalid selection. Please enter a number between 1 and ${#disks[@]}"
            fi
        done
    fi

    log_success "Selected disk: $DISK_DEVICE"
}

# Validate base hostname (hardware config)
validate_base_host() {
    local host="$1"
    for valid in "${BASE_HOSTS[@]}"; do
        if [[ "$host" == "$valid" ]]; then
            return 0
        fi
    done
    return 1
}

# ============================================================================
# Browser Profile Backup/Restore Functions
# ============================================================================

# Load browser backup configuration
load_browser_config() {
    if [[ ! -f "$BROWSER_BACKUP_CONFIG" ]]; then
        log_error "Browser backup not configured. Enable it in home.nix and rebuild."
    fi
    # shellcheck source=/dev/null
    source "$BROWSER_BACKUP_CONFIG"

    # Validate required config
    : "${BROWSER_BACKUP_REPO:?BROWSER_BACKUP_REPO not set in config}"
    : "${AGE_RECIPIENT:?AGE_RECIPIENT not set in config}"
    : "${LOCAL_REPO_PATH:=$HOME/.local/share/browser-backup}"
    : "${BACKUP_RETENTION:=3}"

    # Check for 1Password or file-based key
    USE_1PASSWORD=false
    if [[ -n "${AGE_KEY_1PASSWORD:-}" ]]; then
        USE_1PASSWORD=true
    elif [[ -z "${AGE_KEY_PATH:-}" ]]; then
        log_error "Neither AGE_KEY_1PASSWORD nor AGE_KEY_PATH is set in config"
    fi

    # Expand ~ in paths
    if [[ -n "${AGE_KEY_PATH:-}" ]]; then
        AGE_KEY_PATH="${AGE_KEY_PATH/#\~/$HOME}"
    fi
    LOCAL_REPO_PATH="${LOCAL_REPO_PATH/#\~/$HOME}"
}

# Browser profiles menu
do_browser_menu() {
    echo ""
    echo "=============================================="
    echo "  Browser Profiles"
    echo "=============================================="
    echo ""
    echo -e "  ${GREEN}1)${NC} Backup & push to GitHub"
    echo -e "  ${GREEN}2)${NC} Pull & restore from GitHub"
    echo -e "  ${GREEN}3)${NC} Check for updates"
    echo -e "  ${GREEN}4)${NC} Back to main menu"
    echo ""

    while true; do
        read -p "Select option (1-4): " choice
        case $choice in
            1) do_browser_backup; break ;;
            2) do_browser_restore; break ;;
            3) do_browser_status; break ;;
            4) show_menu; break ;;
            *) echo "Invalid option. Please enter 1-4." ;;
        esac
    done
}

# Backup browser profiles
do_browser_backup() {
    local force="${1:-false}"

    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run as root. Run as normal user."
    fi

    # Check for the Home Manager browser-backup script
    if ! command -v browser-backup &>/dev/null; then
        log_error "browser-backup not found. Enable browser-backup in home.nix and rebuild."
    fi

    # Build arguments
    local args=("--push")
    if [[ "$force" == "true" ]]; then
        args+=("--force")
    fi

    # Delegate to Home Manager script (handles minimal backup)
    browser-backup "${args[@]}"
}

# Restore browser profiles
do_browser_restore() {
    local force="${1:-false}"

    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run as root. Run as normal user."
    fi

    # Check for the Home Manager browser-restore script
    if ! command -v browser-restore &>/dev/null; then
        log_error "browser-restore not found. Enable browser-backup in home.nix and rebuild."
    fi

    # Build arguments
    local args=("--pull")
    if [[ "$force" == "true" ]]; then
        args+=("--force")
    fi

    # Delegate to Home Manager script (handles merge-based restore)
    browser-restore "${args[@]}"
}

# Check for browser profile updates
do_browser_status() {
    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run as root. Run as normal user."
    fi

    echo ""
    echo "=============================================="
    echo "  Browser Profile Status"
    echo "=============================================="
    echo ""

    load_browser_config

    if [[ ! -d "$LOCAL_REPO_PATH/.git" ]]; then
        log_info "Local repository not found. Run restore to clone."
        return
    fi

    cd "$LOCAL_REPO_PATH"

    # Ensure origin exists before fetching
    if ! git remote get-url origin >/dev/null 2>&1; then
        log_warn "No remote 'origin' configured for $LOCAL_REPO_PATH"
        echo ""
        echo "Local files:"
        ls -lh ./*.age 2>/dev/null || echo "  (no backup files)"
        echo ""
        return
    fi

    # Fetch without merging (be resilient to offline/missing-remote cases)
    log_info "Checking for updates..."
    if ! git fetch origin >/dev/null 2>&1; then
        log_warn "Unable to reach remote; showing local status only"
        echo ""
        echo "Local files:"
        ls -lh ./*.age 2>/dev/null || echo "  (no backup files)"
        echo ""
        return
    fi

    # Compare local and remote
    LOCAL_HEAD=$(git rev-parse HEAD)
    REMOTE_HEAD=$(git rev-parse origin/main 2>/dev/null || git rev-parse origin/master 2>/dev/null)
    if [[ -z "$REMOTE_HEAD" ]]; then
        log_warn "Remote branch not found (origin/main or origin/master)"
        echo ""
        echo "Local files:"
        ls -lh ./*.age 2>/dev/null || echo "  (no backup files)"
        echo ""
        return
    fi

    if [[ "$LOCAL_HEAD" == "$REMOTE_HEAD" ]]; then
        log_success "Browser profiles are up to date"
    else
        log_warn "Remote has newer profiles"
        echo ""
        echo "Remote commits:"
        git log --oneline "$LOCAL_HEAD..$REMOTE_HEAD"
        echo ""
        log_info "Run './install.sh browser restore' to update"
    fi

    echo ""
    echo "Local files:"
    ls -lh ./*.age 2>/dev/null || echo "  (no backup files)"
    echo ""
}

# ============================================================================
# End Browser Profile Functions
# ============================================================================

# Update system (git pull + flake update + smart rebuild + CLI tools)
do_update() {
    # Don't run as root (git operations should be as normal user)
    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run update as root. Run as normal user (sudo is used only for rebuild)."
    fi

    resolve_current_config  # Sets BASE_HOST, SCRIPT_DIR, CONFIG_NAME

    echo ""
    echo "=============================================="
    echo "  NixOS System Update"
    echo "=============================================="
    echo ""

    cd "$SCRIPT_DIR"

    # Initialize logging
    UPDATE_LOG="$HOME/update.log"
    LOG_DIR=$(mktemp -d)
    trap "rm -rf $LOG_DIR; tput cnorm 2>/dev/null" EXIT
    : > "$UPDATE_LOG"  # Clear log
    echo "NixOS Update - $(date)" >> "$UPDATE_LOG"
    echo "Host: $BASE_HOST" >> "$UPDATE_LOG"
    echo "Config: $CONFIG_NAME" >> "$UPDATE_LOG"

    # Track what was updated for summary
    FLAKE_CHANGES=""
    CLAUDE_OLD=""
    CLAUDE_NEW=""
    CODEX_OLD=""
    CODEX_NEW=""

    # Save current system for comparison
    OLD_SYSTEM=$(readlink -f /run/current-system)
    NEEDS_REBUILD=false

    # Save flake.lock before update for comparison
    cp flake.lock "$LOG_DIR/flake.lock.old" 2>/dev/null || true

    # Step 1: Flake update
    LOCK_BEFORE=$(sha256sum flake.lock 2>/dev/null | cut -d' ' -f1 || echo "")
    if ! run_quiet "Updating flake inputs" "$LOG_DIR/flake.log" nix flake update; then
        log_error "Flake update failed. Check $UPDATE_LOG"
    fi
    LOCK_AFTER=$(sha256sum flake.lock 2>/dev/null | cut -d' ' -f1 || echo "")

    if [[ "$LOCK_BEFORE" != "$LOCK_AFTER" ]]; then
        NEEDS_REBUILD=true
        FLAKE_CHANGES=$(parse_flake_changes "$LOG_DIR/flake.lock.old" flake.lock)
    fi

    # Step 2: Rebuild (only if needed)
    REBUILD_FAILED=false
    if [[ "$NEEDS_REBUILD" == "true" ]]; then
        if ! run_quiet "Rebuilding system" "$LOG_DIR/rebuild.log" sudo nixos-rebuild switch --flake ".#${CONFIG_NAME}"; then
            REBUILD_FAILED=true
        fi
    else
        printf "  ${BLUE}-${NC} Skipping rebuild (no changes)\n"
    fi

    # Step 3: Update Claude Code
    if [[ -x "$HOME/.local/bin/claude" ]]; then
        CLAUDE_OLD=$("$HOME/.local/bin/claude" --version 2>/dev/null | head -1 || echo "")
        run_quiet "Updating Claude Code" "$LOG_DIR/claude.log" "$HOME/.local/bin/claude" update || true
        CLAUDE_NEW=$("$HOME/.local/bin/claude" --version 2>/dev/null | head -1 || echo "")
    else
        printf "  ${BLUE}-${NC} Claude Code not installed\n"
    fi

    # Step 4: Update Codex CLI
    if [[ -x "$HOME/.npm-global/bin/codex" ]]; then
        CODEX_OLD=$(npm list -g @openai/codex 2>/dev/null | grep -o '@[0-9.]*' | tail -1 | tr -d '@' || echo "")
        run_quiet "Updating Codex CLI" "$LOG_DIR/codex.log" npm update -g @openai/codex || true
        CODEX_NEW=$(npm list -g @openai/codex 2>/dev/null | grep -o '@[0-9.]*' | tail -1 | tr -d '@' || echo "")
    else
        printf "  ${BLUE}-${NC} Codex CLI not installed\n"
    fi

    # Get new system path for comparison
    NEW_SYSTEM=$(readlink -f /run/current-system)

    # Helper to clean version strings (remove duplicate labels like "2.0.75 (Claude Code)")
    clean_version() {
        echo "$1" | sed 's/ (Claude Code)//; s/ (Codex)//'
    }

    # Summary section
    echo ""
    echo "=============================================="
    echo "  Update Summary"
    echo "=============================================="

    # Flake inputs
    if [[ -n "$FLAKE_CHANGES" ]]; then
        echo ""
        echo -e "  ${GREEN}Flake inputs updated:${NC}"
        echo "$FLAKE_CHANGES" | sed 's/^  //'
    fi

    # Show dots-hyprland version (used by illogical specialisation)
    if command -v jq &>/dev/null; then
        DOTS_REV=$(jq -r '.nodes."dots-hyprland".locked.rev // empty' flake.lock 2>/dev/null | head -c7)
        DOTS_DATE=$(jq -r '.nodes."dots-hyprland".locked.lastModified // empty' flake.lock 2>/dev/null)
        if [[ -n "$DOTS_REV" ]] && [[ -n "$DOTS_DATE" ]]; then
            DOTS_DATE_FMT=$(date -d "@$DOTS_DATE" "+%Y-%m-%d" 2>/dev/null || echo "")
            echo "    dots-hyprland: $DOTS_REV ($DOTS_DATE_FMT)"
        elif [[ -n "$DOTS_REV" ]]; then
            echo "    dots-hyprland: $DOTS_REV"
        fi
    fi

    # CLI tools section
    CLAUDE_UPDATED=false
    CODEX_UPDATED=false
    if [[ -n "$CLAUDE_OLD" ]] && [[ "$CLAUDE_OLD" != "$CLAUDE_NEW" ]] && [[ -n "$CLAUDE_NEW" ]]; then
        CLAUDE_UPDATED=true
    fi
    if [[ -n "$CODEX_OLD" ]] && [[ "$CODEX_OLD" != "$CODEX_NEW" ]] && [[ -n "$CODEX_NEW" ]]; then
        CODEX_UPDATED=true
    fi

    if [[ "$CLAUDE_UPDATED" == "true" ]] || [[ "$CODEX_UPDATED" == "true" ]]; then
        echo ""
        echo -e "  ${GREEN}CLI tools updated:${NC}"
        if [[ "$CLAUDE_UPDATED" == "true" ]]; then
            echo "    Claude Code: $(clean_version "$CLAUDE_OLD") → $(clean_version "$CLAUDE_NEW")"
        fi
        if [[ "$CODEX_UPDATED" == "true" ]]; then
            echo "    Codex CLI: $CODEX_OLD → $CODEX_NEW"
        fi
    fi

    # Package changes - parse nvd output for cleaner display
    if [[ "$OLD_SYSTEM" != "$NEW_SYSTEM" ]]; then
        NVD_OUTPUT=$(nix run nixpkgs#nvd -- diff "$OLD_SYSTEM" "$NEW_SYSTEM" 2>/dev/null || true)

        # Extract version changes - one package per line with clean version
        # Only show packages where the version actually changed
        VERSION_CHANGES=$(echo "$NVD_OUTPUT" | grep -E '^\[' | while read -r line; do
            # Parse: [A.]  #1  packagename  version -> version
            pkg=$(echo "$line" | awk '{print $3}')
            # Get everything after the package name for version info
            versions=$(echo "$line" | sed "s/.*$pkg  *//")
            # Extract just the first old and new version (before any comma)
            old_ver=$(echo "$versions" | sed 's/ *->.*//; s/,.*//' | sed 's/2025-[0-9-]*_//')
            new_ver=$(echo "$versions" | sed 's/.*-> *//; s/,.*//' | sed 's/2025-[0-9-]*_//')
            # Only output if versions differ
            if [[ "$old_ver" != "$new_ver" ]]; then
                echo "    $pkg: $old_ver → $new_ver"
            fi
        done)

        # Extract closure stats
        CLOSURE_LINE=$(echo "$NVD_OUTPUT" | grep "Closure size:" || true)
        if [[ -n "$CLOSURE_LINE" ]]; then
            PATHS_ADDED=$(echo "$CLOSURE_LINE" | grep -oP '\d+ paths added' || true)
            PATHS_REMOVED=$(echo "$CLOSURE_LINE" | grep -oP '\d+ paths removed' || true)
            DISK_DELTA=$(echo "$CLOSURE_LINE" | grep -oP 'disk usage [^)]+' | sed 's/disk usage //' || true)
        fi

        if [[ -n "$VERSION_CHANGES" ]]; then
            echo ""
            echo -e "  ${GREEN}Packages changed:${NC}"
            echo "$VERSION_CHANGES"
        fi

        if [[ -n "$PATHS_ADDED" ]] || [[ -n "$PATHS_REMOVED" ]] || [[ -n "$DISK_DELTA" ]]; then
            echo ""
            echo "  Closure:"
            [[ -n "$PATHS_ADDED" ]] && echo "    $PATHS_ADDED"
            [[ -n "$PATHS_REMOVED" ]] && echo "    $PATHS_REMOVED"
            [[ -n "$DISK_DELTA" ]] && echo "    disk: $DISK_DELTA"
        fi
    fi

    # Status line (only show items that weren't updated above)
    echo ""
    echo "  ─────────────────────────────────────────"
    echo ""

    # System status
    if [[ "$REBUILD_FAILED" == "true" ]]; then
        echo -e "  System:      ${RED}Rebuild failed${NC}"
    elif [[ "$OLD_SYSTEM" == "$NEW_SYSTEM" ]] && [[ -z "$FLAKE_CHANGES" ]]; then
        echo "  System:      Already up to date"
    fi

    # Show versions that weren't updated
    if [[ -n "$CLAUDE_OLD" ]] && [[ "$CLAUDE_UPDATED" != "true" ]]; then
        echo "  Claude Code: $(clean_version "$CLAUDE_OLD")"
    fi
    if [[ -n "$CODEX_OLD" ]] && [[ "$CODEX_UPDATED" != "true" ]]; then
        echo "  Codex CLI:   $CODEX_OLD"
    fi

    echo ""
    echo "  Log: $UPDATE_LOG"
    echo ""
    echo "=============================================="
}

# Fresh NixOS installation
do_install() {
    # If base host not set, prompt for selection
    if [[ -z "${BASE_HOST:-}" ]]; then
        select_hostname
    fi

    # Validate base hostname
    if ! validate_base_host "$BASE_HOST"; then
        log_error "Invalid hostname: $BASE_HOST. Must be one of: ${BASE_HOSTS[*]}"
    fi

    # Config name is the hostname (specialisations handle shell variants)
    CONFIG_NAME="$BASE_HOST"

    # Check if running as root
    if [[ $EUID -ne 0 ]]; then
        log_error "Installation must be run as root. Try: sudo $0 install $BASE_HOST"
    fi

    # If no disk specified, show interactive selector
    if [[ -z "${DISK_DEVICE:-}" ]]; then
        select_disk
    fi

    # Check if disk exists
    if [[ ! -b "$DISK_DEVICE" ]]; then
        log_error "Disk device $DISK_DEVICE does not exist. Use 'lsblk' to list available disks."
    fi

    # Display configuration
    echo ""
    echo "=============================================="
    echo "  NixOS Installation with Disko"
    echo "=============================================="
    echo ""
    log_info "Target host: $BASE_HOST"
    log_info "Configuration: $CONFIG_NAME"
    log_info "Desktop shells: Noctalia (default), Illogical, Caelestia (via boot menu)"
    log_info "Target disk: $DISK_DEVICE"
    echo ""
    lsblk "$DISK_DEVICE"
    echo ""
    log_warn "WARNING: This will ERASE ALL DATA on $DISK_DEVICE!"
    echo ""
    read -p "Type 'yes' to continue: " confirm
    if [[ "$confirm" != "yes" ]]; then
        log_error "Installation cancelled by user"
    fi

    # Step 1: Check network connectivity
    echo ""
    log_info "Step 1/6: Checking network connectivity..."
    if ! ping -c 1 -W 5 github.com &>/dev/null; then
        log_warn "No network connection detected"
        echo ""
        echo "Please connect to WiFi using: nmtui"
        echo "Or connect ethernet cable"
        echo ""
        read -p "Press Enter when connected..."

        if ! ping -c 1 -W 5 github.com &>/dev/null; then
            log_error "Still no network connection. Please connect and try again."
        fi
    fi
    log_success "Network connected"

    # Step 2: Enable flakes
    log_info "Step 2/6: Enabling Nix flakes..."
    export NIX_CONFIG="experimental-features = nix-command flakes"
    log_success "Flakes enabled"

    # Step 3: Clone configuration repository
    log_info "Step 3/6: Cloning configuration repository..."
    rm -rf "$TEMP_CONFIG"
    nix-shell -p git --run "git clone --depth 1 $REPO_URL $TEMP_CONFIG"
    log_success "Configuration cloned to $TEMP_CONFIG"

    # Step 4: Update disk device in disko configuration
    log_info "Step 4/6: Configuring disk device..."
    DISKO_HOST_FILE="$TEMP_CONFIG/modules/disko/${BASE_HOST}.nix"
    if [[ -f "$DISKO_HOST_FILE" ]]; then
        sed -i "s|device = \"/dev/[^\"]*\"|device = \"$DISK_DEVICE\"|" "$DISKO_HOST_FILE"
        log_success "Updated disk device to $DISK_DEVICE in $DISKO_HOST_FILE"
    else
        log_error "Disko configuration not found: $DISKO_HOST_FILE"
    fi

    # Step 5: Run disko to partition and format
    log_info "Step 5/6: Running disko to partition and format..."

    # Pre-fetch disko to ensure stable TTY when prompting for LUKS passphrase
    log_info "Preparing disko (this may take a moment on first run)..."
    nix build "$TEMP_CONFIG#disko" --no-link 2>/dev/null || true

    echo ""
    log_warn "You will be prompted to enter the LUKS encryption passphrase."
    log_warn "Choose a strong passphrase - you will need it every time you boot."
    echo ""
    read -p "Press Enter to continue with disk partitioning..."

    nix run "$TEMP_CONFIG#disko" -- \
        --mode destroy,format,mount \
        --flake "$TEMP_CONFIG#$BASE_HOST"

    log_success "Disk partitioned and mounted at /mnt"

    # Step 6: Install NixOS
    log_info "Step 6/6: Installing NixOS..."

    # Copy configuration to user home directory
    mkdir -p "$CONFIG_DIR"
    cp -r "$TEMP_CONFIG"/* "$CONFIG_DIR/"
    rm -rf "$CONFIG_DIR/.git"

    # Create symlink from /etc/nixos
    mkdir -p "$(dirname "$SYMLINK_PATH")"
    ln -sf /home/john/nixos-config "$SYMLINK_PATH"

    # Initialize git repo with remote origin (as root, then fix ownership)
    cd "$CONFIG_DIR"
    nix-shell -p git --run "git init -b main && git remote add origin $REPO_URL && git add -A && git -c user.name='NixOS Install' -c user.email='install@localhost' commit -m 'Initial configuration' && git fetch origin && git branch --set-upstream-to=origin/main main"
    cd - >/dev/null

    # Set ownership to john (uid 1000, gid 100)
    chown 1000:100 "$(dirname "$CONFIG_DIR")"
    chown -R 1000:100 "$CONFIG_DIR"

    log_info "Configuration installed to $CONFIG_DIR"
    log_info "Running nixos-install (this may take a while)..."
    echo ""

    nixos-install --flake "${CONFIG_DIR}#${CONFIG_NAME}" --no-root-passwd

    # Installation complete
    echo ""
    echo "=============================================="
    echo "  Installation Complete!"
    echo "=============================================="
    echo ""
    echo "Next steps:"
    echo "  1. Reboot: reboot"
    echo "  2. Enter your LUKS passphrase at boot"
    echo "  3. In the boot menu, select a shell:"
    echo "     - Default (Noctalia)"
    echo "     - illogical (Illogical Impulse)"
    echo "     - caelestia (Caelestia)"
    echo "  4. You will be auto-logged in as 'john'"
    echo "  5. Set your user password: passwd"
    echo "  6. Your config is at ~/nixos-config (symlinked from /etc/nixos)"
    echo ""
    echo "Your LUKS passphrase will be required at every boot."
    echo "To switch shells, reboot and select from the boot menu."
    echo "=============================================="
}

# Parse command line arguments
COMMAND="${1:-}"
ARG2="${2:-}"
ARG3="${3:-}"

case "$COMMAND" in
    "")
        show_menu
        ;;
    "install")
        BASE_HOST="$ARG2"
        DISK_DEVICE="$ARG3"
        do_install
        ;;
    "update")
        do_update
        ;;
    "browser")
        # Check for --force flag
        BROWSER_FORCE=false
        [[ "$ARG3" == "--force" || "$ARG3" == "-f" ]] && BROWSER_FORCE=true

        case "$ARG2" in
            "backup")
                do_browser_backup "$BROWSER_FORCE"
                ;;
            "restore")
                do_browser_restore "$BROWSER_FORCE"
                ;;
            "status")
                do_browser_status
                ;;
            *)
                do_browser_menu
                ;;
        esac
        ;;
    "-h"|"--help"|"help")
        usage
        ;;
    *)
        # Legacy: treat first arg as hostname for backwards compatibility
        # e.g., ./install.sh kraken /dev/nvme0n1
        if validate_base_host "$COMMAND"; then
            BASE_HOST="$COMMAND"
            DISK_DEVICE="$ARG2"
            do_install
        else
            echo "Unknown command: $COMMAND"
            echo ""
            usage
        fi
        ;;
esac
