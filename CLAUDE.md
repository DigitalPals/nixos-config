# NixOS Configuration Notes

Configuration details and solutions to issues in this NixOS setup.

## Configuration Structure

```
~/nixos-config/                     # Symlinked from /etc/nixos
├── flake.nix                       # Main flake with host definitions
├── hosts/
│   ├── kraken/                     # Desktop: AMD Ryzen 9 9950X3D, NVIDIA RTX 5090, 96GB RAM
│   ├── G1a/                        # HP ZBook Ultra G1a: AMD Ryzen AI MAX+ PRO 395, Radeon 8060S, 64GB RAM
│   └── proart/                     # ASUS ProArt P16 OLED: AMD Ryzen AI 9 HX 370, Radeon 890M + RTX 5090, 64GB RAM
├── modules/
│   ├── boot/limine-plymouth.nix    # Bootloader + Plymouth config
│   ├── common.nix                  # Shared system config
│   ├── shell-config.nix            # Desktop shell option
│   ├── desktop-environments.nix
│   ├── gaming.nix
│   ├── disko/                      # Disk partitioning configs
│   ├── iso/                        # Forge installer ISO config
│   └── hardware/
│       ├── nvidia.nix              # NVIDIA driver config
│       └── intel.nix               # Intel GPU config (unused)
├── home/
│   ├── home.nix                    # Main Home Manager config
│   ├── ghostty.nix                 # Terminal config
│   ├── neovim.nix
│   ├── 1password-secrets.nix       # 1Password SSH agent integration
│   ├── app-backup/                 # App profile backup/restore (browsers, Portal)
│   │   └── default.nix
│   ├── hyprland/                   # Hyprland WM config (modular)
│   │   ├── default.nix
│   │   ├── bindings.nix
│   │   ├── monitors.nix
│   │   └── ...
│   └── shells/
│       └── noctalia/               # Noctalia Desktop Shell config
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
forge apps backup       # Backup app profiles (browsers, Portal)
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
6. Reboot

### Building the Forge ISO

Build a custom ISO that boots directly into Forge:

```bash
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

The ISO will be at `result/iso/NixOS-Cybex-<version>.iso`. Flash to USB:

```bash
sudo dd if=result/iso/NixOS-Cybex-*.iso of=/dev/sdX bs=4M status=progress
```

The ISO automatically:
1. Boots with Plymouth cybex theme
2. Auto-logins and checks internet connectivity
3. Opens `nmtui` if WiFi needed
4. Launches Forge installer from GitHub

## Rebuilding the System

```bash
# Rebuild with explicit host
sudo nixos-rebuild switch --flake .#kraken

# Or use hostname (auto-detected)
sudo nixos-rebuild switch --flake .
```

## Hyprland 0.53+ Changes

### Startup

**Change:** Hyprland 0.53 introduced `start-hyprland` as the required launcher, replacing direct `Hyprland` invocation.

**Implementation:** The session wrapper scripts in `packages/hyprland-sessions/default.nix` use `exec start-hyprland -- "$@"` to launch Hyprland properly.

**Benefits:**
- Crash recovery - Hyprland can recover from crashes without losing your session
- Safe mode - Allows booting into a minimal config if the main config is broken

**Optional dependency:** `hyprland-guiutils` enhances safe mode and provides a welcome app for new users. Not yet available in nixpkgs as of December 2025.

### Window Rules Syntax

**Change:** Hyprland 0.53 completely overhauled window rules syntax. The old `windowrulev2` format is deprecated.

**Old syntax (deprecated):**
```
windowrulev2 = float, class:^(firefox)$
windowrulev2 = center, class:^(firefox)$
windowrulev2 = size 800 600, class:^(firefox)$
windowrulev2 = suppressevent maximize, class:.*
windowrulev2 = noscreenshare, class:^(1password)$
```

**New syntax (0.53+):**
```
# IMPORTANT: match clauses MUST come first, then effects
windowrule = match:class firefox, float on, center on, size 800 600
windowrule = match:class .*, suppress_event maximize
windowrule = match:class 1[pP]assword, no_screen_share on
```

**Key differences:**
- `windowrulev2` → `windowrule`
- `class:^(pattern)$` → `match:class pattern` (regex simplified, no anchors needed)
- `title:^(pattern)$` → `match:title pattern`
- **Match clauses must come FIRST**, before any effects
- Actions use `on/off` suffix: `float` → `float on`, `center` → `center on`
- Property names use underscores: `suppressevent` → `suppress_event`, `noscreenshare` → `no_screen_share`
- Multiple actions can be combined in one rule

