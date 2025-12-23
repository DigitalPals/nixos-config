# Noctalia Desktop Shell configuration
{ config, pkgs, lib, inputs, hostname, ... }:

let
  # Load base settings from JSON
  baseSettings = builtins.fromJSON (builtins.readFile ./settings.json);

  # Filter out Battery widget for hosts without a battery (desktop PCs)
  # Use hasPrefix to handle hostname variants (e.g., kraken-illogical)
  hostsWithoutBattery = [ "kraken" ];
  hasBattery = !builtins.any (host: lib.hasPrefix host hostname) hostsWithoutBattery;

  # Generate host-specific settings
  settings = baseSettings // {
    bar = baseSettings.bar // {
      widgets = baseSettings.bar.widgets // {
        right = builtins.filter
          (widget: hasBattery || widget.id != "Battery")
          baseSettings.bar.widgets.right;
      };
    };
  };

  settingsJson = pkgs.writeText "noctalia-settings.json" (builtins.toJSON settings);
in
{
  # Noctalia Desktop Shell
  # The module is loaded via home-manager.sharedModules in flake.nix
  programs.noctalia-shell = {
    enable = true;

    # Disable automatic systemd service - we control startup via Hyprland autostart
    systemd.enable = false;
  };

  # Noctalia configuration files
  xdg.configFile = {
    "noctalia/settings.json".source = settingsJson;
    "noctalia/gui-settings.json".source = ./gui-settings.json;
    "noctalia/colors.json".source = ./colors.json;
    "noctalia/plugins.json".source = ./plugins.json;
  };

  # Noctalia-specific packages
  home.packages = with pkgs; [
    quickshell  # For Noctalia IPC commands
  ];
}
