# ASUS ProArt P16 OLED H7606WX-SE003X
# AMD Ryzen AI 9 HX 370 (Strix Point) + NVIDIA RTX 5090
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
  ];

  networking.hostName = "proart";

  # === Dual GPU Configuration ===
  # AMD Radeon 890M integrated: early KMS, Plymouth
  # NVIDIA RTX 5090: discrete rendering

  # Enable AMD integrated GPU for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # Dual GPU module loading: amdgpu first (integrated), nvidia second (discrete)
  # Override shared config to ensure proper module ordering
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"           # AMD integrated GPU (early KMS, Plymouth)
    "nvidia"           # NVIDIA discrete GPU
    "nvidia_modeset"
    "nvidia_uvm"
    "nvidia_drm"
    "hid-generic"      # Generic HID for keyboard
    "usbhid"           # USB HID for keyboard
  ];

  # AMD APU kernel parameters for power management and display init
  boot.kernelParams = [
    "amdgpu.ppfeaturemask=0xffffffff"
    "amdgpu.dcdebugmask=0x10"  # Helps with display init on new AMD APUs
    "pcie_aspm=force"  # Give Linux ASPM control so mt7925e driver can disable it
  ];

  # MediaTek MT7925 WiFi 7 configuration
  # 1. Load driver explicitly - udev auto-loading can be unreliable
  # 2. Disable ASPM in driver for stable suspend/resume
  # 3. Disable CLC to prevent random disconnects (known bug, fixed in kernel 6.19)
  boot.kernelModules = [ "mt7925e" ];
  boot.extraModprobeConfig = ''
    options mt7925e disable_aspm=1
    options mt7925-common disable_clc=1
  '';

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';
}
