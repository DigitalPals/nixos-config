# Hyprland window manager configuration
{ config, pkgs, lib, hostname, osConfig, ... }:

let
  isExternalDisplayLaptop = lib.hasPrefix "G1a" hostname || lib.hasPrefix "xps" hostname;

  # Import config generators
  monitorsConfig = import ./monitors.nix { inherit hostname lib; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix { inherit hostname lib; };
  brightnessControl = import ./brightness.nix { inherit pkgs lib hostname; };
  bindingsConfig = import ./bindings.nix {
    inherit brightnessControl;
    homeDirectory = config.home.homeDirectory;
  };
  externalMonitorFunctions = ''
    apply_monitor_state() {
      monitors="$(${pkgs.hyprland}/bin/hyprctl monitors -j 2>/dev/null)" || return 0

      if printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '
        .[]
        | select(
            (.model // "") == "XREAL One Pro"
            or (.model // "") == "Nreal XREAL One Pro"
            or (.model // "") == "Studio XDR"
            or (.model // "") == "Pro Display XDR"
            or (.model // "") == "AORUS FO32U2"
          )
      ' > /dev/null 2>&1; then
        if printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '.[] | select(.name == "eDP-1")' > /dev/null 2>&1; then
          ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,disable || true
        fi
      else
        if ! printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '.[] | select(.name == "eDP-1" and .x == 0 and .y == 0)' > /dev/null 2>&1; then
          ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,preferred,0x0,auto || true
        fi
      fi

      return 0
    }
  '';

  externalMonitorApply = pkgs.writeShellScript "external-monitor-apply" ''
    ${externalMonitorFunctions}
    apply_monitor_state
  '';

  # Script to disable the laptop panel when a known external display is
  # connected. On monitor removal, bring eDP-1 back immediately so clients do
  # not sit through a no-output Wayland interval before the debounce completes.
  externalMonitorToggle = pkgs.writeShellScript "external-monitor-toggle" ''
    ${externalMonitorFunctions}

    if [ -z "''${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
      exit 0
    fi

    # Listen for monitor hotplug events
    ${pkgs.socat}/bin/socat -U - "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" | while read -r line; do
      case "$line" in
        monitorremoved*)
          ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,preferred,0x0,auto || true
          sleep 0.5
          apply_monitor_state
          ;;
        monitoradded*)
          sleep 0.5
          apply_monitor_state
          ;;
      esac
    done
  '';

  autostartConfig = import ./autostart.nix {
    inherit pkgs lib osConfig;
    preShellCommand = if isExternalDisplayLaptop then externalMonitorApply else null;
  };

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

  # External monitor toggle script for laptops that should blank the internal
  # panel when a known external display is attached.
  xdg.configFile."hypr/external-monitor-toggle.conf".text =
    if isExternalDisplayLaptop then ''
      exec-once = ${externalMonitorToggle}
    '' else "";

  # Modular config files in ~/.config/hypr/
  xdg.configFile."hypr/monitors.conf".text = monitorsConfig;
  xdg.configFile."hypr/input.conf".text = inputConfig;
  xdg.configFile."hypr/bindings.conf".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.conf".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.conf".text = autostartConfig;
}
