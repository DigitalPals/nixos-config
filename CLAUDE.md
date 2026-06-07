# NixOS Configuration Notes

Configuration details and solutions to issues in this NixOS setup.

## Configuration Structure

```
~/nixos-config/                     # Symlinked from /etc/nixos
├── flake.nix                       # Main flake with host definitions
├── hosts/
│   ├── kraken/                     # Desktop: AMD Ryzen 9 9950X3D, NVIDIA RTX 5090, 96GB RAM
│   ├── G1a/                        # HP ZBook Ultra G1a: AMD Ryzen AI MAX+ PRO 395, Radeon 8060S, 64GB RAM
│   ├── z2-mini-g1a/                # HP Z2 Mini G1a: AMD Ryzen AI MAX+ PRO 395 (Strix Halo), Apple Studio Display
│   ├── proart/                     # ASUS ProArt P16 OLED: AMD Ryzen AI 9 HX 370, Radeon 890M + RTX 5090, 64GB RAM
│   └── xps/                        # Dell XPS 14 DA14260: Intel Core Ultra X7 358H, Intel Arc (Xe3), 32GB RAM
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
│       └── intel.nix               # Intel GPU config (XPS / Panther Lake)
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
| `forge apps setup-keys` | Import missing local keys from 1Password |
| `forge keys setup` | Import missing local keys from 1Password |

Note: `forge browser` is still supported as an alias for `forge apps`.

### `forge update` NVIDIA Kernel Pre-flight

On NVIDIA hosts (e.g. **kraken**, **proart**), when a flake update bumps the
kernel (via nixpkgs or an explicit kernel input), `forge update` builds the
host's configured NVIDIA driver (`config.hardware.nvidia.package`) against the
new kernel **before** the full system rebuild. A kernel/driver mismatch fails at
build time, so this is a real build, not `nix build --dry-run` (which only
evaluates and would miss the failure). The successful build is reused by the
subsequent rebuild, so the only extra cost is when the driver is genuinely not
ready.

If the driver won't build, `forge update` restores `flake.lock` to the previous
inputs, skips the rebuild, and reports a partial update with a follow-up
warning — leaving the system on the known-good kernel instead of switching into
one whose GPU driver won't build. Override with `forge update --skip-nvidia-check`.

Implementation: `packages/forge/src/commands/update/nvidia.rs`, wired into
`run_update` in `packages/forge/src/commands/update/mod.rs`.

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
- `scroll_mouse X` - Set mouse scroll factor for window (e.g., `scroll_mouse 0.2`)
- `scroll_touchpad X` - Set touchpad scroll factor for window

**Implementation:** Window rules are in:
- `home/hyprland/looknfeel.nix` - Most window rules
- `home/hyprland/input.nix` - Scroll input rule for Ghostty

**Documentation:** https://wiki.hypr.land/Configuring/Window-Rules/

## Plymouth + NVIDIA Issue

**Problem:** Plymouth doesn't display on NVIDIA systems due to framebuffer timing race - `simpledrm` initializes first, Plymouth attaches, then NVIDIA takes over fb0.

**Solution:** Make Plymouth wait for udev to settle (`modules/boot/limine-plymouth.nix:51-54`).

**Trade-off:** Adds a few seconds to boot time.

**What doesn't work:** Adding `simpledrm` to initrd (builtin), `video=` kernel params (simpledrm ignores them), skipping udev-settle.

**Host notes:**
- **kraken (NVIDIA):** Requires udev-settle workaround
- **G1a (AMD):** Uses `hardware.amdgpu.initrd.enable`, no timing issues
- **z2-mini-g1a (AMD):** Opposite of G1a — `amdgpu` is kept OUT of the initrd; see "Z2 Mini G1a Boot Display" below

## Plymouth Resolution on Limine

**Problem:** Plymouth displays at low resolution (~1080p) regardless of native display.

**Root cause:** NixOS Limine module doesn't expose per-entry `resolution:` option. The `interface.resolution` only affects the menu, not the Linux framebuffer.

**Status:** Accepted limitation. Consider filing nixpkgs feature request for `boot.loader.limine.resolution`.

## Z2 Mini G1a Boot Display (Thunderbolt-only Display)

**Host:** HP Z2 Mini G1a (AMD Strix Halo desktop)

**Problem:** The graphical Plymouth LUKS unlock screen flashes up briefly, then
the Studio Display goes black and only returns after physically unplugging and
replugging the monitor.

**Hardware context:** The Z2 Mini is a desktop with NO internal panel. Its only
display is an Apple Studio Display attached over a **USB4/Thunderbolt
DisplayPort tunnel** that the UEFI firmware sets up before Linux boots. This is
fundamentally different from the G1a laptop, which has an internal eDP panel
wired directly to the APU.

**The reliable initrd display is `simpledrm`** — it just scans out the
framebuffer the firmware already lit on the Studio Display, and Plymouth draws
the LUKS prompt on it. For that prompt to *stay* visible, two kernel drivers
must be kept out of the initrd, because each disturbs the firmware framebuffer
or its DP tunnel before unlock:

**Cause 1 — `amdgpu` in the initrd.** Loading amdgpu evicts `simpledrm` from
`fb0` and starts a full modeset, but the Thunderbolt DP tunnel is not ready that
early (`amdgpu ... [drm] Cannot find any crtc or sizes`). The screen blanks.

**Cause 2 — `thunderbolt` in the initrd.** The thunderbolt driver's `host_reset`
(module param, default `true`) resets the USB4 host router on probe to clear the
firmware's tunnels. That tears down the DP tunnel feeding `simpledrm`, so the
Studio Display drops off the bus mid-boot — right at the LUKS prompt:

```
thunderbolt 0-2: device disconnected       <- ~19s into boot, at the LUKS prompt
thunderbolt 0-2: Apple Studio Display XDR   <- re-enumerates ~5s later
```

With nothing in the initrd to re-light the display, it stays black until a
physical re-plug.

**Solution:** Keep BOTH drivers out of the initrd; they load normally in stage 2
after unlock, where amdgpu re-lights the display.

```nix
# hosts/z2-mini-g1a/default.nix
hardware.amdgpu.initrd.enable = false;
boot.initrd.kernelModules = lib.mkForce [ "hid-generic" "usbhid" ];  # no amdgpu

