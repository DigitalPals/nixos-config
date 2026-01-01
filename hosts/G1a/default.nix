# G1a configuration - HP ZBook Ultra G1a (Strix Halo)
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
  ];

  networking.hostName = "G1a";

  # === AMD Strix Halo (RDNA 3.5) GPU Configuration ===
  # Enable official amdgpu initrd support for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # Battery/power management (disable power-profiles-daemon, use TLP instead)
  services.power-profiles-daemon.enable = false;
  services.tlp = {
    enable = true;
    settings = {
      # CPU behavior
      CPU_SCALING_GOVERNOR_ON_AC = "schedutil";
      CPU_SCALING_GOVERNOR_ON_BAT = "schedutil";
      CPU_ENERGY_PERF_POLICY_ON_AC = "performance";
      CPU_ENERGY_PERF_POLICY_ON_BAT = "balance_power";
      CPU_BOOST_ON_AC = 1;
      CPU_BOOST_ON_BAT = 0;

      # Platform profiles
      PLATFORM_PROFILE_ON_AC = "performance";
      PLATFORM_PROFILE_ON_BAT = "balanced";

      # Runtime power management
      RUNTIME_PM_ON_AC = "auto";
      RUNTIME_PM_ON_BAT = "auto";

      # PCIe power saving
      PCIE_ASPM_ON_AC = "default";
      PCIE_ASPM_ON_BAT = "powersupersave";

      # Battery care
      START_CHARGE_THRESH_BAT0 = 20;
      STOP_CHARGE_THRESH_BAT0 = 80;

      # USB
      USB_AUTOSUSPEND = 1;
      USB_EXCLUDE_AUDIO = 1;
      USB_EXCLUDE_BTUSB = 1;
      USB_EXCLUDE_PHONE = 1;

      # Wi-Fi
      WIFI_PWR_ON_AC = "off";
      WIFI_PWR_ON_BAT = "on";

      # Audio (s2idle-friendly)
      SOUND_POWER_SAVE_ON_AC = 0;
      SOUND_POWER_SAVE_ON_BAT = 1;
      SOUND_POWER_SAVE_CONTROLLER = "Y";
    };
  };

  # Override shared config: set GPU + HID modules for early boot
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"       # AMD GPU for early KMS/Plymouth
    "hid-generic"  # Generic HID for keyboard
    "usbhid"       # USB HID for keyboard
  ];

  # AMD GPU Configuration
  # Note: We do NOT add libva-mesa-driver or amdvlk to extraPackages because:
  # - libva-mesa-driver: VA-API is already included in Mesa by default
  # - amdvlk: Being discontinued, and Mesa RADV is faster and more stable
  # Mesa RADV (Vulkan) and radeonsi (VA-API) are automatically available via hardware.graphics.enable
  #
  # If you need explicit VA-API driver selection, set environment variable:
  # environment.sessionVariables.LIBVA_DRIVER_NAME = "radeonsi";

  # AMD GPU power management and display initialization
  boot.kernelParams = [
    "amdgpu.ppfeaturemask=0xffffffff"
    "amdgpu.dcdebugmask=0x10"  # Helps with display init on new AMD APUs
  ];

  # MediaTek WiFi fix for suspend/resume
  boot.extraModprobeConfig = ''
    options mt7925e disable_aspm=1
  '';

  # AMD Wayland environment variables (equivalent to NVIDIA config)
  environment.sessionVariables = {
    # Force Qt to use native Wayland (improves Quickshell/QML performance)
    QT_QPA_PLATFORM = "wayland";
    # Help Electron/Chromium apps use Wayland
    NIXOS_OZONE_WL = "1";
    # Disable Qt window decorations (Hyprland handles them)
    QT_WAYLAND_DISABLE_WINDOWDECORATION = "1";
  };

  # LUKS configuration is handled by disko (modules/disko/G1a.nix)
  # Disko sets allowDiscards and bypassWorkqueues automatically
}
