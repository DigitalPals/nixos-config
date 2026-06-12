# Disko configuration for thebeast (Threadripper workstation)
{ ... }:

{
  imports = [ ./default.nix ];

  # Forge writes the selected install disk into hosts/thebeast/local.nix.
  # Keep this as an evaluation default for fresh clones and direct flake checks.
  disko.devices.disk.main.device = "/dev/nvme0n1";
}
