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
      # Direct hl.monitor(... disabled = false) returns ok but does not restore
      # eDP-1 while Hyprland is on FALLBACK after the Xreal disconnects.
      ${pkgs.coreutils}/bin/timeout 5s ${pkgs.hyprland}/bin/hyprctl reload >/dev/null || true
    }

    hypr_monitor_disable_edp() {
      ${pkgs.hyprland}/bin/hyprctl eval 'hl.monitor({ output = "eDP-1", disabled = true })' >/dev/null || true
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

    lid_closed() {
      for state in /proc/acpi/button/lid/*/state; do
        [ -r "$state" ] || continue
        case "$(<"$state")" in
          *closed*) return 0 ;;
        esac
      done

      return 1
    }

    xreal_monitor_active() {
      printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '
        .[]
        | select(
            ((.disabled // false) | not)
            and (
              ((.description // "") | test("(^| )Nreal XREAL|(^| )XREAL"; "i"))
              or ((.make // "") | test("^(Nreal|XREAL)$"; "i"))
              or ((.model // "") | test("XREAL"; "i"))
            )
          )
      ' > /dev/null 2>&1
    }

    edp_active() {
      printf '%s\n' "$monitors" | ${pkgs.jq}/bin/jq -e '
        .[]
        | select(.name == "eDP-1")
        | select((.disabled // false) | not)
      ' > /dev/null 2>&1
    }

    apply_monitor_state() {
      if ! physical_external_monitor_connected; then
        hypr_monitor_enable_edp
        return 0
      fi

      monitors="$(${pkgs.hyprland}/bin/hyprctl monitors -j 2>/dev/null)" || return 0

      if lid_closed || xreal_monitor_active; then
        if edp_active; then
          hypr_monitor_disable_edp
        fi
      else
        if ! edp_active; then
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

  # Listen for Hyprland hotplug events. This mirrors the pre-Lua implementation:
  # socket2 tells us when to re-check, and /sys/class/drm decides whether the
  # external connector is physically gone before we restore eDP.
  externalMonitorToggle = pkgs.writeShellScript "external-monitor-toggle" ''
    ${externalMonitorFunctions}

    if [ -z "''${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
      exit 0
    fi

    apply_monitor_state

    ${pkgs.upower}/bin/upower --monitor-detail | while read -r _line; do
      apply_monitor_state
    done &
    upower_monitor_pid=$!

    cleanup() {
      kill "$upower_monitor_pid" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM

    ${pkgs.socat}/bin/socat -U - "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" | while read -r line; do
      case "$line" in
        monitoradded'>>'*|monitorremoved'>>'*)
          output="''${line#*>>}"
          case "$output" in
            eDP-1|FALLBACK)
              ;;
            *)
              sleep 2
              apply_monitor_state
              ;;
          esac
          ;;
      esac
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
  # panel for clamshell mode and Xreal/Nreal glasses.
  xdg.configFile."hypr/external-monitor-toggle.lua".text =
    if isExternalDisplayLaptop then ''
      hl.on("hyprland.start", function()
        hl.exec_cmd([[${externalMonitorToggle}]])
      end)
    '' else "";

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
