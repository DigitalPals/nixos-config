# Disko configuration for proart (ASUS ProArt P16 OLED)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme0n1";
}
