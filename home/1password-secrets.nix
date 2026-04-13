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
    matchBlocks = lib.mkForce {};
    enableDefaultConfig = false;

    extraConfig = "";
  };

  # OpenSSH inside Distrobox rejects ~/.ssh/config when it is a Home Manager
  # symlink because the symlink itself appears as mode 0777. Keep a real file.
  home.activation.sshConfigRegularFile = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    mkdir -p "$HOME/.ssh"
    rm -f "$HOME/.ssh/config"
    cat > "$HOME/.ssh/config" <<'EOF'
Host *
  IdentityFile ~/.ssh/id_ed25519
  IdentitiesOnly yes
  StrictHostKeyChecking accept-new
  HashKnownHosts yes
EOF
    chmod 600 "$HOME/.ssh/config"
  '';
}
