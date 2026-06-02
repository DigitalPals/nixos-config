# MediaTek MT7925 WiFi 7 configuration
# Used in: G1a (HP ZBook Ultra G1a), z2-mini-g1a (HP Z2 Mini G1a), proart (ASUS ProArt P16)
#
# Known MT7925 issues observed on earlier kernels:
# - Commit cb1353ef34735 causes speed drops on some routers
# - CLC (Country Location Code) feature causes instability
# - ASPM power management interferes with driver operation
# - WiFi power save causes disconnects
#
# These workarounds should be retested periodically on current latest kernels.
# Check: nix eval .#nixosConfigurations.G1a.config.boot.kernelPackages.kernel.version --raw
# Test without disable_clc=1 when mt76/MT7925 changelogs mention CLC fixes.
# See CLAUDE.md "MT7925 WiFi Stability" section for details.
{ config, pkgs, lib, ... }:

{
  # Load driver explicitly - udev auto-loading can be unreliable
  boot.kernelModules = [ "mt7925e" ];

  # Disable ASPM in driver for stable suspend/resume
  # Disable CLC to prevent random disconnects on affected firmware/router combos.
  boot.extraModprobeConfig = ''
    options mt7925e disable_aspm=1
    options mt7925-common disable_clc=1
  '';

  # Use iwd instead of wpa_supplicant for faster WiFi reconnection after suspend
  # iwd handles suspend/resume much better than wpa_supplicant
  networking.wireless.iwd = {
    enable = true;
    settings = {
      General = {
        EnableNetworkConfiguration = false;  # Let NetworkManager handle IP config
      };
      Settings = {
        AutoConnect = true;
      };
    };
  };
  networking.networkmanager.wifi.backend = "iwd";
  networking.networkmanager.wifi.scanRandMacAddress = false;
}
