# Caelestia Desktop Shell configuration
{ config, pkgs, lib, ... }:

{
  # Caelestia Desktop Shell
  # The module is loaded via home-manager.sharedModules in flake.nix
  programs.caelestia = {
    enable = true;

    # Enable CLI for full functionality (required for wallpaper, theming, etc.)
    cli.enable = true;

    # Disable automatic systemd service - we control startup via Hyprland autostart
    systemd.enable = false;
  };
}