# hosts/z2-mini-g1a/hardware-configuration.nix
# "thunderbolt" removed from boot.initrd.availableKernelModules:
boot.initrd.availableKernelModules = [ "nvme" "xhci_pci" "ahci" "usbhid" "uas" "sd_mod" "btrfs" ];
```

The initrd needs neither driver: the root disk is internal NVMe (not behind
Thunderbolt) and the keyboard is on a native AMD xHCI (`xhci_pci`), not a
Thunderbolt-tunneled controller.

**Do NOT "align" this host with G1a.** G1a's GPU-first initrd works because it
has an internal eDP panel amdgpu can light immediately. The Z2 Mini's display
topology (Thunderbolt DP tunnel) is not comparable.

**Expected behavior:** The Plymouth LUKS prompt is stable on `simpledrm`. After
entering the passphrase there is a brief (~3s) black screen while stage 2 loads
`thunderbolt` + `amdgpu` and re-negotiates the Thunderbolt DP tunnel, then the
desktop appears. That post-unlock flash is intrinsic to the Thunderbolt display
path and is not a failure.

**The bootloader is not involved.** Limine (or systemd-boot/GRUB) only hands off
to the kernel; the amdgpu/Thunderbolt timing is identical regardless of
bootloader. Switching bootloaders does not help.

**What doesn't work:**
- `amdgpu` in the initrd / `hardware.amdgpu.initrd.enable = true` — evicts
  simpledrm and loses the race with the Thunderbolt DP tunnel
- `thunderbolt` in the initrd — `host_reset` blanks the display mid-boot
- `plymouth.use-simpledrm=0` — forces Plymouth to wait for amdgpu KMS, the slow
  path here; makes it worse
- Forced `video=DP-6:...` modes — the connector is not up early enough to pin
- Disabling Plymouth entirely — works, but loses the graphical splash

**Debugging commands:**
```bash
# Boot display + Thunderbolt sequence (watch for "device disconnected")
sudo dmesg | grep -iE "amdgpu|thunderbolt|simpledrm|crtc|DP resource|HPD"

# Confirm the keyboard xHCI is native (no "thunderbolt" in the path)
readlink -f /sys/bus/usb/devices/usb5

