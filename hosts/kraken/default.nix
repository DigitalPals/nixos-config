# Beast - Desktop with NVIDIA RTX 5090
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
  ];

  networking.hostName = "kraken";

  # Early boot kernel modules (order matters for proper initialization)
  # - NVIDIA modules: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  # Using mkForce to override any defaults from imported modules
  # Note: simpledrm is kernel builtin, provides fallback framebuffer
  boot.initrd.kernelModules = lib.mkForce [
    "nvidia"           # GPU: main NVIDIA driver
    "nvidia_modeset"   # GPU: kernel modesetting for early KMS
    "nvidia_uvm"       # GPU: unified virtual memory (CUDA)
    "nvidia_drm"       # GPU: DRM for Wayland compositor
    "hid-generic"      # Input: generic HID driver for keyboards
    "usbhid"           # Input: USB HID for external keyboards
  ];
}
