# Dell XPS 14 DA14260 (Intel Panther Lake)
# CPU: Intel Core Ultra X7 358H (16 cores, up to 4.8 GHz)
# GPU: Intel Arc (Xe3 integrated)
# Display: 14.0" 2.8K OLED touch, 20-120Hz
# RAM: 32GB LPDDR5x 9600 MT/s
# WiFi: Intel Wi-Fi 7 BE211
{ config, pkgs, lib, ... }:
let
  hurricanOmarchyEnabling = pkgs.fetchFromGitHub {
    owner = "TsaiGaggery";
    repo = "hurrican_omarchy_enabling";
    rev = "2e3aba622c03a2510da2b69a57ea00111f9759c9";
    hash = "sha256-b1468p0YcZA8gp0XMPIVg0BpswM5vUYenUdmZrkbuWA=";
  };

  sdcaPatchNames = [
    "0001-ASoC-SDCA-functions-Fix-confusing-cleanup.h-syntax.patch"
    "0002-ASoC-SDCA-Add-ASoC-jack-hookup-in-class-driver.patch"
    "0003-ASoC-SDCA-Replace-use-of-system_wq-with-system_dfl_w.patch"
    "0004-ASoC-SDCA-Add-SDCA-IRQ-enable-disable-helpers.patch"
    "0005-ASoC-SDCA-Add-basic-system-suspend-support.patch"
    "0006-ASoC-SDCA-Device-boot-into-the-system-suspend-proces.patch"
    "0007-ASoC-SDCA-Add-lock-to-serialise-the-Function-initial.patch"
    "0008-ASoC-SDCA-Tidy-up-some-memory-allocations.patch"
    "0009-ASoC-SDCA-Handle-CONFIG_PM_SLEEP-not-being-set.patch"
    "0010-ASoC-SDCA-Add-NO_DIRECT_COMPLETE-flag-to-class-drive.patch"
    "0011-ASoC-sdca-Fix-missing-regmap-dependencies-in-Kconfig.patch"
    "0012-ASoC-SDCA-Rearrange-FDL-file-messages.patch"
    "0013-ASoC-SDCA-Add-regmap-defaults-for-specification-defi.patch"
    "0014-ASoC-SDCA-Limit-values-user-can-write-to-Selected-Mo.patch"
    "0015-ASoC-SDCA-Fix-comments-for-sdca_irq_request.patch"
    "0016-ASoC-SDCA-Add-allocation-failure-check-for-Entity-na.patch"
    "0017-ASoC-SDCA-Fix-NULL-pointer-dereference-in-sdca_jack_.patch"
  ];

  mkSdcaPatch = name: {
    inherit name;
    patch = "${hurricanOmarchyEnabling}/sdca-backport-patches/${name}";
  };
