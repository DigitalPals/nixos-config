# NixOS Configuration

A declarative NixOS configuration for single-user workstations using Flakes and Disko.

## Features

- **Declarative disk partitioning** with [Disko](https://github.com/nix-community/disko)
- **Full disk encryption** with LUKS2 (interactive passphrase at boot)
- **Btrfs filesystem** with subvolumes and zstd compression
- **Passwordless auto-login** via greetd (password set after first boot)
- **Hyprland** window manager with Noctalia desktop shell
- **Home Manager** integration for user configuration

## Hosts

| Host | Description | GPU |
|------|-------------|-----|
| `kraken` | Desktop PC | NVIDIA RTX 5090 |
| `G1a` | HP ZBook Ultra G1a | AMD Strix Halo (RDNA 3.5) |

## Partition Layout

| Partition | Size | Filesystem | Description |
|-----------|------|------------|-------------|
| ESP | 2 GB | FAT32 | EFI boot partition (`/boot`) |
| Root | Remaining | Btrfs (LUKS2) | Encrypted root with subvolumes |

### Btrfs Subvolumes

| Subvolume | Mount Point | Purpose |
|-----------|-------------|---------|
| `@` | `/` | Root filesystem |
| `@home` | `/home` | User home directories |
| `@nix` | `/nix` | Nix store |
| `@var-log` | `/var/log` | System logs |

Swap is handled by zram (25% of RAM) - no swap partition.

## Installation

### Prerequisites

- Official NixOS minimal ISO (download from [nixos.org](https://nixos.org/download/))
- UEFI-capable system
- Internet connection (Ethernet or WiFi)

### Step 1: Boot the NixOS ISO

Boot from the NixOS minimal ISO. You'll be logged in as `nixos` with root privileges.

### Step 2: Connect to the Internet

**For WiFi:**
```bash
nmtui
```
Select "Activate a connection" and connect to your network.

**For Ethernet:** Should work automatically.

Verify connectivity:
```bash
ping -c 1 github.com
```

### Step 3: Run the Installation Script

Clone the repository and run the installer:
```bash
nix-shell -p git --run "git clone https://github.com/DigitalPals/nixos-config.git"
cd nixos-config
sudo ./install.sh G1a  # or: sudo ./install.sh kraken
```

### Step 4: Select Installation Disk

If you have multiple disks, the installer will show an interactive menu:

```
[INFO] Available disks:

  1) /dev/nvme0n1       1.8T  Samsung SSD 990 PRO
  2) /dev/nvme1n1       500G  WD Black SN850X

Select disk (1-2): 1
```

Or specify the disk directly:
```bash
sudo ./install.sh G1a /dev/nvme0n1
```

### Step 5: Set LUKS Passphrase

You'll be prompted to enter a LUKS encryption passphrase. Choose a strong passphrase - you'll need it every time you boot.

### Step 6: Wait for Installation

The installer will:
1. Partition and format the disk
2. Mount the filesystems
3. Install NixOS with your configuration
4. This typically takes 10-30 minutes depending on your internet speed

### Step 7: Reboot

```bash
reboot
```

## Post-Installation

### First Boot

1. Enter your LUKS passphrase at the boot prompt
2. You'll be automatically logged in as `john` (no password required)
3. Set your user password:
   ```bash
   passwd
   ```

### Clone Your Configuration

For future modifications:
```bash
git clone https://github.com/DigitalPals/nixos-config.git ~/nixos-config
cd ~/nixos-config
```

### Rebuilding the System

After making changes to the configuration:
```bash
sudo nixos-rebuild switch --flake ~/nixos-config#G1a
```

Or use the included alias:
```bash
nrs  # nixos-rebuild switch
```

## Configuration Structure

```
nixos-config/
├── flake.nix                 # Main flake definition
├── flake.lock                # Locked dependencies
├── install.sh                # Installation script
├── hosts/
│   ├── kraken/               # Desktop configuration
│   │   ├── default.nix
│   │   └── hardware-configuration.nix
│   └── G1a/               # HP ZBook Ultra G1a
│       ├── default.nix
│       └── hardware-configuration.nix
├── modules/
│   ├── common.nix            # Shared system configuration
│   ├── desktop-environments.nix
│   ├── disko/                # Disk partitioning
│   │   ├── default.nix       # Common disko config
│   │   ├── kraken.nix        # Kraken disk device
│   │   └── G1a.nix        # G1a disk device
│   ├── boot/
│   │   └── limine-plymouth.nix
│   └── hardware/
│       └── nvidia.nix
├── home/                     # Home Manager configuration
│   ├── home.nix
│   ├── fish.nix
│   ├── ghostty.nix
│   ├── hyprland/
│   └── noctalia.nix
└── packages/                 # Custom packages
```

## Troubleshooting

### No network on first boot
NetworkManager should work automatically. If not:
```bash
nmtui
```

### Forgot LUKS passphrase
There is no recovery option. You'll need to reinstall.

### Change disk device after installation
Edit `modules/disko/<hostname>.nix` and update the device path, then reinstall.

## License

MIT
