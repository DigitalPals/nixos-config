# 1Password SSH Agent Integration
#
# This module configures SSH to use 1Password's SSH agent, making SSH keys
# stored in 1Password available after a single unlock.
#
# === MANUAL ONE-TIME SETUP REQUIRED ===
#
# After rebuilding, open 1Password GUI and configure:
#
# 1. Settings -> Developer -> Enable "Integrate with 1Password CLI"
# 2. Settings -> Developer -> Enable "Use the SSH agent"
# 3. Add your SSH key(s) to 1Password (or import existing keys)
#
# The SSH agent socket will be available at ~/.1password/agent.sock
#
{ config, pkgs, lib, ... }:

{
  # Point SSH_AUTH_SOCK to 1Password's agent socket
  home.sessionVariables = {
    SSH_AUTH_SOCK = "$HOME/.1password/agent.sock";
  };

  # SSH client configuration
  programs.ssh = {
    enable = true;
    extraConfig = ''
      # Use 1Password SSH agent for all hosts
      Host *
        IdentityAgent ~/.1password/agent.sock
    '';
  };

  # Fish shell integration - ensure SSH_AUTH_SOCK is set in interactive shells
  programs.fish.interactiveShellInit = lib.mkAfter ''
    # Point SSH to 1Password agent if socket exists
    if test -S ~/.1password/agent.sock
      set -gx SSH_AUTH_SOCK ~/.1password/agent.sock
    end
  '';
}
