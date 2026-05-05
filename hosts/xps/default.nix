# Dell XPS 14 DA14260 (Intel Panther Lake)
# CPU: Intel Core Ultra X7 358H (16 cores, up to 4.8 GHz)
# GPU: Intel Arc (Xe3 integrated)
# Display: 14.0" 2.8K OLED touch, 20-120Hz
# RAM: 32GB LPDDR5x 9600 MT/s
# WiFi: Intel Wi-Fi 7 BE211
{ config, pkgs, lib, ... }:
let
  xpsHapticTouchpad = pkgs.writeTextFile {
    name = "xps-haptic-touchpad";
    executable = true;
    destination = "/bin/xps-haptic-touchpad";
    text = ''
      #!${pkgs.python3}/bin/python3

      """Haptic feedback daemon for the Dell XPS Synaptics touchpad.

      Current kernels expose button press events but do not reliably trigger
      click feedback, so send the Synaptics manual haptic trigger on each click.
      """

      import fcntl
      import glob
      import os
      import select
      import struct
      import sys

      VENDOR = "06CB"
      REPORT_ID = 0x37
      INTENSITY = 100

      EVENT_FORMAT = "llHHi"
      EVENT_SIZE = struct.calcsize(EVENT_FORMAT)
      EV_KEY = 0x01
      BTN_LEFT = 272
      BTN_RIGHT = 273
      BTN_MIDDLE = 274


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


      def send_haptic(fd, ioctl_req, report):
          try:
              fcntl.ioctl(fd, ioctl_req, report)
          except OSError as error:
              print(f"Failed to send haptic report: {error}", file=sys.stderr, flush=True)


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
          report = struct.pack("BB", REPORT_ID, INTENSITY)
          ioctl_req = hid_ioctl(len(report))

          try:
              print(
                  f"Haptic touchpad: hidraw={hidraw} input={event} "
                  f"intensity={INTENSITY}",
                  flush=True,
              )
              send_haptic(hidraw_fd, ioctl_req, report)

              while True:
                  ready, _, _ = select.select([event_fd], [], [], 1.0)
                  if not ready:
                      continue

                  data = os.read(event_fd, EVENT_SIZE)
                  if len(data) < EVENT_SIZE:
                      continue

                  _, _, ev_type, code, value = struct.unpack(EVENT_FORMAT, data)
                  if ev_type == EV_KEY and code in (BTN_LEFT, BTN_RIGHT, BTN_MIDDLE) and value == 1:
                      send_haptic(hidraw_fd, ioctl_req, report)
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
  xpsIpu7CameraInit = pkgs.writeShellScript "xps-ipu7-camera-init" ''
    set -eu

    ${pkgs.kmod}/bin/modprobe intel_cvs 2>/dev/null || true
    ${pkgs.coreutils}/bin/sleep 2
    ${pkgs.kmod}/bin/modprobe ov08x40 2>/dev/null || true
    ${pkgs.kmod}/bin/modprobe v4l2loopback
  '';
  xpsIpu7CameraRelay = pkgs.writeShellScript "xps-ipu7-camera-relay" ''
    set -eu

    for _ in $(${pkgs.coreutils}/bin/seq 1 30); do
      [ -e /dev/video50 ] && break
      ${pkgs.coreutils}/bin/sleep 0.5
    done

    if [ ! -e /dev/video50 ]; then
      echo "Timed out waiting for /dev/video50" >&2
      exit 1
    fi

    export GST_PLUGIN_SYSTEM_PATH_1_0="${lib.makeSearchPath "lib/gstreamer-1.0" [
      pkgs.icamerasrcIpu75xa
      pkgs.gst_all_1.gstreamer
      pkgs.gst_all_1.gst-plugins-base
      pkgs.gst_all_1.gst-plugins-good
      pkgs.gst_all_1.gst-plugins-bad
    ]}"
    export GST_PLUGIN_PATH_1_0="$GST_PLUGIN_SYSTEM_PATH_1_0"

    exec ${pkgs.v4l2-relayd}/bin/v4l2-relayd \
      -i "icamerasrc device-name=ov08x40-uf sharpness=80 ev=-1 saturation=10 ! videoflip method=rotate-180" \
      -o "appsrc name=appsrc caps=video/x-raw,format=NV12,width=1920,height=1080,framerate=30/1 ! videoconvert ! v4l2sink name=v4l2sink device=/dev/video50"
  '';
