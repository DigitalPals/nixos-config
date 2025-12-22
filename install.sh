#!/usr/bin/env bash
#
# NixOS Installation Script with Disko
#
# Usage: ./install.sh <hostname> [disk-device]
#   hostname: kraken or G1a
#   disk-device: optional, shows interactive selector if not specified
#
# Run from the official NixOS minimal ISO
#
# Steps:
#   1. Boot NixOS ISO
#   2. Connect to WiFi: nmtui
#   3. Clone and run:
#      nix-shell -p git --run "git clone https://github.com/DigitalPals/nixos-config.git"
#      cd nixos-config && sudo ./install.sh <hostname>

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

# Configuration
REPO_URL="https://github.com/DigitalPals/nixos-config.git"
TEMP_CONFIG="/tmp/nixos-config"
CONFIG_DIR="/mnt/home/john/nixos-config"
SYMLINK_PATH="/mnt/etc/nixos"

# Parse arguments
HOSTNAME="${1:-}"
DISK_DEVICE="${2:-}"

# Show usage
usage() {
    echo "NixOS Installation Script with Disko"
    echo ""
    echo "Usage: $0 <hostname> [disk-device]"
    echo ""
    echo "Arguments:"
    echo "  hostname     Required. Must be 'kraken' or 'G1a'"
    echo "  disk-device  Optional. If not specified, shows interactive selector"
    echo ""
    echo "Examples:"
    echo "  $0 G1a              # Interactive disk selection"
    echo "  $0 G1a /dev/nvme0n1 # Use specific disk"
    echo "  $0 kraken /dev/sda     # Use SATA drive"
    echo ""
    exit 1
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

# Validate arguments
if [[ -z "$HOSTNAME" ]]; then
    usage
fi

if [[ "$HOSTNAME" != "kraken" && "$HOSTNAME" != "G1a" ]]; then
    log_error "Invalid hostname: $HOSTNAME. Must be 'kraken' or 'G1a'"
fi

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root. Try: sudo $0 $*"
fi

# If no disk specified, show interactive selector
if [[ -z "$DISK_DEVICE" ]]; then
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
log_info "Target hostname: $HOSTNAME"
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
DISKO_HOST_FILE="$TEMP_CONFIG/modules/disko/${HOSTNAME}.nix"
if [[ -f "$DISKO_HOST_FILE" ]]; then
    sed -i "s|device = \"/dev/[^\"]*\"|device = \"$DISK_DEVICE\"|" "$DISKO_HOST_FILE"
    log_success "Updated disk device to $DISK_DEVICE in $DISKO_HOST_FILE"
else
    log_error "Disko configuration not found: $DISKO_HOST_FILE"
fi

# Step 5: Run disko to partition and format
log_info "Step 5/6: Running disko to partition and format..."
echo ""
log_warn "You will be prompted to enter the LUKS encryption passphrase."
log_warn "Choose a strong passphrase - you will need it every time you boot."
echo ""
read -p "Press Enter to continue with disk partitioning..."

nix run github:nix-community/disko -- \
    --mode destroy,format,mount \
    --flake "$TEMP_CONFIG#$HOSTNAME"

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

# Initialize git repo (as root, then fix ownership)
cd "$CONFIG_DIR"
nix-shell -p git --run "git init && git add -A && git -c user.name='NixOS Install' -c user.email='install@localhost' commit -m 'Initial configuration'"
cd - >/dev/null

# Set ownership to john (uid 1000, gid 100)
chown 1000:100 "$(dirname "$CONFIG_DIR")"
chown -R 1000:100 "$CONFIG_DIR"

log_info "Configuration installed to $CONFIG_DIR"
log_info "Running nixos-install (this may take a while)..."
echo ""

nixos-install --flake "${CONFIG_DIR}#${HOSTNAME}" --no-root-passwd

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
