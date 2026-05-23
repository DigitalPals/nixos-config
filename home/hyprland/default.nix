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
    hypr_monitor_enable_edp() {
      ${pkgs.hyprland}/bin/hyprctl eval 'hl.monitor({ output = "eDP-1", disabled = false, mode = "preferred", position = "0x0", scale = "auto" })' || true
    }

    hypr_monitor_disable() {
      output="$1"
      output_lua="$(${pkgs.jq}/bin/jq -Rn --arg output "$output" '$output')"
      ${pkgs.hyprland}/bin/hyprctl eval "hl.monitor({ output = $output_lua, disabled = true })" || true
    }

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
        hypr_monitor_enable_edp
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
          hypr_monitor_disable "eDP-1"
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
            hypr_monitor_disable "$output"
          fi
        done
      else
        if ! printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '.[] | select(.name == "eDP-1" and ((.disabled // false) | not) and .x == 0 and .y == 0)' > /dev/null 2>&1; then
          hypr_monitor_enable_edp
        fi
      fi

      return 0
    }
  '';

  externalMonitorApply = pkgs.writeShellScript "external-monitor-apply" ''
    ${externalMonitorFunctions}
    apply_monitor_state
  '';
  externalMonitorDaemon = pkgs.writeShellScript "external-monitor-daemon" ''
    ${externalMonitorFunctions}

    while true; do
      apply_monitor_state
      sleep 1
    done
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
        hl.exec_cmd("systemctl --user restart hyprland-external-monitor-toggle.service")
      end)
    '' else "";

  systemd.user.services.hyprland-external-monitor-toggle = lib.mkIf isExternalDisplayLaptop {
    Unit = {
      Description = "Hyprland external monitor toggle";
      PartOf = [ "hyprland-session.target" ];
      After = [ "hyprland-session.target" ];
    };

    Service = {
      ExecStart = "${externalMonitorDaemon}";
      Restart = "always";
      RestartSec = 1;
    };
  };

  # Modular config files in ~/.config/hypr/
  xdg.configFile."hypr/monitors.lua".text = monitorsConfig;
  xdg.configFile."hypr/input.lua".text = inputConfig;
  xdg.configFile."hypr/bindings.lua".text = bindingsConfig;
  xdg.configFile."hypr/looknfeel.lua".text = looknfeelConfig;
  xdg.configFile."hypr/autostart.lua".text = autostartConfig;

  home.activation.removeLegacyHyprlandFiles = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    conf="$HOME/.config/hypr/hyprland.conf"
    noctalia_conf="$HOME/.config/hypr/noctalia/noctalia-colors.conf"
    noctalia_dir="$HOME/.config/hypr/noctalia"

    if [ -f "$conf" ] && ${pkgs.gnugrep}/bin/grep -q '^autogenerated = 1$' "$conf"; then
      rm -f "$conf"
    fi

    if [ -f "$noctalia_conf" ]; then
      rm -f "$noctalia_conf"
    fi

    if [ -d "$noctalia_dir" ]; then
      rmdir "$noctalia_dir" 2>/dev/null || true
    fi
  '';
}
