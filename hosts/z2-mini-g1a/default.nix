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

  # === Boot display path: simpledrm owns the initrd/LUKS phase ===
  #
  # Do NOT "align" this host with the G1a laptop. G1a has an internal eDP panel
  # wired straight to the APU. The Z2 Mini has NO internal panel -- its only
  # display is an Apple Studio Display on a USB4/Thunderbolt DisplayPort tunnel
  # that the UEFI firmware sets up and lights before Linux boots.
  #
  # The reliable initrd display is simpledrm: it just scans out the framebuffer
  # the firmware already lit on the Studio Display, and Plymouth draws the LUKS
  # prompt on it. For that prompt to STAY visible, two drivers must be kept out
  # of the initrd -- each one disturbs the firmware framebuffer or its DP tunnel:
  #
  #   * amdgpu      -- loading it evicts simpledrm from fb0 and starts a full
  #                    modeset, but the Thunderbolt DP tunnel is not ready that
  #                    early ("Cannot find any crtc or sizes"); the screen blanks.
  #   * thunderbolt -- its host_reset (module param, default true) resets the
  #                    USB4 host router on probe, tearing down the firmware DP
  #                    tunnel. The Studio Display drops off the bus mid-boot
  #                    ("thunderbolt 0-2: device disconnected") and does not
  #                    return without a physical re-plug.
  #
  # amdgpu is kept out via hardware.amdgpu.initrd.enable below; thunderbolt is
  # kept out of boot.initrd.availableKernelModules in hardware-configuration.nix.
  # Both load normally in stage 2 after unlock, where amdgpu re-lights the
  # display. The initrd needs neither: the root disk is internal NVMe and the
  # keyboard is on a native AMD xHCI -- nothing on the boot path is behind
  # Thunderbolt.
  hardware.amdgpu.initrd.enable = false;

  # AMD display mitigations for the Strix Halo APU driving the Studio Display
  # over the USB4/Thunderbolt DP tunnel. dcdebugmask=0x410 disables PSR/Panel
  # Replay; sg_display=0 disables scatter/gather display for the external panel.
  boot.kernelParams = lib.mkAfter [
    "amdgpu.dcdebugmask=0x410"
    "amdgpu.sg_display=0"
  ];

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

  # The initrd only needs keyboard support for the LUKS passphrase. amdgpu is
  # deliberately absent here -- see the hardware.amdgpu.initrd.enable note
  # above. mkForce overrides the GPU-first default from limine-plymouth.nix.
  boot.initrd.kernelModules = lib.mkForce [
    "hid-generic"
    "usbhid"
  ];
}
