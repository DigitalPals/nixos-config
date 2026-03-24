# Dell XPS 14 DA14260 (Intel Panther Lake)
# CPU: Intel Core Ultra X7 358H (16 cores, up to 4.8 GHz)
# GPU: Intel Arc (Xe3 integrated)
# Display: 14.0" 2.8K OLED touch, 20-120Hz
# RAM: 32GB LPDDR5x 9600 MT/s
# WiFi: Intel Wi-Fi 7 BE211
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    # Intel GPU (intel.nix) is imported via extraModules in flake.nix
  ];

  networking.hostName = "xps";

  # WiFi regulatory domain (enables proper 5GHz/6GHz channels and power levels)
  networking.wireless.regulatory.domain = "NL";

  # Intel CPU configuration
  hardware.cpu.intel.updateMicrocode = true;
  boot.kernelModules = [ "kvm-intel" "coretemp" ];

  # Intel thermald for thermal management (Dell DPTF integration)
  services.thermald.enable = true;

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

  # === Audio: SOF/HDA conflict workaround ===
  # On some Panther Lake systems, SOF (Sound Open Firmware) modules conflict
  # with standard HDA drivers, causing audio failure or boot hangs.
  # Kernel 6.19+ likely has upstream fixes. Test audio first — only uncomment
  # if audio doesn't work or causes boot issues.
  #
  # boot.blacklistedKernelModules = [
  #   "snd_sof_pci" "snd_sof_pci_intel_lnl" "snd_sof_pci_intel_mtl"
  #   "snd_sof_intel_hda" "snd_sof"
  #   "snd_soc_hdac_hda" "snd_soc_hdac_hdmi"
  #   "soundwire_intel" "snd_soc_spl_cirrus"
  # ];

  # Lid switch behavior
  services.logind.settings.Login = {
    HandleLidSwitch = "suspend";
    HandleLidSwitchExternalPower = "suspend";
    HandleLidSwitchDocked = "ignore";
  };
}
