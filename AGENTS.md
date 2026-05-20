# Repo Memory

## HP Z2 Mini G1a Bluetooth support

Current host baseline:
- `z2-mini-g1a` is an HP Z2 Mini G1a Workstation with AMD Strix Halo and MediaTek MT7925 Wi-Fi/Bluetooth.
- On kernel 7.0.8, MT7925 Wi-Fi is usable with the existing `modules/hardware/mediatek-wifi.nix` workarounds.
- MT7925 Bluetooth currently fails to initialize with `Bluetooth: hci0: Failed to send wmt func ctrl (-22)`.
- The user does not use Bluetooth on this machine.

Policy:
- Do not carry a custom kernel, kernel patch, or kernel pin solely to fix MT7925 Bluetooth on `z2-mini-g1a`.
- Leave Bluetooth enabled so it can begin working automatically once upstream support lands in the normal NixOS kernel path.
- A simple non-kernel config fix is acceptable to consider, but avoid invasive workarounds unless the user says they now need Bluetooth.
- When revisiting, check whether `bluetoothctl show` reports a controller and whether the `wmt func ctrl (-22)` kernel log error is gone.

## Dell XPS 14 update checks

When the user asks whether Omarchy has any new Dell XPS 14 support worth copying, check Omarchy first before answering.

Preferred comparison targets:
- Their current Dell XPS hardware fix scripts
- Their recent commit history for Dell XPS and Panther Lake support
- Their current Panther Lake release notes or PR summaries when relevant

Look especially for changes in:
- Display refresh or panel workarounds
- Touchpad or haptic behavior
- Wi-Fi reliability fixes
- Audio enablement or kernel/backport strategy
- Camera enablement
- Thermals or power-management changes

Current baseline in this repo:
- Wi-Fi workaround is already present
- Haptic touchpad daemon approach is already present
- Do not replace the XPS haptic touchpad daemon with Omarchy's newer manual event-pulse approach unless new evidence shows it works on this exact machine. It found the devices but produced no haptic feedback here. Keep the local feature-restore approach that sets button/surface switches and intensity, then lets the touchpad generate feedback itself.
- Do not implement Omarchy's IPU7 camera fix in this repo for now. Their working path relies on out-of-tree/patched camera modules such as `intel_cvs` plus matching userspace relay packaging. The preferred policy is to wait for upstream kernel/libcamera/PipeWire support for the built-in Dell XPS IPU7 camera instead of carrying that stack locally.
- Display workaround keeps `xe.enable_panel_replay=0`; broader `xe.enable_psr=0` was removed after Omarchy dropped the temporary xe params
- Do not remove `xe.enable_panel_replay=0` on kernel 7.0.x. The Dell XPS 14 DA14260 Panel Replay quirk is expected in Linux 7.1, or via an explicit backport of upstream commits `45c77d4bf8d4` and `1de647abdfda9`.
- Audio is handled by kernel 7.0+ mainline SDCA support rather than Omarchy's older temporary blacklist

Default research approach:
1. Check Omarchy's current Dell XPS hardware scripts.
2. Check recent Omarchy commits touching Dell, XPS, or Panther Lake support.
3. Compare against this repo's XPS configuration and notes.
4. Only recommend changes that are newer than this repo's baseline or materially better supported.

When reporting back:
- Lead with whether there is anything new worth implementing.
- Call out exact recent dates for Omarchy changes when relevant.
- Distinguish between active current fixes and older superseded workarounds.
