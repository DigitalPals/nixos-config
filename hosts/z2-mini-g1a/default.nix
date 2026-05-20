# z2-mini-g1a - HP Z2 Mini G1a workstation
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "z2-mini-g1a";

  # AMD Strix Halo integrated GPU.
  hardware.amdgpu.initrd.enable = true;

  # This desktop wakes from suspend via the power button, but once the OS is
  # awake logind's default short-press action is poweroff. Ignore short presses
  # so a display-resume delay does not accidentally shut the machine down.
  services.logind.settings.Login.HandlePowerKey = "ignore";

  # Early boot kernel modules:
  # - amdgpu: early KMS for Plymouth and console
  # - HID modules: keyboard support for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"
    "hid-generic"
    "usbhid"
  ];
}
