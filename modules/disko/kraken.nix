# Disko configuration for kraken (desktop)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/nvme2n1";
}
