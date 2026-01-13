# proart-p16 - Laptop with Hybrid NVIDIA + AMD GPU
{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
  ];

  networking.hostName = "proart-p16";

  # Hybrid NVIDIA + AMD GPU configuration (PRIME offload)
  # Verify bus IDs with: lspci -nn | grep -E "VGA|3D|Display"
  hardware.amdgpu.initrd.enable = true;
  hardware.nvidia.prime = {
    offload.enable = true;
    offload.enableOffloadCmd = true;
    amdgpuBusId = "PCI:0:0:0"; # TODO: update with the AMD iGPU bus ID
    nvidiaBusId = "PCI:0:0:0"; # TODO: update with the NVIDIA dGPU bus ID
  };

  boot.kernelParams = [
    "amdgpu.ppfeaturemask=0xffffffff"
  ];

  # Early KMS for Plymouth boot splash
  boot.initrd.kernelModules = lib.mkForce [
    "nvidia"
    "nvidia_modeset"
    "nvidia_uvm"
    "nvidia_drm"
    "amdgpu"
    "hid-generic"
    "usbhid"
  ];
}
