# Hyprland window manager configuration
{ config, pkgs, lib, hostname, osConfig, ... }:

let
  isExternalDisplayLaptop = lib.hasPrefix "G1a" hostname || lib.hasPrefix "xps" hostname;

  # Import config generators
  monitorsConfig = import ./monitors.nix { inherit hostname lib; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix { inherit hostname lib; };
  brightnessControl = import ./brightness.nix { inherit pkgs; };
  portalDevLauncher = pkgs.writeShellScript "portal-dev" ''
    set -euo pipefail

    repo="${config.home.homeDirectory}/Code/portal"
    log_dir="''${XDG_STATE_HOME:-$HOME/.local/state}/portal"
    log_file="$log_dir/dev-launch.log"

    mkdir -p "$log_dir"

    if [ ! -x "$repo/run.sh" ]; then
      ${pkgs.libnotify}/bin/notify-send "Portal dev launcher" "Missing executable: $repo/run.sh" || true
      exit 1
    fi

    cd "$repo"
    exec ./run.sh dev >> "$log_file" 2>&1
  '';
  bindingsConfig = import ./bindings.nix {
    inherit brightnessControl portalDevLauncher;
    homeDirectory = config.home.homeDirectory;
  };
  externalMonitorFunctions = ''
    physical_external_monitor_connected() {
      for status in /sys/class/drm/card*-*/status; do
        [ -e "$status" ] || continue
        case "$status" in
          *-eDP-*/status) continue ;;
        esac

        if [ "$(<"$status")" = "connected" ]; then
          return 0
        fi
      done

      return 1
    }

    apply_monitor_state() {
      if ! physical_external_monitor_connected; then
        ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,preferred,0x0,auto || true
        return 0
      fi

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
          | select((.disabled // false) | not)
      ' > /dev/null 2>&1; then
        if printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '.[] | select(.name == "eDP-1" and ((.disabled // false) | not))' > /dev/null 2>&1; then
          ${pkgs.hyprland}/bin/hyprctl keyword monitor eDP-1,disable || true
        fi

        printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -r '
          map(select(
            ((.model // "") == "Studio XDR" or (.model // "") == "Pro Display XDR")
            and ((.disabled // false) | not)
          ))
          | group_by(.serial // "")
          | map(
            select(length > 1)
            | sort_by((.width * .height), .refreshRate)
            | .[0:-1][]
            | .name
          )
          | .[]
        ' | while read -r output; do
          if [ -n "$output" ]; then
            ${pkgs.hyprland}/bin/hyprctl keyword monitor "$output,disable" || true
          fi
        done
      else
        if ! printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '.[] | select(.name == "eDP-1" and ((.disabled // false) | not) and .x == 0 and .y == 0)' > /dev/null 2>&1; then
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

  autostartConfig = import ./autostart.nix {
    inherit pkgs lib osConfig;
    preShellCommand = if isExternalDisplayLaptop then externalMonitorApply else null;
  };

  # Hyprland Lua entry point.
  hyprlandExtraConfig = ''
    local hyprConfigDir = os.getenv("HOME") .. "/.config/hypr"
    package.path = hyprConfigDir .. "/?.lua;" .. hyprConfigDir .. "/?/init.lua;" .. package.path

    require("monitors")
    require("input")
    require("bindings")
    require("looknfeel")
    require("autostart")
    require("external-monitor-toggle")
  '';

in {
  imports = [
    ./hypridle.nix
  ];

  wayland.windowManager.hyprland = {
    enable = true;
    configType = "lua";
    settings = {};
    extraConfig = hyprlandExtraConfig;
  };

  # External monitor toggle script for laptops that should blank the internal
  # panel when a known external display is attached.
  xdg.configFile."hypr/external-monitor-toggle.lua".text =
    if isExternalDisplayLaptop then ''
      hl.on("hyprland.start", function()
        hl.exec_cmd("${externalMonitorApply}")
      end)

      hl.on("monitor.added", function()
        hl.exec_cmd("sleep 2 && ${externalMonitorApply}")
      end)

      hl.on("monitor.removed", function()
        hl.exec_cmd("sleep 2 && ${externalMonitorApply}")
      end)
    '' else "";

  # Modular config files in ~/.config/hypr/
  xdg.configFile."hypr/monitors.lua".text = monitorsConfig;
  xdg.configFile."hypr/input.lua".text = inputConfig;
  xdg.configFile."hypr/bindings.lua".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.lua".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.lua".text = autostartConfig;

  home.activation.removeGeneratedHyprlandConf = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    conf="$HOME/.config/hypr/hyprland.conf"

    if [ -f "$conf" ] && ${pkgs.gnugrep}/bin/grep -q '^autogenerated = 1$' "$conf"; then
      rm -f "$conf"
    fi
  '';
}
