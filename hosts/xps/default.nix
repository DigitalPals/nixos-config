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
    "0012-ASoC-SDCA-Rearrange-FDL-file-messages.patch"
    "0013-ASoC-SDCA-Add-regmap-defaults-for-specification-defi.patch"
    "0014-ASoC-SDCA-Limit-values-user-can-write-to-Selected-Mo.patch"
  ];

  mkSdcaPatch = name: {
    inherit name;
    patch = "${hurricanOmarchyEnabling}/sdca-backport-patches/${name}";
  };

  xpsHapticTouchpad = pkgs.writeTextFile {
    name = "xps-haptic-touchpad";
    executable = true;
    destination = "/bin/xps-haptic-touchpad";
    text = ''
      #!${pkgs.python3}/bin/python3

      """Haptic feedback daemon for the Dell XPS Synaptics touchpad.

      The device uses the HID Manual Trigger protocol, so we listen for
      button press events and send a short haptic pulse through hidraw.
      """

      import fcntl
      import glob
      import os
      import struct
      import sys

      VENDOR = "06CB"
      PRODUCT = "D01A"
      REPORT_ID = 0x37
      INTENSITY = 40

      EVENT_FORMAT = "llHHi"
      EVENT_SIZE = struct.calcsize(EVENT_FORMAT)
      EV_KEY = 0x01
      BUTTONS = {272, 273, 274}


      def hid_ioctl(length):
          return 0xC0000000 | (length << 16) | (ord("H") << 8) | 0x06


      def find_hidraw():
          for path in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
              uevent = os.path.join(path, "device", "uevent")
              try:
                  with open(uevent, encoding="utf-8") as handle:
                      content = handle.read().upper()
                  if f"0000{VENDOR}" in content and f"0000{PRODUCT}" in content:
                      return os.path.join("/dev", os.path.basename(path))
              except OSError:
                  continue
          return None


      def find_touchpad_event():
          for path in sorted(glob.glob("/sys/class/input/event*/device/name")):
              try:
                  with open(path, encoding="utf-8") as handle:
                      name = handle.read().strip().upper()
                  if VENDOR in name and PRODUCT in name and "TOUCHPAD" in name:
                      event = path.split("/")[-3]
                      return os.path.join("/dev/input", event)
              except OSError:
                  continue
          return None


      def main():
          hidraw = find_hidraw()
          if not hidraw:
              print("No Synaptics haptic hidraw device found", file=sys.stderr)
              sys.exit(1)

          event = find_touchpad_event()
          if not event:
              print("No Synaptics touchpad input device found", file=sys.stderr)
              sys.exit(1)

          report = struct.pack("BB", REPORT_ID, INTENSITY)
          ioctl_request = hid_ioctl(len(report))

          hidraw_fd = os.open(hidraw, os.O_RDWR)
          event_fd = os.open(event, os.O_RDONLY)

          try:
              while True:
                  data = os.read(event_fd, EVENT_SIZE)
                  if len(data) < EVENT_SIZE:
                      continue
                  _, _, event_type, code, value = struct.unpack(EVENT_FORMAT, data)
                  if event_type == EV_KEY and code in BUTTONS and value == 1:
                      try:
                          fcntl.ioctl(hidraw_fd, ioctl_request, report)
                      except OSError:
                          pass
          except KeyboardInterrupt:
              pass
          finally:
              os.close(event_fd)
              os.close(hidraw_fd)


      if __name__ == "__main__":
          main()
    '';
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
    # Temporary Panther Lake WiFi 7 workaround: BE211 is currently more stable
    # when EHT/802.11be is disabled, falling back to WiFi 6/802.11ax.
    options iwlwifi disable_11be=Y
    options v4l2loopback video_nr=50 card_label="Intel IPU7 Camera" exclusive_caps=1
  '';

  # Intel CPU configuration
  hardware.cpu.intel.updateMicrocode = true;
  # Use the upstream 7.0 release-candidate kernel so Panther Lake audio support
  # comes from mainline instead of our temporary 6.19 backport set.
  boot.kernelPackages = lib.mkForce pkgs.linuxPackages_testing;
  boot.kernelModules = [ "kvm-intel" "coretemp" "v4l2loopback" ];
  boot.extraModulePackages = [ config.boot.kernelPackages.v4l2loopback ];

  # Intel thermald for thermal management (Dell DPTF integration)
  services.thermald.enable = true;

  # Keep the SDCA backports available only for older 6.19-based testing.
  # They are skipped automatically once the selected kernel is 7.0+.
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

  # === Haptic Touchpad Fix ===
  # Dell XPS uses a Synaptics haptic touchpad (06CB:D01A) whose haptic engine
  # is more reliable when runtime PM stays off, and whose click feedback still
  # needs a userspace nudge on current kernels.

  # Keep the touchpad controller awake so the haptic engine does not lose state.
  services.udev.extraRules = ''
    # Keep the Synaptics touchpad path powered to preserve haptic state.
    ACTION=="add", SUBSYSTEM=="pci", KERNEL=="0000:00:19.0", ATTR{power/control}="on"
    ACTION=="add", SUBSYSTEM=="platform", KERNEL=="i2c_designware.0", ATTR{power/control}="on"
    ACTION=="add|change", SUBSYSTEM=="i2c", ATTRS{idVendor}=="06cb", ATTRS{idProduct}=="d01a", ATTR{power/control}="on"

    # Make the virtual camera device accessible to the desktop session
    KERNEL=="video50", GROUP="video", MODE="0660"

    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="*::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';

  # Generate haptic click feedback in userspace until the kernel path matures.
  systemd.services.xps-haptic-touchpad = {
    description = "Dell XPS haptic touchpad feedback";
    after = [ "systemd-udev-settle.service" ];
    wants = [ "systemd-udev-settle.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "simple";
      ExecStart = "${xpsHapticTouchpad}/bin/xps-haptic-touchpad";
      Restart = "on-failure";
      RestartSec = 2;
    };
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
  # Omarchy carried SDCA backports on top of 6.19.x for early Panther Lake
  # support. With the testing kernel on 7.0-rc, audio should come from
  # upstream support instead of this repo's temporary backport set.

  # Lid switch behavior
  services.logind.settings.Login = {
    HandleLidSwitch = "suspend";
    HandleLidSwitchExternalPower = "suspend";
    HandleLidSwitchDocked = "ignore";
  };
}
