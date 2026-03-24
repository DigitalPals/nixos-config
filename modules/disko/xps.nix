# Disko configuration for xps (Dell XPS 14)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme0n1";
}
