# Disko configuration for z2-mini-g1a (HP Z2 Mini G1a)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme0n1";
}
