# 1Password SSH Agent Integration
#
# SSH tries local key files first (no agent needed). If the local key
# doesn't authenticate, 1Password's SSH agent is used as a fallback.
#
# === MANUAL ONE-TIME SETUP REQUIRED ===
#
# After rebuilding, open 1Password GUI and configure:
#
# 1. Settings -> Developer -> Enable "Integrate with 1Password CLI"
# 2. Settings -> Developer -> Enable "Use the SSH agent"
# 3. Add your SSH key(s) to 1Password (or import existing keys)
#
{ config, pkgs, lib, ... }:

{
  # NOTE: No global SSH_AUTH_SOCK override. This lets SSH read local key
  # files directly from disk without requiring any agent to be unlocked.

  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;

    # Default: try local key file first (read from disk, no agent needed)
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

      # 1Password agent fallback: if the socket exists, make it available
      # SSH tries identityFile first; if that fails, the agent provides keys
      Match host * exec "test -S %d/.1password/agent.sock"
        IdentityAgent ~/.1password/agent.sock
    '';
  };
}
