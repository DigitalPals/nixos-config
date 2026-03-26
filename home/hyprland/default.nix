# Hyprland window manager configuration
{ config, pkgs, lib, hostname, osConfig, ... }:

let
  # Import config generators
  monitorsConfig = import ./monitors.nix { inherit hostname lib; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix { inherit hostname lib; };
  brightnessControl = import ./brightness.nix { inherit pkgs; };
  bindingsConfig = import ./bindings.nix { inherit brightnessControl; };
  autostartConfig = import ./autostart.nix { inherit pkgs lib osConfig; };

  # Script to disable laptop screen when an external display is connected
  # Supports: XREAL One Pro glasses, Apple Studio Display XDR
  externalMonitorToggle = pkgs.writeShellScript "external-monitor-toggle" ''
    check_external() {
      ${pkgs.hyprland}/bin/hyprctl monitors -j | ${pkgs.jq}/bin/jq -e '.[] | select(.model == "XREAL One Pro" or .model == "Studio XDR" or .model == "AORUS FO32U2")' > /dev/null 2>&1
    }

    toggle() {
      if check_external; then
        ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,disable
      else
        ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,preferred,0x540,auto
      fi
    }

    # Check on startup
    toggle

    # Listen for monitor hotplug events
    ${pkgs.socat}/bin/socat -U - "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" | while read -r line; do
      case "$line" in
        monitoradded*|monitorremoved*)
          sleep 0.5
          toggle
          ;;
      esac
    done
  '';

  # Hyprland configuration
  hyprlandExtraConfig = ''
    # Modular Hyprland configuration
    source = ~/.config/hypr/monitors.conf
    source = ~/.config/hypr/input.conf
    source = ~/.config/hypr/bindings.conf
    source = ~/.config/hypr/looknfeel.conf
    source = ~/.config/hypr/autostart.conf
    source = ~/.config/hypr/external-monitor-toggle.conf
    source = ~/.config/hypr/noctalia/noctalia-colors.conf
  '';

in {
  imports = [
    ./hypridle.nix
  ];

  wayland.windowManager.hyprland = {
    enable = true;
    settings = {};
    extraConfig = hyprlandExtraConfig;
  };

  # External monitor toggle script (G1a only)
  xdg.configFile."hypr/external-monitor-toggle.conf".text =
    if lib.hasPrefix "G1a" hostname then ''
      exec-once = ${externalMonitorToggle}
    '' else "";

  # Modular config files in ~/.config/hypr/
  xdg.configFile."hypr/monitors.conf".text = monitorsConfig;
  xdg.configFile."hypr/input.conf".text = inputConfig;
  xdg.configFile."hypr/bindings.conf".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.conf".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.conf".text = autostartConfig;
}
