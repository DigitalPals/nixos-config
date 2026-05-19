# SSH defaults
#
# SSH keys live in 1Password and are exposed through the 1Password SSH agent.
# Do not point OpenSSH at a local private key file; fresh installs may not have
# ~/.ssh/id_ed25519, and forcing it breaks Git operations such as app-restore.
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
  IdentityAgent ~/.1password/agent.sock
  StrictHostKeyChecking accept-new
  HashKnownHosts yes
EOF
    chmod 600 "$HOME/.ssh/config"
  '';
}
