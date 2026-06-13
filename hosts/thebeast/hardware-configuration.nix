# Hardware configuration for thebeast
# AMD Threadripper 9970X / ASUS Pro WS TRX50-SAGE WIFI A
# Filesystem declarations are handled by disko
{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  boot.initrd.availableKernelModules = [ "xhci_pci" "thunderbolt" "nvme" "ahci" "usbhid" "usb_storage" "uas" "sd_mod" "btrfs" ];
  boot.initrd.kernelModules = [ ];
  boot.kernelModules = [ "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Filesystems are handled by disko and host-local install settings.
  # Hibernate swapfile settings live in hosts/thebeast/local.nix.

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
}
