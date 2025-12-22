# NVIDIA GPU configuration
{ config, pkgs, lib, ... }:

{
  # NVIDIA driver
  services.xserver.videoDrivers = [ "nvidia" ];

  hardware.nvidia = {
    open = true;                    # Use open-source kernel modules
    modesetting.enable = true;      # Required for Wayland
    nvidiaSettings = true;          # Enable nvidia-settings GUI
    package = config.boot.kernelPackages.nvidiaPackages.stable;

    # Power management for better efficiency during idle
    powerManagement.enable = true;
  };

  # Load NVIDIA modules in initrd for high-resolution boot display
  boot.initrd.kernelModules = [
    "nvidia"
    "nvidia_modeset"
    "nvidia_uvm"
    "nvidia_drm"
  ];

  # Kernel parameters for NVIDIA DRM
  boot.kernelParams = [
    "nvidia-drm.modeset=1"
    "nvidia-drm.fbdev=1"
  ];

  # Environment variables for NVIDIA Wayland
  environment.sessionVariables = {
    GBM_BACKEND = "nvidia-drm";
    __GLX_VENDOR_LIBRARY_NAME = "nvidia";
    NIXOS_OZONE_WL = "1";
  };
}