in
{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    # Intel GPU (intel.nix) is imported via extraModules in flake.nix
  ];

  networking.hostName = "xps";

  # WiFi regulatory domain (enables proper 5GHz/6GHz channels and power levels)
  hardware.wirelessRegulatoryDatabase = true;
  boot.extraModprobeConfig = ''
    options cfg80211 ieee80211_regdom=NL
    options v4l2loopback video_nr=50 card_label="Intel IPU7 Camera" exclusive_caps=1
  '';

  # Intel CPU configuration
  hardware.cpu.intel.updateMicrocode = true;
  boot.kernelModules = [ "kvm-intel" "coretemp" "v4l2loopback" ];
  boot.extraModulePackages = [ config.boot.kernelPackages.v4l2loopback ];

  # Intel thermald for thermal management (Dell DPTF integration)
  services.thermald.enable = true;

  # Panther Lake audio support matches Omarchy's SDCA backports on 6.19.x.
  boot.kernelPatches = lib.mkIf (lib.hasPrefix "6.19" config.boot.kernelPackages.kernel.version) (
    map mkSdcaPatch sdcaPatchNames
  );

  environment.systemPackages = with pkgs; [
    icamerasrcIpu75xa
    ipu7CameraBins
    ipu7CameraHal
    v4l-utils
  ];

  environment.etc = {
    "intel_lpmd".source = "${pkgs.intelLpmd}/etc/intel_lpmd";
    "dbus-1/system.d/org.freedesktop.intel_lpmd.conf".source =
      "${pkgs.intelLpmd}/etc/dbus-1/system.d/org.freedesktop.intel_lpmd.conf";
  };

  # Early boot kernel modules (order matters for proper initialization)
  # - GPU module first: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "xe"           # GPU: Intel Xe3 for early KMS/Plymouth
    "hid-generic"  # Input: generic HID driver for keyboards
    "usbhid"       # Input: USB HID for external keyboards
  ];

  # === Haptic Touchpad Fix ===
  # Dell XPS 2024+ uses a Synaptics haptic touchpad (06CB:D01A) that loses
  # haptic feedback after suspend/resume due to aggressive I2C power management.
  # Fix sourced from Omarchy Linux:
  # https://github.com/basecamp/omarchy/blob/main/install/config/hardware/fix-dell-xps-haptic-touchpad.sh

  # Keep I2C HID controller powered (prevent suspend from cutting power)
  services.udev.extraRules = ''
    # Synaptics haptic touchpad: keep I2C controller active to preserve haptics
    ACTION=="add|change", SUBSYSTEM=="i2c", ATTRS{idVendor}=="06cb", ATTRS{idProduct}=="d01a", ATTR{power/control}="on"

    # Make the virtual camera device accessible to the desktop session
    KERNEL=="video50", GROUP="video", MODE="0660"

    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="*::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';

  # Rebind I2C HID driver on resume to reinitialize haptic feedback
  systemd.services.xps-haptic-touchpad = {
    description = "Rebind haptic touchpad I2C HID driver after resume";
    after = [ "suspend.target" "hibernate.target" ];
    wantedBy = [ "suspend.target" "hibernate.target" ];

    serviceConfig = {
      Type = "oneshot";
      ExecStartPre = "${pkgs.coreutils}/bin/sleep 1";
    };

    script = ''
      set -euo pipefail

      # Find the I2C HID device for the Synaptics haptic touchpad
      for dev in /sys/bus/i2c/drivers/i2c_hid_acpi/i2c-*; do
        [ -d "$dev" ] || continue
        # Unbind and rebind to reinitialize haptic feedback
        devname="$(basename "$dev")"
        echo "$devname" > /sys/bus/i2c/drivers/i2c_hid_acpi/unbind 2>/dev/null || true
        sleep 0.5
        echo "$devname" > /sys/bus/i2c/drivers/i2c_hid_acpi/bind 2>/dev/null || true
      done
    '';
  };

  systemd.services.xps-wireless-regdom = {
    description = "Apply XPS wireless regulatory domain";
    after = [ "NetworkManager.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "oneshot";
    };

    script = ''
      ${pkgs.iw}/bin/iw reg set NL || true
    '';
  };

  systemd.services.intel-lpmd = {
    description = "Intel Low Power Mode Daemon";
    after = [ "dbus.service" "upower.service" ];
    wants = [ "dbus.service" "upower.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      ExecStart = "${pkgs.intelLpmd}/bin/intel_lpmd --systemd --dbus-enable";
      Restart = "on-failure";
      Type = "simple";
    };
  };

  # === Audio: SOF/HDA conflict workaround ===
  # Omarchy currently carries SDCA backports on top of 6.19.x for Panther Lake.
  # If linuxPackages_latest moves beyond 6.19.x, re-test audio before removing
  # or reworking the kernel patch series above.

  # Lid switch behavior
  services.logind.settings.Login = {
    HandleLidSwitch = "suspend";
    HandleLidSwitchExternalPower = "suspend";
    HandleLidSwitchDocked = "ignore";
  };
}
