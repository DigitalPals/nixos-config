# Caelestia Desktop Shell configuration
{ config, pkgs, lib, ... }:

{
  # Caelestia Desktop Shell
  # The module is loaded via conditional import in home/home.nix
  programs.caelestia = {
    enable = true;

    # Enable CLI for full functionality (required for wallpaper, theming, etc.)
    cli.enable = true;

    # Disable automatic systemd service - we control startup via Hyprland autostart
    systemd.enable = false;
  };

  # Hyprlock for screen locking (used by hypridle and keybindings)
  home.packages = [ pkgs.hyprlock ];
}
