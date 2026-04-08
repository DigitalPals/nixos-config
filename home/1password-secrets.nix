# SSH defaults
#
# SSH uses the local key file exported from 1Password. This avoids waking up
# the desktop agent for CLI tools and works in sessions where SSH_AUTH_SOCK
# is not set (Claude Code, cron, etc.).
#
# To refresh the local key from 1Password:
#   op item get "SSH Key (Kraken)" --fields label=private_key --reveal \
#     | install -m 600 /dev/stdin ~/.ssh/id_ed25519
#   op item get "SSH Key (Kraken)" --fields label=public_key \
#     > ~/.ssh/id_ed25519.pub
#
{ config, pkgs, lib, ... }:

{
  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;

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
