# G1a configuration - HP ZBook Ultra G1a (Strix Halo)
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "G1a";

  # Fan spin-ups at "idle" are usually caused by brief CPU boost bursts that cross the
  # EC's fan threshold. On this machine, the default PPD "balanced" profile maps to
  # EPP=balance_performance, which is quite eager to boost.
  #
  # Policy:
  # - On AC: keep platform_profile=balanced, but set EPP=balance_power (quieter idle).
  # - On battery: set power-saver + EPP=power and disable CPU boost.
  #
  # This runs at boot and whenever AC online status changes.
  systemd.services.g1a-power-policy = {
    description = "G1a power policy (PPD + AMD P-State EPP + boost)";
    after = [ "power-profiles-daemon.service" ];
    wants = [ "power-profiles-daemon.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "oneshot";
    };

    script = ''
      set -euo pipefail

      ac_online=0
      if [ -r /sys/class/power_supply/AC/online ]; then
        ac_online="$(cat /sys/class/power_supply/AC/online || echo 0)"
      fi

      if [ "$ac_online" = "1" ]; then
        # Keep platform_profile balanced, but dial back boost eagerness.
        ${pkgs.power-profiles-daemon}/bin/powerprofilesctl set balanced || true
        for p in /sys/devices/system/cpu/cpufreq/policy*/energy_performance_preference; do
          [ -w "$p" ] || continue
          echo balance_power > "$p" || true
        done
        if [ -w /sys/devices/system/cpu/cpufreq/boost ]; then
          echo 1 > /sys/devices/system/cpu/cpufreq/boost || true
        fi
      else
        # Battery: prioritize quietness and efficiency.
        ${pkgs.power-profiles-daemon}/bin/powerprofilesctl set power-saver || true
        for p in /sys/devices/system/cpu/cpufreq/policy*/energy_performance_preference; do
          [ -w "$p" ] || continue
          echo power > "$p" || true
        done
        if [ -w /sys/devices/system/cpu/cpufreq/boost ]; then
          echo 0 > /sys/devices/system/cpu/cpufreq/boost || true
        fi
      fi
    '';
  };

  systemd.paths.g1a-power-policy = {
    description = "Trigger G1a power policy when AC online changes";
    wantedBy = [ "multi-user.target" ];
    pathConfig = {
      PathChanged = "/sys/class/power_supply/AC/online";
      Unit = "g1a-power-policy.service";
    };
  };

  # === AMD Strix Halo (RDNA 3.5) GPU Configuration ===
  # Enable official amdgpu initrd support for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # Early boot kernel modules (order matters for proper initialization)
  # - GPU modules first: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  # Using mkForce to override any defaults from imported modules
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"       # GPU: early KMS for Plymouth and console
    "hid-generic"  # Input: generic HID driver for keyboards
    "usbhid"       # Input: USB HID for external keyboards
  ];

  # AMD GPU Configuration
  # Note: We do NOT add libva-mesa-driver or amdvlk to extraPackages because:
  # - libva-mesa-driver: VA-API is already included in Mesa by default
  # - amdvlk: Being discontinued, and Mesa RADV is faster and more stable
  # Mesa RADV (Vulkan) and radeonsi (VA-API) are automatically available via hardware.graphics.enable
  #
  # If you need explicit VA-API driver selection, set environment variable:
  # environment.sessionVariables.LIBVA_DRIVER_NAME = "radeonsi";

  # LUKS configuration is handled by disko (modules/disko/G1a.nix)
  # Disko sets allowDiscards and bypassWorkqueues automatically

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';
}
