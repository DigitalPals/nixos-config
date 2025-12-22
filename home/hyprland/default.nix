# Hyprland window manager configuration
# Generates modular config for Noctalia environment
{ config, pkgs, lib, hostname, ... }:

let
  # Import config generators
  monitorsConfig = import ./monitors.nix { inherit hostname; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix {};
  bindingsConfig = import ./bindings.nix {};
  autostartConfig = import ./autostart.nix {};

in {
  imports = [
    ./hypridle.nix
  ];

  wayland.windowManager.hyprland = {
    enable = true;
    settings = {};
    # Source modular config files from ~/.config/hypr/
    extraConfig = ''
      # Modular Hyprland configuration (Noctalia environment)
      # Each file handles one concern for easier maintenance
      source = ~/.config/hypr/monitors.conf
      source = ~/.config/hypr/input.conf
      source = ~/.config/hypr/bindings.conf
      source = ~/.config/hypr/looknfeel.conf
      source = ~/.config/hypr/autostart.conf
    '';
  };

  # Modular config files in ~/.config/hypr/
  xdg.configFile."hypr/monitors.conf".text = monitorsConfig;
  xdg.configFile."hypr/input.conf".text = inputConfig;
  xdg.configFile."hypr/bindings.conf".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.conf".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.conf".text = autostartConfig;
}