**Common properties:**
- `float on/off` - Float the window
- `center on` - Center the window
- `size W H` or `size W% H%` - Set window size
- `opacity X Y` - Set active/inactive opacity (0.0-1.0)
- `suppress_event maximize/fullscreen/activate` - Ignore window events
- `no_screen_share on` - Hide window from screen sharing

**Properties without new syntax equivalent:**
- `scrollInput` - No equivalent in `windowrule`; must use legacy `windowrulev2` format

**Implementation:** Window rules are split across files:
- `home/hyprland/looknfeel.nix` - Most window rules (using new syntax)
- `home/hyprland/input.nix` - `scrollInput` rule (must use legacy `windowrulev2`)

**Documentation:** https://wiki.hypr.land/Configuring/Window-Rules/

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

## Strix Halo (G1a) Suspend/Resume Fix

**Problem:** Intermittent suspend/resume failure on HP ZBook Ultra G1a (AMD Strix Halo). The display doesn't wake up, showing errors:

```
amd_pmc: Last suspend didn't reach deepest state
amdgpu: resume of IP block <vpe_v6_1> failed -110
amdgpu: amdgpu_device_ip_resume failed (-110)
```

**Root cause:** The VPE (Video Processing Engine) block times out during resume (~8% of cycles). The VPE_IDLE_TIMEOUT is 1 second but Strix Halo needs ~2 seconds.

**Current status:** Using latest kernel (6.18) and accepting ~8% suspend failure rate. Kernels 6.14-6.17 have reached EOL in nixpkgs. The VPE fix is expected in kernel 6.19+.

**Kernel config:** Kernel is set centrally in `modules/boot/limine-plymouth.nix` to `linuxPackages_latest`. No per-host override needed.

**Kernel 6.18 regression:** Kernel 6.18.x has a VPE regression that breaks suspend even with `amd_iommu=off`. A problematic VPE patch was merged; the revert targets kernel 6.19, not 6.18. Framework 13/AMD and other Strix Halo users confirm this regression.

**Fallback option:** If suspend failures are unacceptable, override with 6.12 LTS in `hosts/G1a/default.nix`:
```nix
boot.kernelPackages = lib.mkForce pkgs.linuxPackages_6_12;
```

**When fixed:** Kernel 6.19+ should include the VPE revert. Once `linuxPackages_latest` points to 6.19+, suspend should work reliably.

**Additional fix:** MediaTek WiFi module needs ASPM disabled for reliable resume. See "MT7925 WiFi Stability (G1a)" section below for full configuration.

**BIOS settings (important):**
- **Disable**: "Motion sensing cooling mode" (causes suspend issues)
- **Keep enabled**: Secure Boot, RAM encryption, Pluton (disabling these can break suspend)

**Security note:** `amd_iommu=off` disables DMA attack protection. LUKS encryption still protects data. This is the same configuration used by Arch/Fedora users.

**What doesn't work:**
- `mem_sleep_default=deep` - S3 not supported; only s2idle available (ACPI: S0 S4 S5)
- Hibernate - Requires LVM swap partition (see "ProArt P16 Hibernate" section for setup)
- `amdgpu.ip_block_mask=0xfffff7ff` - Disables VPE; breaks hardware video processing
- `amdgpu.pg_mask=0` - Disables all power gating; high power consumption