# Which connector the Studio Display is on (DP-6 at time of writing)
for s in /sys/class/drm/card*/status; do echo "$s: $(cat $s)"; done
```

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

**Current status:** Using `linuxPackages_latest` from nixpkgs. Re-test any older Strix Halo suspend notes against the currently evaluated kernel before reintroducing a per-host kernel override.

**Kernel config:** Kernel is set centrally in `modules/boot/limine-plymouth.nix` to `linuxPackages_latest`. No per-host override needed.

**Older kernel 6.18 regression:** Kernel 6.18.x had a VPE regression that could break suspend even with `amd_iommu=off`. Treat this as historical context unless the current latest kernel shows the same failure mode.

**Fallback option:** If suspend failures are unacceptable, override with 6.12 LTS in `hosts/G1a/default.nix`:
```nix
boot.kernelPackages = lib.mkForce pkgs.linuxPackages_6_12;
```

**When revisiting:** Check current suspend behavior on `linuxPackages_latest` first, then decide whether any workaround is still needed.

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

**Root cause:** Multiple bugs in the mt7925e driver observed on earlier kernels:
- Commit `cb1353ef34735` ("wifi: mt76: mt7925: integrate *mlo_sta_cmd and *sta_cmd") causes speed drops on some routers
- CLC (Country Location Code) feature causes instability
- ASPM power management interferes with driver operation
- WiFi power save causes disconnects

**Current workarounds** (in `modules/hardware/mediatek-wifi.nix`, imported by G1a and ProArt):
```nix
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

**When revisiting:** Check current kernel version with `nix eval .#nixosConfigurations.G1a.config.boot.kernelPackages.kernel.version --raw`. Test without `disable_clc=1` when mt76/MT7925 changelogs mention CLC or stability fixes.

