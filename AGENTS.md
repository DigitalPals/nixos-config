# Repo Memory

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
