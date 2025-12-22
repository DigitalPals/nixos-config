# Disko configuration for G1a (HP ZBook Ultra G1a)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme0n1";
}
