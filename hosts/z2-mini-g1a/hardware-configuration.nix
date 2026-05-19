# Hardware configuration for z2-mini-g1a (HP Z2 Mini G1a with AMD Strix Halo)
# Filesystem declarations are handled by disko
{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  boot.initrd.availableKernelModules = [ "nvme" "xhci_pci" "ahci" "thunderbolt" "usbhid" "uas" "sd_mod" "btrfs" ];
  boot.initrd.kernelModules = [ ];
  boot.kernelModules = [ "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Filesystem declarations removed - handled by disko (modules/disko/z2-mini-g1a.nix)
  # Swap removed - using zram only (modules/common.nix)

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
}
