# Dell XPS 14 DA14260 (Intel Panther Lake)
# CPU: Intel Core Ultra X7 358H (16 cores, up to 4.8 GHz)
# GPU: Intel Arc (Xe3 integrated)
# Display: 14.0" 2.8K OLED touch, 20-120Hz
# RAM: 32GB LPDDR5x 9600 MT/s
# WiFi: Intel Wi-Fi 7 BE211
{ pkgs, pkgsMaster, lib, inputs, ... }:
let
  xpsHapticTouchpad = pkgs.writeTextFile {
    name = "xps-haptic-touchpad";
    executable = true;
    destination = "/bin/xps-haptic-touchpad";
    text = ''
      #!${pkgs.python3}/bin/python3

      """Haptic feedback daemon for the Dell XPS Synaptics touchpad.

      The device initiates haptic feedback itself, but needs its HID haptic
      settings restored after enumeration or device reset.
      """

      import fcntl
      import glob
      import os
      import struct
      import sys

      VENDOR = "06CB"
      BUTTON_SWITCH_REPORT = 0x06
      HAPTIC_INTENSITY_REPORT = 0x37
      # Bit 0 = surface switch, bit 1 = button switch.
      BUTTON_SWITCHES = 0x03
      INTENSITY = 1

      EVENT_FORMAT = "llHHi"
      EVENT_SIZE = struct.calcsize(EVENT_FORMAT)


      def hid_ioctl(length):
          return 0xC0000000 | (length << 16) | (ord("H") << 8) | 0x06


      def read_uevent(path):
          values = {}
          try:
              with open(path, encoding="utf-8") as handle:
                  for line in handle:
                      key, _, value = line.strip().partition("=")
                      if key:
                          values[key] = value
          except OSError:
              pass
          return values


      def find_hidraw(phys):
          fallback = None
          for path in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
              values = read_uevent(os.path.join(path, "device", "uevent"))
              if f"0000{VENDOR}" not in values.get("HID_ID", "").upper():
                  continue

              hidraw = os.path.join("/dev", os.path.basename(path))
              if fallback is None:
                  fallback = hidraw
              if phys and values.get("HID_PHYS") == phys:
                  return hidraw
          return fallback


      def find_touchpad_event():
          for path in sorted(glob.glob("/sys/class/input/event*/device/name")):
              try:
                  with open(path, encoding="utf-8") as handle:
                      name = handle.read().strip().upper()
                  if VENDOR in name and "TOUCHPAD" in name:
                      values = read_uevent(os.path.join(os.path.dirname(path), "uevent"))
                      event = path.split("/")[-3]
                      return os.path.join("/dev/input", event), values.get("PHYS")
              except OSError:
                  continue
          return None, None


      def set_feature(fd, report_id, value):
          report = struct.pack("BB", report_id, value)
          fcntl.ioctl(fd, hid_ioctl(len(report)), report)


      def main():
          event, phys = find_touchpad_event()
          if not event:
              print("No Synaptics touchpad input device found", file=sys.stderr)
              sys.exit(1)

          hidraw = find_hidraw(phys)
          if not hidraw:
              print("No Synaptics haptic hidraw device found", file=sys.stderr)
              sys.exit(1)

          hidraw_fd = os.open(hidraw, os.O_RDWR)
          event_fd = os.open(event, os.O_RDONLY)

          try:
              set_feature(hidraw_fd, BUTTON_SWITCH_REPORT, BUTTON_SWITCHES)
              set_feature(hidraw_fd, HAPTIC_INTENSITY_REPORT, INTENSITY)
              print(
                  f"Haptic touchpad: hidraw={hidraw} input={event} "
                  f"switches={BUTTON_SWITCHES} intensity={INTENSITY}",
                  flush=True,
              )

              # Keep the service tied to the input device. If the touchpad
              # resets after suspend, read() fails and systemd restarts us,
              # which reapplies the haptic feature reports.
              while True:
                  data = os.read(event_fd, EVENT_SIZE)
                  if len(data) < EVENT_SIZE:
                      continue
          except KeyboardInterrupt:
              pass
          finally:
              os.close(event_fd)
              os.close(hidraw_fd)


      if __name__ == "__main__":
          main()
    '';
  };
  xpsHapticTouchpadWait = pkgs.writeShellScript "xps-haptic-touchpad-wait" ''
    set -eu

    for _ in $(${pkgs.coreutils}/bin/seq 1 30); do
      if ${pkgs.gnugrep}/bin/grep -qi 'HID_ID=.*000006CB' /sys/class/hidraw/hidraw*/device/uevent 2>/dev/null \
        && ${pkgs.gnugrep}/bin/grep -qli '06CB.*Touchpad' /sys/class/input/event*/device/name 2>/dev/null; then
        exit 0
      fi
      ${pkgs.coreutils}/bin/sleep 0.5
    done

    echo "Timed out waiting for Synaptics haptic touchpad devices" >&2
    exit 1
  '';
