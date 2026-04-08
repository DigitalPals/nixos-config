# SSH defaults
#
# Keys are managed through 1Password's SSH agent. The agent socket is set
# explicitly so SSH works even when SSH_AUTH_SOCK is not exported to the
# current session (e.g. Claude Code terminal, cron jobs).
#
{ config, pkgs, lib, ... }:

{
  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;

    matchBlocks."*" = {
      extraOptions = {
        IdentityAgent = "~/.1password/agent.sock";
      };
    };

    extraConfig = ''
      # Security defaults
      StrictHostKeyChecking accept-new
      HashKnownHosts yes
    '';
  };
}
