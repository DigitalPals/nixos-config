# z2-mini-g1a - HP Z2 Mini G1a workstation
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "z2-mini-g1a";

  # AMD Strix Halo integrated GPU.
  hardware.amdgpu.initrd.enable = true;

  # This desktop wakes from suspend via the power button, but once the OS is
  # awake logind's default short-press action is poweroff. Ignore short presses
  # so a display-resume delay does not accidentally shut the machine down.
  services.logind.settings.Login.HandlePowerKey = "ignore";

  # External keyboard/mouse wake goes through the Apple Studio Display USB hub.
  # Keep the full USB chain wake-enabled so an idle suspend can resume cleanly.
  systemd.services.z2-mini-g1a-usb-wakeup = {
    description = "Enable USB wake for z2-mini-g1a external inputs";
    wantedBy = [ "multi-user.target" ];
    after = [ "systemd-udev-settle.service" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      for path in /sys/bus/usb/devices/*/power/wakeup; do
        [ -f "$path" ] && echo enabled > "$path" 2>/dev/null || true
      done

      for path in \
        /sys/bus/pci/devices/0000:00:08.3/power/wakeup \
        /sys/bus/pci/devices/0000:c8:00.4/power/wakeup \
        /sys/bus/pci/devices/0000:c8:00.5/power/wakeup \
        /sys/bus/pci/devices/0000:c8:00.6/power/wakeup
      do
        [ -f "$path" ] && echo enabled > "$path" 2>/dev/null || true
      done
    '';
  };

  # Early boot kernel modules:
  # - amdgpu: early KMS for Plymouth and console
  # - HID modules: keyboard support for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"
    "hid-generic"
    "usbhid"
  ];
}