**References:**
- [Ubuntu Bug #2118937](https://bugs.launchpad.net/ubuntu/+source/linux/+bug/2118937) - Intermittent connection loss
- [Ubuntu Bug #2118755](https://bugs.launchpad.net/ubuntu/+source/linux/+bug/2118755) - 6GHz instability
- [linux-mediatek regression report](https://lists.infradead.org/pipermail/linux-mediatek/2025-August/096746.html) - Bisected to specific commit
- [Framework Community thread](https://community.frame.work/t/issues-with-mediatek-mt7925-rz717-wi-fi-card/75815)

## G1a Fan Cycling Fix

**Host:** HP ZBook Ultra G1a (AMD Strix Halo)

**Problem:** Fans cycle rapidly between 0 and ~1000-2100 RPM every ~30 seconds at idle (~40°C). The EC fan curve has no hysteresis and reacts to brief CPU boost micro-bursts.

**Root cause:** The EC register MFAC (offset 0xD5) controls the minimum fan speed on AC power. Factory default is 255 (0xFF = no minimum enforced), allowing fans to drop to 0 RPM between boost bursts.

**Solution: BIOS/firmware update**

Update to BIOS **01.04.05** (or later) which includes **EC firmware 35.33.00**. The updated EC firmware fixes the fan hysteresis — fans hold steady at ~1800 RPM at idle instead of cycling between 0 and 2100 RPM.

Download from: [HP ZBook Ultra G1a Support](https://support.hp.com/be-nl/drivers/hp-zbook-ultra-g1a-14-inch-mobile-workstation-pc/2102737532)

Previous EC firmware (35.2F / 53.47) had no hysteresis in the fan curve. Updated EC firmware (35.33 / 53.51) maintains stable fan speeds.

Check versions:
```bash
cat /sys/class/dmi/id/bios_version          # Should be X89 Ver. 01.04.05+
cat /sys/class/dmi/id/ec_firmware_release    # Should be 53.51+
```

**Software mitigation** (`hosts/G1a/default.nix`):
The `g1a-power-policy` service reduces boost eagerness via EPP tuning (`balance_power` on AC, `power` on battery), which complements the firmware fix by reducing the frequency of temperature spikes.

**What doesn't work (tested on EC 35.2F):**
- **Direct EC register writes** (`ec_sys` with `write_support=1`): The EC ignores all host writes to MFAC (0xD5) and CFAN (0xD6). Even 8000 writes/sec have no effect — the EC hardware write-protects these registers.
- **ACPI method calls** (`acpi_call` → `\_SB.PCI0.LPCB.EC0.KFCL`): The method executes but the EC still ignores the register changes.
- **HP WMI BIOS attributes** (`hp-bioscfg`): All fan-related writes return error 0x4 "Invalid command type". All WMI GUIDs report setable=0.
- **Continuous EC hammering**: Even writing MFAC in a tight loop for 2 seconds (7800+ writes) has zero effect.

**DSDT reference:** The `KFCL` method (DSDT line 11664) writes Arg0→CFAN(0xD6) and Arg1→MFAC(0xD5). The EC ACPI path is `\_SB.PCI0.LPCB.EC0`.

## ProArt P16 Hibernate (LVM) - Currently Non-Functional

**Host:** ASUS ProArt P16 OLED (AMD Ryzen AI 9 HX 370 + NVIDIA RTX 5090, 64GB RAM)

**Status:** Hibernate does NOT work on ProArt P16 due to RTX 5090 (Blackwell) requiring open NVIDIA kernel modules, which fail during hibernate resume. Use suspend (s2idle) instead.

**Why it doesn't work:**
- RTX 5090 (Blackwell architecture) REQUIRES open NVIDIA kernel modules - proprietary modules fail to load
- Open NVIDIA modules fail during hibernate resume with error -5 in `nv_pmops_freeze`
- This is a hardware/driver limitation with no current workaround

**Disk layout** (`modules/disko/proart.nix`) - retained for future driver support:
```
ESP (2GB, FAT32, /boot)
└── LUKS2 (remaining)
    └── LVM volume group "vg"
        ├── swap (66GB) → hibernate (currently unused)
        └── root (remaining) → btrfs with subvolumes
```

**Configuration:**
- Disko: `modules/disko/proart.nix` (standalone, doesn't import default.nix)
- Host: `hosts/proart/default.nix` with `boot.resumeDevice = "/dev/vg/swap"`
- zramSwap disabled (swap partition still useful for memory pressure)

**Why LVM was chosen:**
- Dedicated swap partition doesn't need `resume_offset` calculation
- No btrfs swapfile edge cases with systemd initrd
- Single LUKS unlock, then LVM provides both swap and root
- Configuration retained for potential future NVIDIA driver support

**Available power management:**
```bash
# Suspend (works)
systemctl suspend

# Hibernate (does NOT work - will fail on resume)
# systemctl hibernate
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

**Note:** Hibernate does NOT work on ProArt P16 - the RTX 5090 requires open NVIDIA modules which fail during hibernate resume. Use suspend instead.

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

## AMD NPU (XDNA) Support Status

**Host:** ASUS ProArt P16 OLED (AMD Ryzen AI 9 HX 370)

**Hardware:** 50 TOPS NPU (XDNA architecture)

**Kernel support:** The `amdxdna` driver is included in kernel 6.14+ and loads automatically. Firmware is included in `linux-firmware`.

**Verification:**
```bash
lspci | grep -i "Processing accelerator"  # Check NPU detected
lsmod | grep amdxdna                       # Check driver loaded
```

**User-space limitations:** AMD Ryzen AI SDK is Windows-only as of January 2026. For AI inference, use the NVIDIA RTX 5090 with CUDA/TensorRT.

## ProArt P16 Function Keys (Known Limitation)

**Host:** ASUS ProArt P16 OLED (H7606WX)

**Problem:** Hardware Fn keys for brightness (F3/F4) and volume (F7/F8/F9) do not generate key events. The kernel's `asus-wmi` driver does not yet support this model.

**Status:** Waiting for upstream kernel support (expected kernel 6.20+).

**Workaround:** The keybinds in `home/hyprland/bindings.nix` handle XF86 keys when they work. For models where Fn keys don't generate events, use alternative bindings or external keyboard.

## ProArt P16 Monitoring Commands

### Temperature and Power
```bash
sensors | grep -E "(Tctl|edge|fan)"        # CPU/GPU temps and fans
cat /sys/bus/pci/devices/0000:64:00.0/power/runtime_status  # NVIDIA dGPU state
```

### Power Management
```bash
asusctl profile -p                          # Current platform profile
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_driver     # CPU driver
cat /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference  # EPP
```

### Suspend Debug
```bash
journalctl -b | grep -iE "(suspend|resume|s2idle)"  # Last suspend
cat /proc/acpi/wakeup | grep enabled                 # Wakeup sources
```

## Dell XPS 14 (Panther Lake) Configuration

**Host:** Dell XPS 14 DA14260 (Intel Core Ultra X7 358H, Intel Arc Xe3, 32GB LPDDR5x)

### Panther Lake Display

**Problem:** The Dell XPS OLED panel can drop to 10Hz and window dragging becomes laggy after suspend/resume on Panther Lake systems using the `xe` kernel module. Root cause is Panel Replay.

**Status:** `xe.enable_panel_replay=0` remains enabled in `modules/hardware/intel.nix` for the DA14260 OLED panel. The broader `xe.enable_psr=0` workaround was removed after Omarchy dropped it from their RC branch. Re-test after kernel or BIOS changes before changing display power-saving overrides.

### Dell XPS Haptic Touchpad

**Problem:** Synaptics haptic touchpad (06CB:D01A) loses haptic feedback after suspend/resume due to aggressive I2C power management. The kernel also does not auto-generate haptic pulses via the HID Manual Trigger path.

**Solution:** Two-part fix in `hosts/xps/default.nix`:
1. Udev rules to keep I2C controller and touchpad device powered (`power/control="on"`)
2. Userspace haptic feedback daemon that sends click pulses through hidraw

### Dell XPS Audio

**Status:** Audio is fully supported in kernel 7.0+ via mainline SDCA drivers. No patches or workarounds needed. The 6.19-era SDCA backport infrastructure has been removed.

### Intel WiFi 7 BE211

Uses the `iwlwifi` driver. WiFi 7 (EHT/802.11be) is disabled via `options iwlwifi disable_11be=Y` in `hosts/xps/default.nix` because the BE211 driver has shown poor real-world performance even when the kernel accepts the connection. Falling back to WiFi 6/802.11ax is stable. WiFi power save is globally disabled in `modules/common.nix`.

**When to re-test:** Try removing `disable_11be=Y` on a stable 7.x release or when iwlwifi changelog mentions BE211/EHT fixes.

### Wireless Regulatory Domain

The regulatory domain (NL) is set via kernel parameter rather than a modprobe option, because `cfg80211` is built into the testing kernel and modprobe options have no effect on built-in modules:

```nix
boot.kernelParams = [ "cfg80211.ieee80211_regdom=NL" ];
```

## Key NVIDIA Settings

All NVIDIA config is in `modules/hardware/nvidia.nix`:
- **Open kernel modules** (`open = true`) - Default for better compatibility with newer cards
- Modesetting + power management
- Initrd modules: `nvidia`, `nvidia_modeset`, `nvidia_uvm`, `nvidia_drm`
- Kernel params: `nvidia-drm.modeset=1`, `nvidia-drm.fbdev=1`
- Wayland env vars: `GBM_BACKEND`, `__GLX_VENDOR_LIBRARY_NAME`, `NIXOS_OZONE_WL`

**Host-specific driver selection:**
- **kraken**: Uses open drivers (default) - desktop without hibernate needs
- **proart**: Uses open drivers (REQUIRED for RTX 5090 Blackwell) - hibernate does NOT work

**RTX 5090 (Blackwell) Hibernate Limitation:**
The RTX 5090 (GB202) REQUIRES open NVIDIA kernel modules - proprietary modules fail to load on Blackwell architecture. However, open modules fail during hibernate resume with error -5 in `nv_pmops_freeze`. This is a hardware/driver limitation with no current workaround.

**Available power states on ProArt P16:**
- **Suspend (s2idle)**: Works correctly with GPIO 16 fix
- **Hibernate**: Does NOT work - use suspend instead

The LVM swap configuration is retained for potential future driver support.

Host `kraken` uses `lib.mkForce` to ensure all modules load together (`hosts/kraken/default.nix:15-22`).

## Shell Restart on Store Path Change

**Problem:** After `nixos-rebuild switch`, the running quickshell process has old `/nix/store/...` paths baked in, while IPC commands (like `noctalia-shell ipc call launcher toggle`) reference the new path. This causes IPC failures with "No running instances" errors.

**Root cause:** Quickshell processes embed their store path at startup. When the package updates, the symlink at `~/.config/quickshell/noctalia-shell` points to the new path, but the running process still has the old path.

**Solution:** Home Manager activation hook that automatically restarts the shell when store paths change. This runs during the `nixos-rebuild switch` activation phase, ensuring a single restart regardless of how the rebuild was triggered (Forge, AI agents, or manual `nixos-rebuild`).

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
The generated `~/.ssh/config` points OpenSSH at `~/.1password/agent.sock`; it
does not require or create a local `~/.ssh/id_ed25519` private key.

## App Profile Backup/Restore

Encrypted app profile backup system using Age encryption and a private GitHub repository. Supports 1Password integration for automatic age key retrieval and SSH agent authentication across machines.

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

The age key is retrieved from 1Password on-the-fly, and Git uses the 1Password
SSH agent for the private backup repo. No local SSH private key file is needed.

### Troubleshooting

- **"Apps are running"**: Close Chrome/Firefox/Portal or use `--force`
- **"1Password not unlocked"**: Open 1Password app and sign in
- **"op: command not found"**: Rebuild to install 1Password CLI
- **"Git push failed" / "Permission denied (publickey)"**: Check 1Password is unlocked and the SSH key is available in the 1Password SSH agent
- **"Config not found"**: Enable `programs.app-backup` and rebuild

### Security Notes

Profile archives contain session cookies, auth tokens, and potentially saved passwords. The archives are encrypted with Age before being pushed to GitHub.

- Age private key is stored in 1Password, never on disk
- Key is retrieved on-the-fly and never written to filesystem
- LUKS disk encryption (enabled by default) provides additional protection
- Decrypted archives are only created in temp directories and shredded after use
