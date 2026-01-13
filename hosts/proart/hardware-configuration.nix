# Hardware configuration for proart (ASUS ProArt P16 OLED H7606WX-SE003X)
# PLACEHOLDER: Replace this file with the output from nixos-generate-config
# Filesystem declarations are handled by disko
{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  # Initrd modules for ASUS ProArt P16
  # - nvme: NVMe SSD support (2x 2TB drives)
  # - xhci_pci: USB 3.x via PCI
  # - thunderbolt: Thunderbolt 4 / USB4 support
  # - btrfs: For encrypted Btrfs root
  boot.initrd.availableKernelModules = [ "nvme" "xhci_pci" "thunderbolt" "btrfs" ];
  boot.initrd.kernelModules = [ ];

  # AMD Ryzen AI 9 HX 370 uses kvm-amd for virtualization
  boot.kernelModules = [ "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Filesystem declarations removed - handled by disko (modules/disko/proart.nix)
  # Swap removed - using zram only (modules/common.nix)

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
  hardware.cpu.amd.updateMicrocode = lib.mkDefault config.hardware.enableRedistributableFirmware;
}
