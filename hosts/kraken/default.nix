# Beast - Desktop with NVIDIA RTX 5090
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
  ];

  networking.hostName = "kraken";

  # NVIDIA early KMS for Plymouth
  # Override shared config to ensure all required modules are loaded
  # Note: simpledrm is builtin to the kernel, no need to specify it
  boot.initrd.kernelModules = lib.mkForce [
    "nvidia"
    "nvidia_modeset"
    "nvidia_uvm"
    "nvidia_drm"
    "hid-generic"
    "usbhid"
  ];
}
