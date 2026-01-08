# AMD-specific hardware configuration
{ lib, ... }:

{
  # Thermal monitoring
  boot.kernelModules = [ "k10temp" ];

  # CPU microcode updates (use mkDefault so host can override)
  hardware.cpu.amd.updateMicrocode = lib.mkDefault true;
}
