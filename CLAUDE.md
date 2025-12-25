# NixOS Configuration Notes

Configuration details and solutions to issues in this NixOS setup.

## Configuration Structure

```
~/nixos-config/                     # Symlinked from /etc/nixos
├── flake.nix                       # Main flake with host definitions
├── hosts/
│   ├── kraken/                     # Desktop with NVIDIA RTX 5090
│   └── G1a/                        # HP ZBook Ultra G1a (AMD Strix Halo)
├── modules/
│   ├── boot/limine-plymouth.nix    # Bootloader + Plymouth config
│   ├── common.nix                  # Shared system config
│   ├── shell-config.nix            # Desktop shell option (specialisations)
│   ├── desktop-environments.nix
│   ├── gaming.nix
│   ├── disko/                      # Disk partitioning configs
│   └── hardware/
│       ├── nvidia.nix              # NVIDIA driver config
│       └── intel.nix               # Intel GPU config (unused)
├── home/
│   ├── home.nix                    # Main Home Manager config
│   ├── ghostty.nix                 # Terminal config
│   ├── neovim.nix
│   ├── 1password-secrets.nix       # 1Password SSH agent integration
│   ├── app-backup/                 # App profile backup/restore (browsers, Termius)
│   │   └── default.nix
│   ├── hyprland/                   # Hyprland WM config (modular)
│   │   ├── default.nix
│   │   ├── bindings.nix
│   │   ├── monitors.nix
│   │   └── ...
│   └── shells/                     # Desktop shell options
│       ├── noctalia/               # AGS-based shell
│       ├── illogical/              # Illogical Impulse shell
│       └── caelestia/              # Caelestia shell
└── packages/
    ├── forge/                      # Rust TUI configuration tool
    ├── plymouth-cybex/             # Custom Plymouth theme
    └── hyprland-sessions/          # Session desktop entries
```

## Forge - NixOS Configuration Tool

Forge is a Rust TUI application for managing NixOS installations and updates. Copyright Cybex B.V.

### Running Forge

```bash
# From installed system
forge                    # Interactive TUI menu
forge update            # Update flake + rebuild + CLI tools
forge apps backup       # Backup app profiles (browsers, Termius)
forge apps restore      # Restore app profiles

# From NixOS ISO (fresh install)
nix run github:DigitalPals/nixos-config
```

### Commands

| Command | Description |
|---------|-------------|
| `forge` | Interactive TUI with main menu |
| `forge install [hostname] [disk]` | Fresh NixOS installation |
| `forge create-host [hostname]` | Create a new host configuration |
| `forge update` | Update flake, rebuild, update CLI tools |
| `forge apps backup` | Backup + push app profiles |
| `forge apps restore` | Pull + restore app profiles |
| `forge apps status` | Check for profile updates |

Note: `forge browser` is still supported as an alias for `forge apps`.

### Fresh Installation from ISO

1. Boot the NixOS minimal ISO
2. Connect to WiFi: `nmtui`
3. Run Forge: `nix run github:DigitalPals/nixos-config`
4. Select "Install NixOS", choose host and disk
5. Enter LUKS passphrase when prompted
6. Reboot and select a shell from the boot menu

## Rebuilding the System

Each host has one configuration with shell variants as specialisations:

| Config | Host | Specialisations |
|--------|------|-----------------|
| `kraken` | kraken (NVIDIA) | Default (Noctalia), illogical, caelestia |
| `G1a` | G1a (AMD) | Default (Noctalia), illogical, caelestia |

```bash
# Rebuild (includes all shell specialisations)
sudo nixos-rebuild switch --flake .#kraken

# Or use hostname (auto-detected)
sudo nixos-rebuild switch --flake .
```

## Switching Desktop Shells

Desktop shells are switched via the **boot menu** (Limine):

1. Reboot your system
2. In Limine, select your generation
3. Choose from the sub-menu:
   - **Default** - Noctalia (AGS-based shell)
   - **illogical** - Illogical Impulse (Material Design 3)
   - **caelestia** - Caelestia desktop shell

The selected shell persists for that boot session. To change shells, reboot and select a different specialisation.

**Note:** Each rebuild builds all three shell variants. The boot menu shows all options for each generation.

## Plymouth + NVIDIA Issue

**Problem:** Plymouth doesn't display on NVIDIA systems due to framebuffer timing race - `simpledrm` initializes first, Plymouth attaches, then NVIDIA takes over fb0.

**Solution:** Make Plymouth wait for udev to settle (`modules/boot/limine-plymouth.nix:51-54`).

**Trade-off:** Adds a few seconds to boot time.

**What doesn't work:** Adding `simpledrm` to initrd (builtin), `video=` kernel params (simpledrm ignores them), skipping udev-settle.

**Host notes:**
- **kraken (NVIDIA):** Requires udev-settle workaround
- **G1a (AMD):** Uses `hardware.amdgpu.initrd.enable`, no timing issues

## Plymouth Resolution on Limine

**Problem:** Plymouth displays at low resolution (~1080p) regardless of native display.

**Root cause:** NixOS Limine module doesn't expose per-entry `resolution:` option. The `interface.resolution` only affects the menu, not the Linux framebuffer.

**Status:** Accepted limitation. Consider filing nixpkgs feature request for `boot.loader.limine.resolution`.

## Home Manager Backup File Conflicts

**Problem:** `programs.ghostty.themes.*` creates regular files that cause backup conflicts on each activation.

**Solution:** Use `xdg.configFile` with `force = true` instead (`home/ghostty.nix:77-80`).

## NVIDIA Suspend/Resume Fix

**Problem:** Display issues after waking from suspend on NVIDIA.

**Solution:** Added resume hook to call `nvidia-sleep.sh resume` and extended `nvidia-resume` service for suspend-then-hibernate (`modules/hardware/nvidia.nix:44-53`).

