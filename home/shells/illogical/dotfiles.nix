# Fetch and copy Illogical Impulse dotfiles from upstream
{ config, pkgs, lib, dots-hyprland, ... }:

let
  # Source paths from dots-hyprland
  configSource = "${dots-hyprland}/dots/.config";
in
{
  # Copy Quickshell configuration to ~/.config/quickshell/ii/
  xdg.configFile."quickshell/ii" = {
    source = "${configSource}/quickshell/ii";
    recursive = true;
  };

  # Create state directories for Material You theming
  home.activation.setupIllogicalDirs = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    mkdir -p "$HOME/.local/state/quickshell/user/generated/terminal"
  '';
}
