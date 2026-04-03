# Intel GPU configuration (Panther Lake / Xe3 architecture)
# Used in: xps (Dell XPS 14 DA14260, Intel Core Ultra X7 358H)
#
# Panther Lake uses the xe kernel module, not i915.
# Display fix kernel params sourced from Omarchy Linux:
# https://github.com/basecamp/omarchy/blob/main/install/config/hardware/fix-intel-panther-lake-display.sh
{ config, pkgs, lib, ... }:

{
  # Intel Xe GPU module for early KMS (Plymouth support)
  boot.initrd.kernelModules = [
    "xe"  # Intel Xe3 discrete/integrated GPU (Panther Lake+)
  ];

  # Intel-specific VA-API for hardware video acceleration
  hardware.graphics = {
    extraPackages = with pkgs; [
      intel-media-driver    # iHD driver for Broadwell+ (VA-API)
      intel-vaapi-driver    # i965 driver for older Intel GPUs (VA-API)
      intel-compute-runtime # OpenCL support
    ];
    extraPackages32 = with pkgs.pkgsi686Linux; [
      intel-media-driver
      intel-vaapi-driver
    ];
  };

  # Panther Lake display fix for the Dell XPS OLED panel: newer Omarchy
  # testing narrowed the 10Hz regression down to panel replay alone.
  boot.kernelParams = [
    "xe.enable_panel_replay=0" # Disable Panel Replay
  ];

  # Environment variables for Intel Wayland
  environment.sessionVariables = {
    LIBVA_DRIVER_NAME = "iHD";  # Use Intel Media Driver for VA-API
    NIXOS_OZONE_WL = "1";       # Electron apps use Wayland
    # Wayland session variables (also in amd.nix; xps doesn't import amd.nix)
    QT_QPA_PLATFORM = "wayland";
    QT_WAYLAND_DISABLE_WINDOWDECORATION = "1";
  };
}
