# Intel GPU configuration (Panther Lake / Xe3 architecture)
# Used in: xps (Dell XPS 14 DA14260, Intel Core Ultra X7 358H)
#
# Panther Lake uses the xe kernel module, not i915.
{ config, pkgs, lib, ... }:

{
  # Intel Xe GPU module for early KMS (Plymouth support)
  boot.initrd.kernelModules = [
    "xe"  # Intel Xe3 discrete/integrated GPU (Panther Lake+)
  ];

  # Intel-specific VA-API for hardware video acceleration
  # Panther Lake/Xe3 uses the iHD backend exclusively; the old i965 driver
  # targets pre-Broadwell hardware and is not needed here.
  hardware.graphics = {
    extraPackages = with pkgs; [
      intel-media-driver    # iHD driver for Broadwell+ / Xe (VA-API)
      libvpl                # Intel oneVPL dispatcher for Quick Sync video encode/decode
      vpl-gpu-rt            # Intel GPU runtime used by modern media encoders
      intel-compute-runtime # OpenCL support
    ];
    extraPackages32 = with pkgs.pkgsi686Linux; [
      intel-media-driver
    ];
  };

  # Environment variables for Intel Wayland
  environment.sessionVariables = {
    LIBVA_DRIVER_NAME = "iHD";  # Use Intel Media Driver for VA-API
    NIXOS_OZONE_WL = "1";       # Electron apps use Wayland
    # Wayland session variables (also in amd.nix; xps doesn't import amd.nix)
    QT_QPA_PLATFORM = "wayland";
    QT_WAYLAND_DISABLE_WINDOWDECORATION = "1";
  };
}