## Key NVIDIA Settings

All NVIDIA config is in `modules/hardware/nvidia.nix`:
- Open kernel modules enabled
- Modesetting + power management
- Initrd modules: `nvidia`, `nvidia_modeset`, `nvidia_uvm`, `nvidia_drm`
- Kernel params: `nvidia-drm.modeset=1`, `nvidia-drm.fbdev=1`
- Wayland env vars: `GBM_BACKEND`, `__GLX_VENDOR_LIBRARY_NAME`, `NIXOS_OZONE_WL`

Host `kraken` uses `lib.mkForce` to ensure all modules load together (`hosts/kraken/default.nix:15-22`).

## Noctalia Settings (Hybrid Management)

Noctalia settings use a hybrid approach that allows GUI changes while preserving reproducibility across machines.

### How It Works

Settings are stored in `~/.config/noctalia/` as regular files (not symlinks). A hash file tracks when the repo version was last deployed:

- **First run**: Configs are copied from repo to `~/.config/noctalia/`
- **GUI changes**: Saved locally, persist across reboots and rebuilds
- **Repo updated**: When you pull updated configs from another machine and rebuild, the hash changes and local files are overwritten

Implementation: `home/shells/noctalia/shell.nix:44-70`

### Syncing Settings to Another Machine

When you've made GUI changes you want to sync to the repo:

1. **Ask Claude**: "Sync my Noctalia settings to the repo"
   - Claude copies `~/.config/noctalia/*.json` → `home/shells/noctalia/`
2. **Commit and push** the changes
3. **On other machine**: Pull and rebuild → hash changes → local files updated

### Config Files

| File | Purpose |
|------|---------|
| `settings.json` | Main shell settings (bar widgets, layouts) |
| `gui-settings.json` | GUI-specific preferences |
| `colors.json` | Color scheme |
| `plugins.json` | Plugin configuration |
| `.deployed-hash` | Tracks repo version (auto-managed) |

### Forcing a Re-sync

To force re-deployment from repo (discarding local changes):
```bash
rm ~/.config/noctalia/.deployed-hash
sudo nixos-rebuild switch --flake .
```

## 1Password SSH Agent

SSH keys are managed through 1Password's SSH agent (`home/1password-secrets.nix`). After rebuild:

1. Open 1Password GUI
2. Settings → Developer → Enable "Integrate with 1Password CLI"
3. Settings → Developer → Enable "Use the SSH agent"
4. Add/import SSH keys to 1Password

SSH commands will automatically use keys from 1Password after a single unlock.

## App Profile Backup/Restore

Encrypted app profile backup system using Age encryption and a private GitHub repository. Supports 1Password integration for automatic key retrieval across machines.

### Supported Applications

- **Chrome**: Cookies, login data, sessions, preferences
- **Firefox**: Cookies, logins, sessions, sync data
- **Termius**: Session tokens, saved hosts, SSH keys, settings

### Setup with 1Password (Recommended)

1. Generate an Age keypair locally:
   ```bash
   age-keygen
   # Output:
   # Public key: age1xxxxxxxxxx...
   # AGE-SECRET-KEY-1XXXXXXXXXX...
   ```

2. Store the private key in 1Password:
   - Create a new item in 1Password (e.g., "age-key" in Private vault)
   - Add a field called "private-key" with the `AGE-SECRET-KEY-1...` value
   - The 1Password reference will be: `op://Private/age-key/private-key`

3. Configure in `home/home.nix`:
   ```nix
   programs.app-backup = {
     enable = true;
     # Repo is pre-configured to: git@github.com:DigitalPals/private-settings.git
     ageRecipient = "age1...your-public-key...";
     ageKey1Password = "op://Private/age-key/private-key";
   };
   ```

4. Rebuild: `sudo nixos-rebuild switch --flake .`

### Alternative: File-based Key

If not using 1Password, you can use a file-based key:
```nix
programs.app-backup = {
  enable = true;
  ageRecipient = "age1...";
  ageKeyPath = "~/.config/age/key.txt";  # Fallback if ageKey1Password not set
};
```

### Commands

Via Forge TUI (recommended):
```bash
forge apps backup        # Backup + push profiles to GitHub
forge apps restore       # Restore profiles from GitHub
forge apps status        # Check for remote updates
forge apps               # Interactive menu

# Backward compatibility alias
forge browser backup     # Same as forge apps backup
```

Via standalone scripts (after Home Manager activation):
```bash
app-backup --push          # Backup + push
app-restore --pull         # Pull + restore

# Deprecated aliases (still work)
browser-backup --push      # Same as app-backup
browser-restore --pull     # Same as app-restore
```

### New Machine Bootstrap

1. Install NixOS with Forge: `nix run github:DigitalPals/nixos-config`
2. Sign in to 1Password desktop app (unlocks the CLI)
3. Run `forge apps restore`
4. Open apps - sessions restored (Chrome, Firefox, Termius)

The age key is retrieved from 1Password on-the-fly - no manual key management needed!

### Troubleshooting

- **"Apps are running"**: Close Chrome/Firefox/Termius or use `--force`
- **"1Password not unlocked"**: Open 1Password app and sign in
- **"op: command not found"**: Rebuild to install 1Password CLI
- **"Git push failed"**: Check SSH key is in 1Password agent
- **"Config not found"**: Enable `programs.app-backup` and rebuild

### Security Notes

Profile archives contain session cookies, auth tokens, and potentially saved passwords. The archives are encrypted with Age before being pushed to GitHub.

- Age private key is stored in 1Password, never on disk
- Key is retrieved on-the-fly and never written to filesystem
- LUKS disk encryption (enabled by default) provides additional protection
- Decrypted archives are only created in temp directories and shredded after use
