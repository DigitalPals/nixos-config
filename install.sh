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

# Base hosts (hardware configurations)
BASE_HOSTS=("kraken" "G1a")

# Shell definitions: name -> "display|description"
declare -A SHELL_INFO=(
  ["noctalia"]="Noctalia|Modern Qt6/QML desktop shell (default)"
  ["illogical"]="Illogical Impulse|Material Design 3 Quickshell-based shell"
  ["caelestia"]="Caelestia|Feature-rich Quickshell desktop environment"
)
SHELLS=("noctalia" "illogical" "caelestia")

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
    echo "  switch [shell]      Switch desktop shell"
    echo ""
    echo "Examples:"
    echo "  $0                  # Show interactive menu"
    echo "  $0 install          # Install with hostname selection"
    echo "  $0 install kraken   # Install directly to kraken"
    echo "  $0 update           # Update current system"
    echo "  $0 switch illogical # Switch to Illogical Impulse shell"
    echo ""
    echo "Available hosts: ${BASE_HOSTS[*]}"
    echo "Available shells: ${SHELLS[*]}"
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
    echo -e "  ${GREEN}3)${NC} Switch desktop shell"
    echo -e "  ${GREEN}4)${NC} Exit"
    echo ""

    while true; do
        read -p "Select option (1-4): " choice
        case $choice in
            1) do_install; break ;;
            2) do_update; break ;;
            3) do_switch_shell; break ;;
            4) echo "Goodbye!"; exit 0 ;;
            *) echo "Invalid option. Please enter 1, 2, 3, or 4." ;;
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

# Interactive shell selection (dynamic from SHELL_INFO)
select_shell() {
    echo ""
    log_info "Available desktop shells:"
    echo ""

    local i=1
    for shell_name in "${SHELLS[@]}"; do
        IFS='|' read -r display desc <<< "${SHELL_INFO[$shell_name]}"
        printf "  ${GREEN}%d)${NC} %-20s - %s\n" "$i" "$display" "$desc"
        ((i++))
    done
    echo ""

    while true; do
        read -p "Select shell (1-${#SHELLS[@]}) [1]: " choice
        choice=${choice:-1}
        if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#SHELLS[@]} )); then
            SHELL_CHOICE="${SHELLS[$((choice-1))]}"
            break
        else
            echo "Invalid selection. Please enter 1-${#SHELLS[@]}."
        fi
    done

    log_success "Selected shell: $SHELL_CHOICE"
}

# Build full config name from base host + shell
get_config_name() {
    local host="$1"
    local shell="$2"
    if [[ "$shell" == "noctalia" ]]; then
        echo "$host"
    else
        echo "${host}-${shell}"
    fi
}

# Detect current shell from runtime file
# The active desktop shell writes its name to $XDG_RUNTIME_DIR/desktop-shell at startup
# Sets CURRENT_SHELL and SHELL_DETECTED variables directly (not via subshell)
detect_current_shell() {
    local runtime_file="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/desktop-shell"
    if [[ -f "$runtime_file" ]]; then
        SHELL_DETECTED=true
        CURRENT_SHELL=$(cat "$runtime_file")
    else
        # Default to noctalia if no runtime file exists (not in a graphical session)
        SHELL_DETECTED=false
        CURRENT_SHELL="noctalia"
    fi
}

