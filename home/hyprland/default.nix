# Hyprland window manager configuration
# Generates modular config for multi-shell environment
{ config, pkgs, lib, hostname, osConfig, ... }:

let
  # Get shell from NixOS config (set by specialisations)
  shell = osConfig.desktop.shell;
  # Import config generators (only used for noctalia)
  monitorsConfig = import ./monitors.nix { inherit hostname lib; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix { inherit hostname lib; };
  bindingsConfig = import ./bindings.nix { inherit shell; };
  autostartConfig = import ./autostart.nix { inherit shell; };

  # Shell-specific Hyprland configuration
  # CLEAN APPROACH: Use our ENTIRE Hyprland config for both shells
  # Only difference is autostart (which shell to launch)
  # Illogical's Hyprland configs are incompatible with Hyprland 0.52+
  hyprlandExtraConfig = ''
    # Modular Hyprland configuration
    source = ~/.config/hypr/monitors.conf
    source = ~/.config/hypr/input.conf
    source = ~/.config/hypr/bindings.conf
    source = ~/.config/hypr/looknfeel.conf
    source = ~/.config/hypr/autostart.conf
  '' + lib.optionalString (shell == "noctalia") ''
    source = ~/.config/hypr/noctalia/noctalia-colors.conf
  '';

in {
  imports = [
    (import ./hypridle.nix { inherit shell; })
  ];

  wayland.windowManager.hyprland = {
    enable = true;
    settings = {};
    extraConfig = hyprlandExtraConfig;
  };

  # Modular config files in ~/.config/hypr/
  # Same configs for both shells - only autostart differs
  xdg.configFile."hypr/monitors.conf".text = monitorsConfig;
  xdg.configFile."hypr/input.conf".text = inputConfig;
  xdg.configFile."hypr/bindings.conf".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.conf".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.conf".text = autostartConfig;
}
