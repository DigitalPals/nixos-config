# NixOS Configuration Notes

This document captures important configuration details and solutions to issues encountered in this NixOS setup.

## Plymouth + NVIDIA Issue

### Problem

Plymouth boot splash doesn't display on systems with NVIDIA GPUs, even though the service runs successfully.

### Root Cause

The issue is a **framebuffer timing race condition**:

1. `simpledrm` initializes first with the EFI framebuffer (fb0)
2. Plymouth starts immediately and attaches to simpledrm's fb0
3. NVIDIA driver loads 1-3 seconds later and **takes over fb0**
4. Plymouth loses its display connection

Boot log timeline showing the issue:
```
13:37:37 simpledrm: fb0: simpledrmdrmfb frame buffer device
13:37:37 Plymouth starts (on simpledrm)
13:37:38 nvidia-drm loading...
13:37:40 nvidia: fb0: nvidia-drmdrmfb frame buffer device  <-- takeover!
13:37:52 Plymouth quits (never displayed anything)
```

### Solution

Make Plymouth wait for udev to settle, ensuring NVIDIA's DRM device is ready before Plymouth starts.

In `modules/boot/limine-plymouth.nix`:
```nix
# Make Plymouth wait for DRM device (fixes NVIDIA framebuffer takeover issue)
boot.initrd.systemd.services.plymouth-start = {
  wants = [ "systemd-udev-settle.service" ];
  after = [ "systemd-udev-settle.service" ];
};
```

**Trade-off:** This adds a few seconds to boot time, but ensures Plymouth displays correctly.

### What Doesn't Work

- **Adding `simpledrm` to initrd modules** - It's built into the kernel, not a loadable module
- **`video=1920x1080@60` kernel parameter** - Fails with "User-defined mode not supported" on simpledrm
- **Not waiting for udev-settle** - Plymouth starts too early on simpledrm, then NVIDIA takes over

### Host-Specific Notes

- **kraken (NVIDIA RTX 5090)**: Requires the udev-settle workaround
- **G1a (AMD Strix Halo)**: Uses `hardware.amdgpu.initrd.enable = true`, no timing issues

## Plymouth Resolution on Limine (NixOS Module Limitation)

### Problem

Plymouth displays at low resolution (~1080p) on G1a despite having a 2880x1800 native display.

### Root Cause

Plymouth uses `simpledrm` by default on UEFI systems, which inherits the EFI framebuffer resolution set by the bootloader. Limine has a per-entry `resolution:` option that controls this, but the **NixOS Limine module doesn't expose it**.

The module only exposes `boot.loader.limine.style.interface.resolution`, which affects the Limine menu appearance, NOT the framebuffer passed to Linux.

### What Doesn't Work

- **`video=2880x1800@60` kernel parameter** - Only affects GPU drivers, not simpledrm (which can't change modes)
- **`plymouth.use-simpledrm=0`** - Disables simpledrm but causes black screen if GPU driver isn't ready in time
- **`interface.resolution` change** - Only affects Limine's menu, not the Linux framebuffer

### Current Status

Accepted limitation. Plymouth displays at whatever resolution the EFI firmware provides (typically 1024x768 or 1920x1080).

### Potential Fix

File a feature request at [nixpkgs](https://github.com/NixOS/nixpkgs/issues) to add `boot.loader.limine.resolution` option that sets the per-entry `resolution:` field in `limine.conf`.

## Home Manager Backup File Conflicts

### Problem

Home Manager fails with "Existing file would be clobbered by backing up" errors, particularly with Ghostty themes.

### Root Cause

`programs.ghostty.themes.*` creates files as regular files (not symlinks). Home Manager doesn't track these as "owned", so each activation tries to back them up. If a `.backup` file already exists, it fails.

### Solution

Use `xdg.configFile` with `force = true` instead of the built-in themes option:

```nix
# Don't use: programs.ghostty.themes.noctalia = { ... };

# Instead, manage the file directly:
xdg.configFile."ghostty/themes/noctalia" = {
  text = noctaliaTheme;
  force = true;  # Prevents backup conflicts
};
```

This creates a proper symlink to the Nix store and `force = true` prevents backup attempts.

## Configuration Structure

```
~/nixos-config/                 # Symlinked from /etc/nixos
├── flake.nix                    # Main flake with host definitions
├── hosts/
│   ├── kraken/                  # Desktop with NVIDIA RTX 5090
│   │   ├── default.nix          # Host-specific config (NVIDIA modules)
│   │   └── hardware-configuration.nix
│   └── G1a/                  # HP ZBook Ultra G1a with AMD Strix Halo
│       ├── default.nix          # Host-specific config (AMD GPU, TLP)
│       └── hardware-configuration.nix
├── modules/
│   ├── boot/
│   │   └── limine-plymouth.nix  # Shared bootloader + Plymouth config
│   ├── common.nix               # Shared system config
│   ├── desktop-environments.nix
│   └── hardware/
│       └── nvidia.nix           # NVIDIA driver config
├── home/
│   ├── home.nix                 # Main Home Manager config
│   ├── ghostty.nix              # Terminal config
│   └── ...
└── packages/
    └── plymouth-cybex/          # Custom Plymouth theme
```

## Key Configuration Files

### NVIDIA Initrd Modules (kraken)

`hosts/kraken/default.nix`:
```nix
boot.initrd.kernelModules = lib.mkForce [
  "nvidia"
  "nvidia_modeset"
  "nvidia_uvm"
  "nvidia_drm"
  "hid-generic"
  "usbhid"
];
```

### NVIDIA Kernel Parameters

`modules/hardware/nvidia.nix`:
```nix
boot.kernelParams = [
  "nvidia-drm.modeset=1"
  "nvidia-drm.fbdev=1"
];
```

### Plymouth Theme

The custom `cybex` theme uses `ModuleName=script` and requires the `script.so` plugin (included automatically).