in
{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/nix/beast-builder.nix
    # Intel GPU (intel.nix) is imported via extraModules in flake.nix
  ];

  networking.hostName = "xps";

  # Use Linux 7.0.11 from Nixpkgs master while nixos-unstable still carries 7.0.10.
  boot.kernelPackages = lib.mkForce pkgsMaster.linuxPackages_latest;

  # Keep the Limine menu available for rollbacks, but avoid spending multiple
  # seconds there on every normal boot.
  boot.loader.timeout = lib.mkForce 1;

  # WiFi regulatory domain (enables proper 5GHz/6GHz channels and power levels)
  # Use kernel param (not extraModprobeConfig) since cfg80211 is built-in on testing kernels.
  hardware.wirelessRegulatoryDatabase = true;
  boot.kernelParams = [
    "cfg80211.ieee80211_regdom=NL"
    "fred=on"
  ];
  boot.extraModprobeConfig = ''
    # WiFi 7 (EHT/802.11be) causes poor performance with some routers/environments;
    # fall back to WiFi 6/802.11ax until the iwlwifi BE211 driver matures further.
    options iwlwifi disable_11be=Y
  '';

  # Intel CPU configuration
  hardware.cpu.intel.updateMicrocode = true;

  # Intel thermald for thermal management (Dell DPTF integration)
  services.thermald.enable = true;

  environment.etc = {
    "intel_lpmd".source = "${pkgs.intelLpmd}/etc/intel_lpmd";
  };

  services.dbus.packages = [ pkgs.intelLpmd ];

  # Early boot kernel modules (order matters for proper initialization)
  # - GPU module first: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "xe"           # GPU: Intel Xe3 for early KMS/Plymouth
    "hid-generic"  # Input: generic HID driver for keyboards
    "usbhid"       # Input: USB HID for external keyboards
  ];

  # The shared Plymouth module waits for udev-settle to help NVIDIA framebuffer
  # handoff. On this Intel-only XPS, early KMS is provided by xe in initrd and
  # the extra settle ordering only risks delaying the LUKS prompt.
  boot.initrd.systemd.services.plymouth-start = {
    wants = lib.mkForce [ ];
    after = lib.mkForce [ ];
  };

  # === Haptic Touchpad Fix ===
  # Dell XPS uses a Synaptics haptic touchpad (06CB:D01A) whose haptic engine
  # is more reliable when runtime PM stays off, and whose click feedback still
  # needs a userspace nudge on current kernels.

  # Keep the touchpad controller awake so the haptic engine does not lose state.
  services.udev.extraRules = ''
    # Keep the Synaptics touchpad path powered to preserve haptic state.
    ACTION=="add", SUBSYSTEM=="pci", KERNEL=="0000:00:19.0", ATTR{power/control}="on"
    ACTION=="add", SUBSYSTEM=="platform", KERNEL=="i2c_designware.0", ATTR{power/control}="on"
    ACTION=="add|change", SUBSYSTEM=="i2c", KERNEL=="i2c-VEN_06CB:*", ATTR{power/control}="on"
  '';

  # Restore haptic click settings in userspace until the kernel path matures.
  systemd.services.xps-haptic-touchpad = {
    description = "Dell XPS haptic touchpad feedback";
    after = [ "systemd-udev-trigger.service" ];
    wantedBy = [ "multi-user.target" ];
    unitConfig = {
      StartLimitIntervalSec = 120;
      StartLimitBurst = 3;
    };

    serviceConfig = {
      Type = "simple";
      ExecStartPre = "${xpsHapticTouchpadWait}";
      ExecStart = "${xpsHapticTouchpad}/bin/xps-haptic-touchpad";
      Restart = "on-failure";
      RestartSec = 10;
    };
  };

  systemd.services.intel-lpmd = {
    description = "Intel Low Power Mode Daemon";
    after = [ "dbus.service" "upower.service" ];
    wants = [ "dbus.service" "upower.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      ExecStart = "${pkgs.intelLpmd}/bin/intel_lpmd --systemd --dbus-enable";
      RuntimeDirectory = "intel_lpmd";
      Restart = "on-failure";
      Type = "simple";
    };
  };

  # Lid switch behavior
  services.logind.settings.Login = {
    HandleLidSwitch = "suspend";
    HandleLidSwitchExternalPower = "suspend";
    HandleLidSwitchDocked = "ignore";
  };
}
