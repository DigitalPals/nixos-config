# NixOS Configuration

A declarative NixOS configuration for single-user workstations using Flakes and Disko, featuring multi-shell support with three different Hyprland desktop environments.

## Features

- **Declarative disk partitioning** with [Disko](https://github.com/nix-community/disko)
- **Full disk encryption** with LUKS2 (interactive passphrase at boot)
- **Btrfs filesystem** with subvolumes and zstd compression
- **Passwordless auto-login** via greetd (password set after first boot)
- **Hyprland** window manager with choice of desktop shells
- **Home Manager** integration for user configuration
- **Multi-shell support** - switch between 3 desktop environments

## Desktop Shells

This configuration supports three different Hyprland desktop shells. Each provides a complete desktop experience with its own theming, panels, and widgets.

| Shell | Description | Source |
|-------|-------------|--------|
| **Noctalia** (default) | Modern Qt6/QML desktop shell | [noctalia-dev/noctalia-shell](https://github.com/noctalia-dev/noctalia-shell) |
| **Illogical Impulse** | Material Design 3 Quickshell-based shell | [end-4/dots-hyprland](https://github.com/end-4/dots-hyprland) |
| **Caelestia** | Feature-rich Quickshell desktop environment | [caelestia-dots/shell](https://github.com/caelestia-dots/shell) |

### Switching Shells

Desktop shells are switched via the **boot menu** (NixOS specialisations):

1. Reboot your system
2. In the Limine boot menu, select your generation
3. Choose from the sub-menu:
   - **Default** - Noctalia
   - **illogical** - Illogical Impulse
   - **caelestia** - Caelestia

The selected shell persists for that boot session. To switch shells, reboot and select a different option.

## Hosts

| Host | Description | GPU |
|------|-------------|-----|
| `kraken` | Desktop PC | NVIDIA RTX 5090 |
| `G1a` | HP ZBook Ultra G1a | AMD Strix Halo (RDNA 3.5) |

## Flake Configurations

Each host has one configuration with shell variants as specialisations:

| Configuration | Host | Specialisations |
|---------------|------|-----------------|
| `kraken` | kraken (NVIDIA) | Default (Noctalia), illogical, caelestia |
| `G1a` | G1a (AMD) | Default (Noctalia), illogical, caelestia |

Rebuilding includes all shell specialisations:
```bash
sudo nixos-rebuild switch --flake .#kraken
```

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

### Step 3: Run Forge

Run the Forge installer directly from the flake:
```bash
nix run github:DigitalPals/nixos-config#forge
```

The interactive TUI will guide you through:
1. Select your host (kraken or G1a)
2. Select the target disk
3. Confirm the installation (type 'yes')
4. Set your LUKS encryption passphrase when prompted

Alternatively, run with arguments for non-interactive install:
```bash
nix run github:DigitalPals/nixos-config#forge -- install kraken /dev/nvme0n1
```

### Step 4: Wait for Installation

The installer will:
1. Partition and format the disk
2. Mount the filesystems
3. Install NixOS with your configuration
4. This typically takes 10-30 minutes depending on your internet speed

### Step 5: Reboot

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

### CLI Tool Installs (Codex + Claude)

Codex CLI (npm) and Claude Code are installed via Home Manager activation. This is best-effort:
- If online, they are installed on first activation.
- If offline, installation is skipped and retried on the next activation.

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

### Updating the System

Run Forge to update flake inputs, rebuild, and update CLI tools:
```bash
nix run github:DigitalPals/nixos-config#forge -- update
```

Or if you have the config cloned locally:
```bash
nix run .#forge -- update
```

This will:
1. Update all flake inputs (`nix flake update`)
2. Rebuild the system if there are changes
3. Update Claude Code and Codex CLI
4. Check browser profile sync status

## Configuration Structure

```
nixos-config/
├── flake.nix                 # Main flake with host+shell configurations
├── flake.lock                # Locked dependencies
├── hosts/
│   ├── kraken/               # Desktop configuration (NVIDIA)
│   │   ├── default.nix
│   │   └── hardware-configuration.nix
│   └── G1a/                  # HP ZBook Ultra G1a (AMD)
│       ├── default.nix
│       └── hardware-configuration.nix
├── modules/
│   ├── common.nix            # Shared system configuration
│   ├── desktop-environments.nix
│   ├── disko/                # Disk partitioning
│   │   ├── default.nix       # Common disko config
│   │   ├── kraken.nix        # Kraken disk device
│   │   └── G1a.nix           # G1a disk device
│   ├── boot/
│   │   └── limine-plymouth.nix
│   └── hardware/
│       └── nvidia.nix
├── home/                     # Home Manager configuration
│   ├── home.nix              # Main config (imports shell based on flake)
│   ├── ghostty.nix           # Terminal configuration
│   ├── hyprland/             # Hyprland window manager config
│   │   ├── autostart.nix     # Shell-aware autostart
│   │   └── bindings.nix      # Shell-aware keybindings
│   └── shells/               # Desktop shell configurations
│       ├── noctalia/         # Noctalia Desktop Shell
│       │   ├── default.nix
│       │   ├── shell.nix     # Shell + JSON configs
│       │   ├── fish.nix      # Fish + Starship + Zoxide
│       │   └── theming.nix   # GTK, cursor, icons
│       ├── illogical/        # Illogical Impulse
│       │   ├── default.nix
│       │   ├── dotfiles.nix  # Fetch upstream configs
│       │   ├── packages.nix  # Qt, Quickshell, tools
│       │   ├── fish.nix      # Fish shell config
│       │   └── theming.nix   # Cursor, GTK, icons
│       └── caelestia/        # Caelestia Desktop Shell
│           ├── default.nix
│           ├── shell.nix     # Caelestia config
│           ├── fish.nix      # Fish shell config
│           └── theming.nix   # Theme configuration
└── packages/
    ├── forge/                # TUI installer and system management tool
    ├── plymouth-cybex/       # Custom Plymouth theme
    └── hyprland-sessions/    # Session .desktop files for each shell
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

### Shell switch not taking effect
Shell switching is done via the boot menu (specialisations). You must reboot and select the desired shell from the Limine boot menu. Do not use `hyprctl reload` as it will break keybindings.

## License

MIT