**Upstream status:** A kernel patch for VPE_IDLE_TIMEOUT (1s→2s) was [submitted](https://www.mail-archive.com/amd-gfx@lists.freedesktop.org/msg127724.html) but not merged. AMD indicated the fix should come via BIOS updates.

**Debugging commands:**
```bash
# Check last suspend errors
journalctl -b -1 | grep -iE "(suspend|resume|vpe|amdgpu.*failed)"

# Check available sleep states (only s2idle on this hardware)
cat /sys/power/mem_sleep

# List IP blocks and their positions
sudo dmesg | grep "detected ip block"
```

## MT7925 WiFi Stability (G1a)

**Problem:** Random WiFi disconnects on HP ZBook Ultra G1a with MediaTek MT7925 (WiFi 7) adapter. Connection drops intermittently despite strong signal.

**Root cause:** Multiple bugs in the mt7925e driver affecting kernels 6.14.3 through 6.18:
- Commit `cb1353ef34735` ("wifi: mt76: mt7925: integrate *mlo_sta_cmd and *sta_cmd") causes speed drops on some routers
- CLC (Country Location Code) feature causes instability
- ASPM power management interferes with driver operation
- WiFi power save causes disconnects

**Current workarounds** (in `hosts/G1a/default.nix`):
```nix
# Give Linux ASPM control
boot.kernelParams = [ "pcie_aspm=force" ];

# Load driver explicitly + disable problematic features
boot.kernelModules = [ "mt7925e" ];
boot.extraModprobeConfig = ''
  options mt7925e disable_aspm=1
  options mt7925-common disable_clc=1
'';
```

Also in `modules/common.nix`:
```nix
networking.networkmanager.wifi.powersave = false;
```

**When fixed:** Kernel 6.19+ may include fixes for MT792x WiFi issues. Check current kernel version with `nix eval nixpkgs#linuxPackages_latest.kernel.version`. Once on kernel 6.19+, test without `disable_clc=1` to see if the workaround is still needed.

**References:**
- [Ubuntu Bug #2118937](https://bugs.launchpad.net/ubuntu/+source/linux/+bug/2118937) - Intermittent connection loss
- [Ubuntu Bug #2118755](https://bugs.launchpad.net/ubuntu/+source/linux/+bug/2118755) - 6GHz instability
- [linux-mediatek regression report](https://lists.infradead.org/pipermail/linux-mediatek/2025-August/096746.html) - Bisected to specific commit
- [Framework Community thread](https://community.frame.work/t/issues-with-mediatek-mt7925-rz717-wi-fi-card/75815)

## ProArt P16 Hibernate (LVM)

**Host:** ASUS ProArt P16 OLED (AMD Ryzen AI 9 HX 370 + NVIDIA RTX 5090, 64GB RAM)

**Problem:** Btrfs swapfile hibernate has ~70% reliability with systemd initrd due to known NixOS issues ([#213122](https://github.com/NixOS/nixpkgs/issues/213122)).

**Solution:** Use LVM inside LUKS for a dedicated swap partition instead of btrfs swapfile.

**Disk layout** (`modules/disko/proart.nix`):
```
ESP (2GB, FAT32, /boot)
└── LUKS2 (remaining)
    └── LVM volume group "vg"
        ├── swap (66GB) → hibernate
        └── root (remaining) → btrfs with subvolumes
```

**Configuration:**
- Disko: `modules/disko/proart.nix` (standalone, doesn't import default.nix)
- Host: `hosts/proart/default.nix` with `boot.resumeDevice = "/dev/vg/swap"`
- zramSwap disabled for hibernate

**Why LVM?**
- Dedicated swap partition doesn't need `resume_offset` calculation
- No btrfs swapfile edge cases with systemd initrd
- Single LUKS unlock, then LVM provides both swap and root
- More reliable hibernate (~95%+ vs ~70% with btrfs swapfile)

**Installation:**
1. Boot Forge ISO or NixOS minimal ISO
2. Select `proart` host in Forge
3. Select "Hibernate Support" - Forge detects LVM and skips swapfile injection
4. Hibernate is pre-configured in the host config

**Hibernate commands:**
```bash
# Hibernate
systemctl hibernate

# Suspend-then-hibernate (suspend first, hibernate after timeout)
systemctl suspend-then-hibernate
```

**Troubleshooting:**
```bash
# Verify swap is active
swapon --show

# Check resume device
cat /sys/power/resume

# Check last hibernate attempt
journalctl -b -1 | grep -iE "(hibernate|resume|PM:)"
```

## ProArt P16 Suspend Fix (GPIO Wake)

**Host:** ASUS ProArt P16 OLED (H7606WX)

**Problem:** Closing the laptop lid triggers suspend, but the laptop wakes up immediately (within 5 seconds) due to a spurious GPIO interrupt. This creates a rapid suspend/wake cycle.

**Root cause:** GPIO pin 16 on the AMD GPIO controller (AMDI0030:00) triggers a wake interrupt during s2idle suspend. This is a known issue on AMD Ryzen 7000+ laptops with s2idle.

**Solution:** Add kernel parameter to ignore the problematic GPIO pin (`hosts/proart/default.nix`):
```nix
boot.kernelParams = lib.mkAfter [
  "gpiolib_acpi.ignore_interrupt=AMDI0030:00@16"
];
```

**How to identify GPIO pins on other systems:**
```bash
# Enable PM debug messages
echo 1 | sudo tee /sys/power/pm_debug_messages

# Trigger suspend (will wake immediately if affected)
sudo systemctl suspend

# Check dmesg for the active GPIO
sudo dmesg | grep "GPIO.*is active"
# Example output: GPIO 16 is active: 0x30047c00
```

**Additional configuration:** The host also disables various ACPI wakeup sources at boot via a systemd service to prevent other potential wake triggers.

**References:**
- [Arch Wiki - Power management/Wakeup triggers](https://wiki.archlinux.org/title/Power_management/Wakeup_triggers)
- [EndeavourOS Forum - ProArt P16 wakeup issue](https://forum.endeavouros.com/t/asus-proart-p16-h7606wv-wakeup-few-secons-after-suspend/63863)

**Note:** The ProArt P16 also has working hibernate support via LVM swap (see "ProArt P16 Hibernate" section). Hibernate requires proprietary NVIDIA modules.

## ProArt P16 Fan Control (asusctl)

**Host:** ASUS ProArt P16 OLED (H7606WX)

**Problem:** Fans spin at ~2200 RPM even at idle (33°C) when using the default "Balanced" platform profile.

**Solution:** Use `asusctl` with the "Quiet" profile configured as default for both AC and battery power.

**Configuration** (`hosts/proart/default.nix`):
```nix
services.asusd = {
  enable = true;
  enableUserService = true;
  asusdConfig.text = ''
    (
      platform_profile_on_battery: Quiet,
      change_platform_profile_on_battery: true,
      platform_profile_on_ac: Quiet,
      change_platform_profile_on_ac: true,
      # ... other settings
    )
  '';
};
```

**Profile behavior:**

| Profile | Fans at Idle | Under Load | Use Case |
|---------|--------------|------------|----------|
| Quiet | 0 RPM | Spin up gradually | Silent operation, light work |
| Balanced | ~2000 RPM | Spin up faster | Default, mixed workloads |
| Performance | Always on | Maximum cooling | Heavy workloads, gaming |

**Commands:**
```bash
# Check current profile
asusctl profile -p

# Switch profiles
asusctl profile -P Quiet        # Silent operation
asusctl profile -P Balanced     # Default
asusctl profile -P Performance  # Maximum cooling

# Cycle through profiles
asusctl profile -n
```

**Monitoring:**
```bash
# Check fan speeds and temperatures
sensors | grep -E "(fan|Tctl|edge)"

# Check kernel platform profile
cat /sys/firmware/acpi/platform_profile
```

**Note:** The Quiet profile allows fans to spin up automatically when temperatures rise under load. At idle (~45°C), fans remain off for completely silent operation.

## Key NVIDIA Settings

All NVIDIA config is in `modules/hardware/nvidia.nix`:
- **Proprietary kernel modules** (`open = false`) - Required for hibernate support
- Modesetting + power management
- Initrd modules: `nvidia`, `nvidia_modeset`, `nvidia_uvm`, `nvidia_drm`
- Kernel params: `nvidia-drm.modeset=1`, `nvidia-drm.fbdev=1`
- Wayland env vars: `GBM_BACKEND`, `__GLX_VENDOR_LIBRARY_NAME`, `NIXOS_OZONE_WL`

**Hibernate requirement:** The open-source NVIDIA kernel modules fail to restore GPU state during hibernate resume (error -5 in `nv_pmops_freeze`). Proprietary modules with `NVreg_PreserveVideoMemoryAllocations=1` are required.

Host `kraken` uses `lib.mkForce` to ensure all modules load together (`hosts/kraken/default.nix:15-22`).

## Shell Restart on Store Path Change

**Problem:** After `nixos-rebuild switch`, the running quickshell process has old `/nix/store/...` paths baked in, while IPC commands (like `noctalia-shell ipc call launcher toggle`) reference the new path. This causes IPC failures with "No running instances" errors.

**Root cause:** Quickshell processes embed their store path at startup. When the package updates, the symlink at `~/.config/quickshell/noctalia-shell` points to the new path, but the running process still has the old path.

**Solution:** Home Manager activation hook that automatically restarts the shell when store paths change.

**Implementation:** `home/shells/restart-on-change.nix`

The hook:
1. Hashes the noctalia-shell package path
2. Compares to previously stored hash in `~/.local/state/shell-store-hash`
3. If changed and quickshell is running, kills old process and restarts via `hyprctl dispatch exec`
4. Records new hash for next comparison

**Behavior:**
- **First run**: No restart (no previous hash to compare), just records hash
- **Package unchanged**: No restart, hash matches
- **Package updated**: Automatic restart via hyprctl

**Edge cases handled:**
- First run after adding the hook (no restart)
- No Hyprland running (gracefully skipped)
- No quickshell running (gracefully skipped)
- Dry-run mode (respects `$DRY_RUN_CMD`)

**Files:**
- `home/shells/restart-on-change.nix` - Activation hook
- `home/shells/noctalia/default.nix` - Imports restart module

**Manual restart** (if needed):
```bash
pkill -x quickshell && hyprctl dispatch exec noctalia-shell
```

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
- **Portal**: App configuration

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
4. Open apps - sessions restored (Chrome, Firefox, Portal)

The age key is retrieved from 1Password on-the-fly - no manual key management needed!

### Troubleshooting

- **"Apps are running"**: Close Chrome/Firefox/Portal or use `--force`
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
