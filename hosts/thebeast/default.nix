# thebeast - Threadripper workstation with AMD Radeon RX 7700 XT / 7800 XT
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/virtualisation/qemu.nix
  ];

  networking.hostName = "thebeast";

  # Enable official amdgpu initrd support for early KMS and Plymouth.
  hardware.amdgpu.initrd.enable = true;

  # Early boot kernel modules:
  # - amdgpu: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"
    "hid-generic"
    "usbhid"
  ];

  # Realtek RTL8922AE Wi-Fi 7.
  boot.kernelModules = [ "rtw89_8922ae" ];

  networking.wireless.iwd = {
    enable = true;
    settings = {
      General = {
        EnableNetworkConfiguration = false;
      };
      Settings = {
        AutoConnect = true;
      };
    };
  };

  networking.networkmanager.wifi.backend = "iwd";
  networking.networkmanager.wifi.scanRandMacAddress = false;
}
