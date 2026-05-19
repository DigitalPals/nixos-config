# Disko configuration for z2-mini-g1a (HP Z2 Mini G1a)
{ ... }:

{
  imports = [ ./default.nix ];

  disko.devices.disk.main.device = "/dev/disk/by-id/nvme-KXG80ZNV1T02_KIOXIA_45TA20GXKWTK";
}
