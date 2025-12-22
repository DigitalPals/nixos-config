# Hardware configuration for kraken (desktop with NVIDIA RTX 5090)
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

  # Filesystem declarations removed - handled by disko (modules/disko/kraken.nix)
  # Swap removed - using zram only (modules/common.nix)

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
  hardware.cpu.amd.updateMicrocode = lib.mkDefault config.hardware.enableRedistributableFirmware;
}
