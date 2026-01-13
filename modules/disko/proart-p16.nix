# Disko configuration for proart-p16 (ASUS ProArt P16)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme0n1";
}
