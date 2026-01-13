# Hardware configuration for proart (ASUS ProArt P16 OLED H7606WX-SE003X)
# Based on nixos-generate-config output, adapted for disko
{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  # Initrd modules for ASUS ProArt P16
  # - nvme: NVMe SSD support (2x 2TB drives)
  # - xhci_pci: USB 3.x via PCI
  # - thunderbolt: Thunderbolt 4 / USB4 support
  # - usbhid: USB HID for keyboard/mouse during boot
  # - uas: USB Attached SCSI (for USB storage devices)
  # - sd_mod: SCSI disk support
  # - sdhci_pci: SD card reader support
  # - btrfs: For encrypted Btrfs root (disko uses Btrfs)
  boot.initrd.availableKernelModules = [ "nvme" "xhci_pci" "thunderbolt" "usbhid" "uas" "sd_mod" "sdhci_pci" "btrfs" ];
  boot.initrd.kernelModules = [ ];

  # AMD Ryzen AI 9 HX 370 uses kvm-amd for virtualization
  boot.kernelModules = [ "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Filesystem declarations removed - handled by disko (modules/disko/proart.nix)
  # Original system used ext4 + swap, disko uses Btrfs + zram
  # Swap removed - using zram only (modules/common.nix)

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
  hardware.cpu.amd.updateMicrocode = lib.mkDefault config.hardware.enableRedistributableFirmware;
}