# Resolve current host configuration (shared by switch/update)
resolve_current_config() {
    BASE_HOST=$(hostname)
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    if ! validate_base_host "$BASE_HOST"; then
        log_error "Unknown hostname: $BASE_HOST. Expected one of: ${BASE_HOSTS[*]}"
    fi

    detect_current_shell  # Sets CURRENT_SHELL and SHELL_DETECTED
    CONFIG_NAME=$(get_config_name "$BASE_HOST" "$CURRENT_SHELL")
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

# Validate shell name
validate_shell() {
    local shell="$1"
    for valid in "${SHELLS[@]}"; do
        if [[ "$shell" == "$valid" ]]; then
            return 0
        fi
    done
    return 1
}

# Switch desktop shell
do_switch_shell() {
    # Don't run as root
    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run as root. Run as normal user (sudo is used only for rebuild)."
    fi

    resolve_current_config  # Sets BASE_HOST, SCRIPT_DIR, CURRENT_SHELL, CONFIG_NAME

    echo ""
    echo "=============================================="
    echo "  Switch Desktop Shell"
    echo "=============================================="
    echo ""
    log_info "Host: $BASE_HOST"
    if [[ "$SHELL_DETECTED" == "true" ]]; then
        log_info "Current shell: $CURRENT_SHELL"
    else
        log_warn "Current shell: $CURRENT_SHELL (assumed - not in graphical session)"
    fi
    echo ""

    # If shell was passed as argument, use it; otherwise prompt
    if [[ -n "${SHELL_CHOICE:-}" ]]; then
        if ! validate_shell "$SHELL_CHOICE"; then
            log_error "Invalid shell: $SHELL_CHOICE. Must be one of: ${SHELLS[*]}"
        fi
    else
        select_shell
    fi

    # Check if already using this shell (only skip if we're certain about detection)
    if [[ "$SHELL_DETECTED" == "true" ]] && [[ "$SHELL_CHOICE" == "$CURRENT_SHELL" ]]; then
        log_info "Already using $SHELL_CHOICE. No changes needed."
        exit 0
    fi

    # Build new config name
    CONFIG_NAME=$(get_config_name "$BASE_HOST" "$SHELL_CHOICE")

    echo ""
    log_info "Switching to: $CONFIG_NAME"
    echo ""

    cd "$SCRIPT_DIR"

    # Rebuild with new configuration
    log_info "Rebuilding system with $SHELL_CHOICE shell..."
    if sudo nixos-rebuild switch --flake ".#${CONFIG_NAME}"; then
        echo ""
        log_success "Shell switched to $SHELL_CHOICE!"
        echo ""

        # Reboot required for shell switch to take full effect
        log_info "A reboot is required for the shell switch to take full effect."
        echo ""
        read -p "Reboot now? (Y/n): " reboot_choice
        if [[ ! "$reboot_choice" =~ ^[Nn]$ ]]; then
            echo ""
            log_info "Rebooting in 2 seconds..."
            sleep 2
            sudo reboot
        else
            echo ""
            log_warn "Remember to reboot for the shell switch to take effect."
            log_warn "Do NOT reload Hyprland config (hyprctl reload) - it will break keybindings."
        fi
    else
        log_error "Rebuild failed. Check the output above for errors."
    fi
}

# Update system (git pull + flake update + smart rebuild + CLI tools)
do_update() {
    # Don't run as root (git operations should be as normal user)
    if [[ $EUID -eq 0 ]]; then
        log_error "Don't run update as root. Run as normal user (sudo is used only for rebuild)."
    fi

    resolve_current_config  # Sets BASE_HOST, SCRIPT_DIR, CURRENT_SHELL, CONFIG_NAME

    echo ""
    echo "=============================================="
    echo "  NixOS System Update"
    echo "=============================================="
    echo ""

    # Warn if shell detection is assumed
    if [[ "$SHELL_DETECTED" != "true" ]]; then
        log_warn "Shell: $CURRENT_SHELL (assumed - not in graphical session)"
        log_warn "Config: $CONFIG_NAME"
        echo ""
    fi

    cd "$SCRIPT_DIR"

    # Initialize logging
    UPDATE_LOG="$HOME/update.log"
    LOG_DIR=$(mktemp -d)
    trap "rm -rf $LOG_DIR; tput cnorm 2>/dev/null" EXIT
    : > "$UPDATE_LOG"  # Clear log
    echo "NixOS Update - $(date)" >> "$UPDATE_LOG"
    echo "Host: $BASE_HOST (shell: $CURRENT_SHELL)" >> "$UPDATE_LOG"
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

    # Summary section
    echo ""
    echo "=============================================="
    echo "  Update Summary"
    echo "=============================================="
    echo ""

    # Flake inputs
    if [[ -n "$FLAKE_CHANGES" ]]; then
        echo -e "  Flake inputs:   ${GREEN}Updated${NC}"
        echo "$FLAKE_CHANGES"
    else
        echo "  Flake inputs:   Up to date"
    fi

    # Show dots-hyprland version when using illogical shell
    if [[ "$CURRENT_SHELL" == "illogical" ]] && command -v jq &>/dev/null; then
        DOTS_REV=$(jq -r '.nodes."dots-hyprland".locked.rev // empty' flake.lock 2>/dev/null | head -c7)
        DOTS_DATE=$(jq -r '.nodes."dots-hyprland".locked.lastModified // empty' flake.lock 2>/dev/null)
        if [[ -n "$DOTS_REV" ]]; then
            if [[ -n "$DOTS_DATE" ]]; then
                DOTS_DATE_FMT=$(date -d "@$DOTS_DATE" "+%Y-%m-%d" 2>/dev/null || echo "")
                echo "  dots-hyprland:  $DOTS_REV ($DOTS_DATE_FMT)"
            else
                echo "  dots-hyprland:  $DOTS_REV"
            fi
        fi
    fi

    # System
    if [[ "$REBUILD_FAILED" == "true" ]]; then
        echo -e "  System:         ${RED}Rebuild failed${NC}"
    elif [[ "$OLD_SYSTEM" != "$NEW_SYSTEM" ]]; then
        echo -e "  System:         ${GREEN}Rebuilt${NC}"
    else
        echo "  System:         No changes"
    fi

    # Claude
    if [[ -n "$CLAUDE_OLD" ]]; then
        if [[ "$CLAUDE_OLD" != "$CLAUDE_NEW" ]] && [[ -n "$CLAUDE_NEW" ]]; then
            echo -e "  Claude Code:    ${GREEN}$CLAUDE_OLD → $CLAUDE_NEW${NC}"
        else
            echo "  Claude Code:    Up to date ($CLAUDE_OLD)"
        fi
    else
        echo "  Claude Code:    Not installed"
    fi

    # Codex
    if [[ -n "$CODEX_OLD" ]]; then
        if [[ "$CODEX_OLD" != "$CODEX_NEW" ]] && [[ -n "$CODEX_NEW" ]]; then
            echo -e "  Codex CLI:      ${GREEN}$CODEX_OLD → $CODEX_NEW${NC}"
        else
            echo "  Codex CLI:      Up to date ($CODEX_OLD)"
        fi
    else
        echo "  Codex CLI:      Not installed"
    fi

    echo ""
    echo "  Full log: $UPDATE_LOG"

    # Package changes (keep nvd diff - it's already a good summary)
    if [[ "$OLD_SYSTEM" != "$NEW_SYSTEM" ]]; then
        echo ""
        echo "  Package changes:"
        nix run nixpkgs#nvd -- diff "$OLD_SYSTEM" "$NEW_SYSTEM" 2>/dev/null | sed 's/^/  /'
    fi

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

    # Select desktop shell
    if [[ -z "${SHELL_CHOICE:-}" ]]; then
        select_shell
    fi

    # Build full config name
    CONFIG_NAME=$(get_config_name "$BASE_HOST" "$SHELL_CHOICE")

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
    log_info "Desktop shell: $SHELL_CHOICE"
    log_info "Configuration: $CONFIG_NAME"
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
    echo "  3. You will be auto-logged in as 'john'"
    echo "  4. Set your user password: passwd"
    echo "  5. Your config is at ~/nixos-config (symlinked from /etc/nixos)"
    echo ""
    echo "Your LUKS passphrase will be required at every boot."
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
    "switch")
        SHELL_CHOICE="$ARG2"
        do_switch_shell
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
