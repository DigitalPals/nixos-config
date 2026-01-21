# ASUS ProArt P16 OLED H7606WX-SE003X
# AMD Ryzen AI 9 HX 370 (Strix Point) + NVIDIA RTX 5090
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "proart";

  # === Dual GPU Configuration ===
  # AMD Radeon 890M integrated: early KMS, Plymouth
  # NVIDIA RTX 5090: discrete rendering

  # Enable AMD integrated GPU for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # === PRIME Hybrid Graphics ===
  # AMD iGPU renders by default (better battery), NVIDIA powers down when idle
  # Use 'nvidia-offload <app>' or 'prime-run <app>' to run on NVIDIA
  hardware.nvidia.prime = {
    offload = {
      enable = true;
      enableOffloadCmd = true;  # Provides nvidia-offload command
    };
    # PCI bus IDs (lspci shows 64:00.0 and 65:00.0, convert hex to decimal)
    nvidiaBusId = "PCI:100:0:0";  # 0x64 = 100
    amdgpuBusId = "PCI:101:0:0";  # 0x65 = 101
  };

  # Allow NVIDIA dGPU to enter deeper runtime power states when idle
  hardware.nvidia.powerManagement.finegrained = true;

  # Override the forced NVIDIA env vars from nvidia.nix - let apps use iGPU by default
  environment.sessionVariables = {
    GBM_BACKEND = lib.mkForce "";
    __GLX_VENDOR_LIBRARY_NAME = lib.mkForce "";
  };

  # Early boot kernel modules (order matters for hybrid GPU systems)
  # - amdgpu FIRST: integrated GPU handles early KMS/Plymouth (lower power)
  # - nvidia modules SECOND: discrete GPU initializes but stays idle until needed
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  # Using mkForce to override any defaults from imported modules
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"           # GPU: integrated AMD for early KMS/Plymouth
    "nvidia"           # GPU: discrete NVIDIA (loads after amdgpu)
    "nvidia_modeset"   # GPU: NVIDIA kernel modesetting
    "nvidia_uvm"       # GPU: NVIDIA unified virtual memory
    "nvidia_drm"       # GPU: NVIDIA DRM for Wayland
    "hid-generic"      # Input: generic HID driver for keyboards
    "usbhid"           # Input: USB HID for external keyboards
  ];

  # === ASUS Fan/Thermal Control ===
  # asusctl provides fan curve control and platform profile management
  services.asusd = {
    enable = true;
    enableUserService = true;  # Enables user-level notifications and control
    # Default to Quiet profile on both AC and battery for silent operation
    asusdConfig.text = ''
      (
        charge_control_end_threshold: 100,
        disable_nvidia_powerd_on_battery: true,
        ac_command: "",
        bat_command: "",
        platform_profile_linked_epp: true,
        platform_profile_on_battery: Quiet,
        change_platform_profile_on_battery: true,
        platform_profile_on_ac: Quiet,
        change_platform_profile_on_ac: true,
        profile_quiet_epp: Power,
        profile_balanced_epp: BalancePower,
        profile_custom_epp: Performance,
        profile_performance_epp: Performance,
        ac_profile_tunings: {},
        dc_profile_tunings: {},
        armoury_settings: {},
      )
    '';
  };

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
    # Enable PCI runtime PM for NVIDIA dGPU (RTD3) when unused
    ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x030000", TEST=="power/control", ATTR{power/control}="auto"
    ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x030200", TEST=="power/control", ATTR{power/control}="auto"
  '';

  # === Fix s2idle suspend wake ===
  # GPIO 16 causes immediate wake from s2idle - ignore it via kernel parameter.
  # See: https://wiki.archlinux.org/title/Power_management/Wakeup_triggers
  boot.kernelParams = lib.mkAfter [
    "gpiolib_acpi.ignore_interrupt=AMDI0030:00@16"
  ];

  systemd.services.disable-wakeup-sources = {
    description = "Disable wakeup sources for suspend";
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      # Disable ACPI wakeup sources (keep only power button PWRB)
      for dev in XHC0 XHC1 XHC3 XHC4 GPP0 GPP3 GPP4 GPP9 NHI0; do
        if grep -q "^$dev.*enabled" /proc/acpi/wakeup; then
          echo "$dev" > /proc/acpi/wakeup
        fi
      done

      # Disable sysfs wakeup sources
      for path in \
        /sys/bus/thunderbolt/devices/*/power/wakeup \
        /sys/devices/pnp0/00:00/power/wakeup \
        /sys/devices/LNXSYSTM:00/LNXSYBUS:00/PNP0C0A:00/power/wakeup \
        /sys/devices/LNXSYSTM:00/LNXSYBUS:00/PNP0C0D:00/power/wakeup \
        /sys/devices/LNXSYSTM:00/LNXSYBUS:00/USBC000:00/*/power/wakeup \
        /sys/devices/platform/AMDI0010:*/i2c-*/i2c-*/power/wakeup \
        /sys/devices/platform/ACPI0003:00/power_supply/*/power/wakeup \
        /sys/devices/platform/USBC000:00/power_supply/*/power/wakeup
      do
        for f in $path; do
          [ -f "$f" ] && echo disabled > "$f" 2>/dev/null || true
        done
      done
    '';
  };

  # === Hibernate Support ===
  # Using LVM swap partition inside LUKS for reliable hibernate
  # Disko config: LUKS → LVM → 66GB swap LV + btrfs root LV
  boot.resumeDevice = "/dev/vg/swap";
  zramSwap.enable = lib.mkForce false;  # Disable zram, using real swap for hibernate

  # Lid close behavior (suspend should work with GPIO 16 fix)
  services.logind.settings.Login = {
    HandleLidSwitch = "suspend";
    HandleLidSwitchExternalPower = "suspend";
    HandleLidSwitchDocked = "ignore";  # Don't suspend when docked with external monitor
  };
}
