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

  # power-profiles-daemon is currently broken on this machine with amd-pstate-epp:
  # it fails when writing policy*/boost and then reports/sticks to "power-saver"
  # even after the actual kernel policy has been corrected via sysfs.
  services.power-profiles-daemon.enable = lib.mkForce false;

  # Keep this laptop out of the kernel/firmware "power-saver" path.
  #
  # powerprofilesctl currently fails on this host when it tries to toggle AMD boost,
  # so we apply the durable bits directly through sysfs instead:
  # - always keep the platform profile on "balanced"
  # - on AC: prefer snappier boosting with EPP=balance_performance
  # - on battery: stay efficient with EPP=balance_power, but do not drop to power-saver
  #
  # This runs at boot and whenever AC online status changes.
  systemd.services.g1a-power-policy = {
    description = "G1a power policy (platform profile + AMD P-State EPP)";
    # Use graphical.target (not multi-user.target) to avoid a systemd ordering cycle:
    # multi-user.target → g1a-power-policy → PPD → After=multi-user.target → cycle!
    # graphical.target comes after multi-user.target, breaking the cycle.
    wantedBy = [ "graphical.target" ];

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
        if [ -w /sys/firmware/acpi/platform_profile ]; then
          echo balanced > /sys/firmware/acpi/platform_profile || true
        fi
        for p in /sys/devices/system/cpu/cpufreq/policy*/energy_performance_preference; do
          [ -w "$p" ] || continue
          echo balance_performance > "$p" || true
        done
      else
        if [ -w /sys/firmware/acpi/platform_profile ]; then
          echo balanced > /sys/firmware/acpi/platform_profile || true
        fi
        for p in /sys/devices/system/cpu/cpufreq/policy*/energy_performance_preference; do
          [ -w "$p" ] || continue
          echo balance_power > "$p" || true
        done
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

  # Fingerprint reader (Synaptics FS7606, power button)
  services.fprintd.enable = true;
  security.pam.services = {
    polkit-1.fprintAuth = true;
    sudo.fprintAuth = true;
  };

  # Fingerprint reader suspend/resume: force USB reset on resume for this device
  # Without this, the device fails to resume from s2idle (kernel error -107,
  # endpoint stalled) and fprintd can't communicate with it.
  # The 'b' flag sets USB_QUIRK_RESET_RESUME for this specific device.
  #
  # Display stability workarounds for this Strix Halo laptop:
  # - disable Panel Replay and PSR on the internal panel because recent AMD
  #   laptops repeatedly work around amdgpu flip_done stalls that way
  # - disable scatter/gather display for docked external monitors, another
  #   common workaround for AMD APU external display corruption/freezes
  # - avoid forcing PCIe ASPM globally; let firmware + drivers negotiate it
  boot.kernelParams = lib.mkAfter [
    "amdgpu.dcdebugmask=0x410"
    "amdgpu.sg_display=0"
    "usbcore.quirks=06cb:0106:b"
  ];

  # Note: fprintd is NOT stopped before suspend. The USB_QUIRK_RESET_RESUME
  # kernel quirk handles device recovery, and fprintd 1.94+ has built-in
  # suspend/resume support. Stopping fprintd would break the lock screen's
  # active PAM session, causing it to fall back to password-only on resume.

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

  # === Hardware-specific udev fixes ===
  services.udev.extraRules = ''
    # Prevent USB autosuspend for Synaptics fingerprint reader (06cb:0106)
    ACTION=="add", SUBSYSTEM=="usb", ATTR{idVendor}=="06cb", ATTR{idProduct}=="0106", ATTR{power/control}="on"
  '';
}