in
{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    # Intel GPU (intel.nix) is imported via extraModules in flake.nix
  ];

  networking.hostName = "xps";

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
    options v4l2loopback video_nr=50 card_label="Hardware ISP Camera" exclusive_caps=1 max_buffers=16
  '';

  # Intel CPU configuration
  hardware.cpu.intel.updateMicrocode = true;
  boot.kernelModules = [ "v4l2loopback" ];
  # This builds only the small out-of-tree v4l2loopback module for the active
  # kernel. It does not rebuild the kernel itself.
  boot.extraModulePackages = [ config.boot.kernelPackages.v4l2loopback ];

  # Intel thermald for thermal management (Dell DPTF integration)
  services.thermald.enable = true;

  environment.systemPackages = with pkgs; [
    icamerasrcIpu75xa
    ipu7CameraBins
    ipu7CameraHal
    v4l2-relayd
    v4l-utils
  ];

  environment.etc = {
    "camera/ipu75xa".source = "${pkgs.ipu7CameraHal}/etc/camera/ipu75xa";
    "intel_lpmd".source = "${pkgs.intelLpmd}/etc/intel_lpmd";
    "systemd/system-sleep/xps-ipu7-camera".text = ''
      #!${pkgs.bash}/bin/bash

      case "$1" in
        pre)
          ${pkgs.systemd}/bin/systemctl stop xps-ipu7-camera-relay.service 2>/dev/null || true
          ;;
        post)
          ${pkgs.systemd}/bin/systemd-run --no-block --unit=xps-ipu7-camera-resume ${pkgs.bash}/bin/bash -c '
            ${pkgs.systemd}/bin/systemctl restart xps-ipu7-camera-init.service 2>/dev/null || true
            ${pkgs.systemd}/bin/systemctl reset-failed xps-ipu7-camera-relay.service 2>/dev/null || true
            ${pkgs.systemd}/bin/systemctl start xps-ipu7-camera-relay.service 2>/dev/null || true
          '
          ;;
      esac
    '';
    "systemd/system-sleep/xps-ipu7-camera".mode = "0755";
    "wireplumber/wireplumber.conf.d/70-hide-ipu7-v4l2.conf".text = ''
      # Hide raw IPU7 ISP nodes; desktop apps should use the v4l2loopback camera.
      monitor.v4l2.rules = [
        {
          matches = [
            {
              device.bus-path = "pci-0000:00:05.0"
            }
          ]
          actions = {
            update-props = {
              device.disabled = true
            }
          }
        }
      ]
    '';
    "wireplumber/wireplumber.conf.d/71-disable-libcamera.conf".text = ''
      wireplumber.profiles = {
        main = {
          "monitor.libcamera" = disabled
        }
      }
    '';
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
    ACTION=="add|change", SUBSYSTEM=="i2c", KERNEL=="i2c-VEN_06CB:*", ATTR{power/control}="on"

    # Hide raw IPU7 ISYS capture nodes; they expose raw Bayer frames.
    SUBSYSTEM=="video4linux", ATTR{name}=="Intel IPU7 ISYS Capture *", TAG-="uaccess", TAG-="seat", MODE="0600", GROUP="root"
    KERNEL=="ipu7-psys0", MODE="0666", SYMLINK+="ipu-psys0"

    # Make the virtual camera device accessible to the desktop session
    KERNEL=="video50", GROUP="video", MODE="0660"
  '';

  # Generate haptic click feedback in userspace until the kernel path matures.
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

  systemd.services.xps-ipu7-camera-init = {
    description = "Initialize Dell XPS IPU7 camera";
    after = [ "systemd-modules-load.service" ];
    before = [ "graphical.target" "xps-ipu7-camera-relay.service" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${xpsIpu7CameraInit}";
    };
  };

  systemd.services.xps-ipu7-camera-relay = {
    description = "Relay Dell XPS IPU7 camera to v4l2loopback";
    after = [ "xps-ipu7-camera-init.service" "graphical.target" ];
    wants = [ "xps-ipu7-camera-init.service" ];
    wantedBy = [ "graphical.target" ];

    serviceConfig = {
      Type = "simple";
      ExecStart = "${xpsIpu7CameraRelay}";
      Restart = "on-failure";
      RestartSec = 3;
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
