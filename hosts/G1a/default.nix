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

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';
}
