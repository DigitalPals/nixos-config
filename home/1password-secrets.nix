# SSH defaults
#
# SSH uses the local key in ~/.ssh directly so Git and other CLI tools do
# not wake up desktop credential agents just to authenticate over SSH.
#
{ config, pkgs, lib, ... }:

{
  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;

    # Default: use the local key file directly from disk.
    matchBlocks."*" = {
      identityFile = "~/.ssh/id_ed25519";
      extraOptions = {
        IdentitiesOnly = "yes";
      };
    };

    extraConfig = ''
      # Security defaults
      StrictHostKeyChecking accept-new
      HashKnownHosts yes
    '';
  };
}
