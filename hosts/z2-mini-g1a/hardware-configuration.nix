# Hardware configuration for z2-mini-g1a (HP Z2 Mini G1a with AMD Strix Halo)
# Filesystem declarations are handled by disko
{ config, lib, pkgs, modulesPath, ... }:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  # "thunderbolt" is intentionally NOT in this list. The thunderbolt driver's
  # host_reset resets the USB4 host router on probe, tearing down the firmware
  # DisplayPort tunnel and blanking the Studio Display mid-boot (right on the
  # Plymouth LUKS prompt). The initrd does not need it: the root disk is
  # internal NVMe and the keyboard is on a native AMD xHCI (xhci_pci), neither
  # behind Thunderbolt. It loads normally in stage 2. Do not let a hardware
  # re-scan add it back. See hosts/z2-mini-g1a/default.nix.
  boot.initrd.availableKernelModules = [ "nvme" "xhci_pci" "ahci" "usbhid" "uas" "sd_mod" "btrfs" ];
  boot.initrd.kernelModules = [ ];
  boot.kernelModules = [ "kvm-amd" ];
  boot.extraModulePackages = [ ];

  # Filesystem declarations removed - handled by disko (modules/disko/z2-mini-g1a.nix)
  # Swap removed - using zram only (modules/common.nix)

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
}
