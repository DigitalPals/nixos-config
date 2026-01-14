# ASUS ProArt P16 OLED H7606WX-SE003X
# AMD Ryzen AI 9 HX 370 (Strix Point) + NVIDIA RTX 5090
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/hardware/mediatek-wifi.nix
  ];

  networking.hostName = "proart";

  # === Dual GPU Configuration ===
  # AMD Radeon 890M integrated: early KMS, Plymouth
  # NVIDIA RTX 5090: discrete rendering

  # Enable AMD integrated GPU for early KMS and Plymouth
  hardware.amdgpu.initrd.enable = true;

  # === PRIME Hybrid Graphics ===
  # AMD iGPU renders by default (better battery), NVIDIA powers down when idle
  # Use 'nvidia-offload <app>' or 'prime-run <app>' to run on NVIDIA
  hardware.nvidia.prime = {
    offload = {
      enable = true;
      enableOffloadCmd = true;  # Provides nvidia-offload command
    };
    # PCI bus IDs (lspci shows 64:00.0 and 65:00.0, convert hex to decimal)
    nvidiaBusId = "PCI:100:0:0";  # 0x64 = 100
    amdgpuBusId = "PCI:101:0:0";  # 0x65 = 101
  };

  # Override the forced NVIDIA env vars from nvidia.nix - let apps use iGPU by default
  environment.sessionVariables = {
    GBM_BACKEND = lib.mkForce "";
    __GLX_VENDOR_LIBRARY_NAME = lib.mkForce "";
  };

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

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';
}
