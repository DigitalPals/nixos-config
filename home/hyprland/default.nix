# Hyprland window manager configuration
{ config, pkgs, lib, hostname, osConfig, ... }:

let
  isExternalDisplayLaptop = lib.hasPrefix "G1a" hostname || lib.hasPrefix "xps" hostname;
  noctaliaCommand = lib.getExe config.programs.noctalia.package;
  lockCommand = "${noctaliaCommand} msg session lock";
  launcherCommand = "${noctaliaCommand} msg panel-toggle launcher";
  terminalCommand = "${pkgs.kitty}/bin/kitty";
  hermesDesktopCommand = "${config.home.homeDirectory}/.local/bin/hermes-desktop-remote";

  # Import config generators
  monitorsConfig = import ./monitors.nix { inherit hostname lib; };
  inputConfig = import ./input.nix {};
  looknfeelConfig = import ./looknfeel.nix { inherit hostname lib; };
  noctaliaIntegrationConfig = import ./noctalia-integration.nix {};
  noctaliaThemeFallback = ''
    -- Noctalia replaces this regular file when its Hyprland color template is enabled.
    return {
      apply_theme = function() end,
    }
  '';
  brightnessControl = import ./brightness.nix { inherit pkgs; };
  portalLauncher = pkgs.writeShellScript "portal-launcher" ''
    set -euo pipefail

    portal_binary="${config.home.homeDirectory}/Code/portal/target/release/portal"

    if [ ! -x "$portal_binary" ]; then
      ${pkgs.libnotify}/bin/notify-send "Portal" "Missing executable: $portal_binary" || true
      exit 1
    fi

    export WGPU_BACKEND="''${WGPU_BACKEND:-vulkan,gl}"
    export LD_LIBRARY_PATH="${lib.makeLibraryPath [
      pkgs.wayland
      pkgs.libxkbcommon
      pkgs.vulkan-loader
      pkgs.glib
      pkgs.dbus
    ]}:''${LD_LIBRARY_PATH:-}"

    exec "$portal_binary" "$@"
  '';
  bindingsConfig = import ./bindings.nix {
    inherit brightnessControl hermesDesktopCommand launcherCommand lockCommand portalLauncher terminalCommand;
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
      monitors="$(${pkgs.hyprland}/bin/hyprctl monitors -j 2>/dev/null)" || return 0

      if ! physical_external_monitor_connected; then
        if ! edp_active; then
          hypr_monitor_enable_edp
        fi
        return 0
      fi

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

    ${pkgs.upower}/bin/upower --monitor | while read -r _line; do
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
    inherit pkgs osConfig;
    waylandSystemdTarget = config.wayland.systemd.target;
    preShellCommand = if isExternalDisplayLaptop then externalMonitorApply else null;
  };

  # Hyprland Lua entry point.
  hyprlandExtraConfig = ''
    local xdgConfigHome = os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
    local hyprConfigDir = xdgConfigHome .. "/hypr"
    package.path = hyprConfigDir .. "/?.lua;" .. hyprConfigDir .. "/?/init.lua;" .. package.path

    for _, module in ipairs({ "monitors", "input", "bindings", "looknfeel", "noctalia", "noctalia-integration", "autostart", "external-monitor-toggle" }) do
      package.loaded[module] = nil
    end

    require("monitors")
    require("input")
    require("bindings")
    require("looknfeel")
    require("noctalia").apply_theme()
    require("noctalia-integration")
    require("autostart")
    require("external-monitor-toggle")
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
  xdg.configFile."hypr/noctalia-integration.lua".text = noctaliaIntegrationConfig;
  xdg.configFile."hypr/noctalia.lua" = {
    text = noctaliaThemeFallback;
    force = true;
    onChange = lib.mkForce "";
  };
  xdg.configFile."hypr/autostart.lua".text = autostartConfig;
  xdg.configFile."hypr/hyprland.lua" = {
    force = true;
    onChange = lib.mkForce "";
  };

  # Hyprland resolves Home Manager's symlinked hyprland.lua into /nix/store at
  # startup. Keep the entry point as a regular file at the stable XDG path so
  # reloads after a rebuild read the new config instead of the old store path.
  home.activation.materializeHyprlandLuaEntrypoint = lib.hm.dag.entryAfter [ "onFilesChange" ] ''
    config_file="${config.xdg.configHome}/hypr/hyprland.lua"

    if [ -L "$config_file" ]; then
      target="$(${pkgs.coreutils}/bin/readlink -f "$config_file")"
      tmp="$config_file.tmp"

      ${pkgs.coreutils}/bin/install -m 0644 "$target" "$tmp"
      ${pkgs.coreutils}/bin/mv -f "$tmp" "$config_file"
    fi

    runtime_dir="''${XDG_RUNTIME_DIR:-/run/user/$(${pkgs.coreutils}/bin/id -u)}"
    if [ -d "$runtime_dir/hypr" ]; then
      ${pkgs.hyprland}/bin/hyprctl instances -j 2>/dev/null \
        | ${pkgs.jq}/bin/jq -r '.[].instance' 2>/dev/null \
        | while read -r instance; do
            [ -n "$instance" ] || continue
            ${pkgs.hyprland}/bin/hyprctl -i "$instance" reload >/dev/null 2>&1 || true
          done
    fi
  '';

  # Noctalia's color template renderer updates this module at runtime. Keep it
  # at a stable writable path for the same reason as the main Lua entrypoint.
  home.activation.materializeNoctaliaHyprTheme = lib.hm.dag.entryAfter [ "onFilesChange" ] ''
    theme_file="${config.xdg.configHome}/hypr/noctalia.lua"

    if [ -L "$theme_file" ]; then
      target="$(${pkgs.coreutils}/bin/readlink -f "$theme_file")"
      tmp="$theme_file.tmp"
      ${pkgs.coreutils}/bin/install -m 0644 "$target" "$tmp"
      ${pkgs.coreutils}/bin/mv -f "$tmp" "$theme_file"
    fi
  '';

  home.activation.removeLegacyHyprlandFiles = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    conf="$HOME/.config/hypr/hyprland.conf"
    if [ -f "$conf" ] && ${pkgs.gnugrep}/bin/grep -q '^autogenerated = 1$' "$conf"; then
      rm -f "$conf"
    fi
  '';
}
