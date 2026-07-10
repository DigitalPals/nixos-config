# Noctalia v5 desktop shell configuration.
{ config, lib, pkgs, ... }:

let
  wallpaper = "${config.home.homeDirectory}/Pictures/Wallpapers/snow-capped-mountains-with-full-moon-lo.jpg";
  initialState = pkgs.writeText "noctalia-initial-settings.toml" ''
    # Mutable first-run state. Noctalia owns this file after it is seeded.
    [wallpaper.default]
    path = "${wallpaper}"
  '';
in
{
  programs.noctalia = {
    enable = true;
    systemd.enable = true;
    validateConfig = true;

    settings = {
      shell = {
        ui_scale = 0.9;
        corner_radius_scale = 1.0;
        font_family = "Inter";
        time_format = "{:%H:%M}";
        date_format = "%A, %d %B";
        setup_wizard_enabled = false;
        telemetry_enabled = false;
        launch_apps_as_systemd_services = true;

        panel = {
          launcher_placement = "floating";
          launcher_position = "center";
          control_center_placement = "attached";
          wallpaper_placement = "attached";
          session_placement = "attached";
        };

        launcher = {
          categories = true;
          show_icons = true;
          compact = false;
          app_grid = false;
          sort_by_usage = true;
        };
      };

      theme = {
        mode = "dark";
        source = "builtin";
        builtin = "Tokyo-Night";
      };

      bar = {
        order = [ "main" ];
        main = {
          position = "top";
          enabled = true;
          auto_hide = false;
          reserve_space = true;
          layer = "top";
          thickness = 34;
          background_opacity = 0.96;
          radius = 12;
          margin_ends = 5;
          margin_edge = 5;
          padding = 10;
          widget_spacing = 6;
          scale = 0.82;
          font_family = "Inter";
          capsule = false;
          start = [ "workspaces" "media" ];
          center = [ "clock" ];
          end = [ "caffeine" "battery" "network" "control-center" ];
        };
      };

      widget = {
        clock = {
          format = "{:%e %b %Y %H:%M}";
          tooltip_format = "{:%A, %d %B %Y}";
        };
        media = {
          hide_when_no_media = true;
          max_length = 220;
          min_length = 80;
        };
        network.show_label = true;
        workspaces = {
          display = "id";
          max_label_chars = 2;
          focused_output_only = false;
          hide_when_empty = false;
        };
      };

      wallpaper = {
        enabled = true;
        fill_mode = "crop";
        directory = "${config.home.homeDirectory}/Pictures/Wallpapers";
        transition = [ "fade" ];
        transition_duration = 500;
        transition_on_startup = false;
      };

      lockscreen = {
        enabled = true;
        fingerprint = true;
        allow_empty_password = false;
        blurred_desktop = false;
        wallpaper = wallpaper;
      };

      notification = {
        enable_daemon = true;
        position = "top_right";
        layer = "top";
      };
    };
  };

  # The declarative config is the stable base layer. Back up stale state from
  # earlier Noctalia experiments once, then seed a clean writable state file.
  # After this migration Noctalia owns the file and Settings changes persist.
  home.activation.seedNoctaliaState = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    state_dir="${config.xdg.stateHome}/noctalia"
    state_file="${config.xdg.stateHome}/noctalia/settings.toml"
    migration_marker="$state_dir/.forge-v5-migrated"

    mkdir -p "$state_dir"
    if [ ! -e "$migration_marker" ]; then
      if [ -e "$state_file" ]; then
        backup="$state_dir/settings.toml.pre-forge-v5"
        if [ ! -e "$backup" ]; then
          ${pkgs.coreutils}/bin/cp -p "$state_file" "$backup"
        fi
      fi
      install -m 0600 ${initialState} "$state_file"
      touch "$migration_marker"
    fi
  '';

  # Make the first switch replace the already-running legacy shell without a
  # logout. These commands are best-effort because activation can also run at
  # boot before the graphical user bus exists.
  home.activation.switchToNoctalia = lib.hm.dag.entryAfter [ "reloadSystemd" ] ''
    if ${pkgs.systemd}/bin/systemctl --user show-environment >/dev/null 2>&1; then
      ${pkgs.systemd}/bin/systemctl --user stop lumen.service walker.service elephant.service >/dev/null 2>&1 || true
      ${pkgs.coreutils}/bin/rm -f /tmp/elephant.sock
      printf '%s\n' noctalia > "''${XDG_RUNTIME_DIR:-/run/user/$(${pkgs.coreutils}/bin/id -u)}/desktop-shell"
      ${pkgs.systemd}/bin/systemctl --user set-environment DESKTOP_SHELL=noctalia >/dev/null 2>&1 || true
      ${pkgs.systemd}/bin/systemctl --user restart noctalia.service >/dev/null 2>&1 || true
    fi
  '';
}
