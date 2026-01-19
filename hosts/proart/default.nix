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

  # Early boot kernel modules (order matters for hybrid GPU systems)
  # - amdgpu FIRST: integrated GPU handles early KMS/Plymouth (lower power)
  # - nvidia modules SECOND: discrete GPU initializes but stays idle until needed
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  # Using mkForce to override any defaults from imported modules
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"           # GPU: integrated AMD for early KMS/Plymouth
    "nvidia"           # GPU: discrete NVIDIA (loads after amdgpu)
    "nvidia_modeset"   # GPU: NVIDIA kernel modesetting
    "nvidia_uvm"       # GPU: NVIDIA unified virtual memory
    "nvidia_drm"       # GPU: NVIDIA DRM for Wayland
    "hid-generic"      # Input: generic HID driver for keyboards
    "usbhid"           # Input: USB HID for external keyboards
  ];

  # === Mic mute LED fix ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This udev rule allows the user service to control the LED.
  services.udev.extraRules = ''
    # Allow users to control mic mute LED (for WirePlumber sync service)
    SUBSYSTEM=="leds", KERNEL=="hda::micmute", RUN+="${pkgs.coreutils}/bin/chmod 666 %S%p/brightness"
  '';

  # === Hibernate Support ===
  # Using LVM swap partition inside LUKS for reliable hibernate
  # Disko config: LUKS → LVM → 66GB swap LV + btrfs root LV
  boot.resumeDevice = "/dev/vg/swap";
  zramSwap.enable = lib.mkForce false;  # Disable zram, using real swap for hibernate
}
