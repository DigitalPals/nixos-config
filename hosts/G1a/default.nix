# G1a configuration - HP ZBook Ultra G1a (Strix Halo)
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "G1a";

  # === AMD Strix Halo (RDNA 3.5) GPU Configuration ===
  # Enable official amdgpu initrd support for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # Early boot kernel modules (order matters for proper initialization)
  # - GPU modules first: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  # Using mkForce to override any defaults from imported modules
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"       # GPU: early KMS for Plymouth and console
    "hid-generic"  # Input: generic HID driver for keyboards
    "usbhid"       # Input: USB HID for external keyboards
  ];

  # AMD GPU Configuration
  # Note: We do NOT add libva-mesa-driver or amdvlk to extraPackages because:
  # - libva-mesa-driver: VA-API is already included in Mesa by default
  # - amdvlk: Being discontinued, and Mesa RADV is faster and more stable
  # Mesa RADV (Vulkan) and radeonsi (VA-API) are automatically available via hardware.graphics.enable
  #
  # If you need explicit VA-API driver selection, set environment variable:
  # environment.sessionVariables.LIBVA_DRIVER_NAME = "radeonsi";

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
