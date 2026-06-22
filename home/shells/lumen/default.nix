# Lumen desktop shell configuration.
{ config, lib, pkgs, ... }:

let
  lockCommand = "hyprlock --config ${config.xdg.configHome}/hypr/hyprlock.conf --immediate-render --no-fade-in";
  configFile = pkgs.writeText "lumen-config.toml" ''
    # Lumen configuration file

    [styling]
    scale = 0.90

    [bar]
    scale = 0.82
    padding = 0.18
    padding-ends = 0.30
    module-gap = 0.25
    button-icon-size = 0.85
    button-icon-padding = 0.55
    button-label-size = 0.85
    button-label-padding = 0.55
    button-gap = 0.45
    button-group-module-gap = 0.15
  '';
  runtimeConfig = pkgs.writeText "lumen-runtime.toml" ''
    [bar]
    inset-edge = 0.5
    inset-ends = 0.5
    padding-ends = 1.2999999523162842
    module-gap = 0.75
    rounding = "md"

    [[bar.layout]]
    monitor = "*"
    show = true
    left = [
        "hyprland-workspaces",
        "media",
    ]
    center = ["clock"]
    right = [
        "idle-inhibit",
        "battery",
        "network",
        "model-usage",
        "dashboard",
    ]

    [modules.bluetooth]
    label-show = false

    [modules.clock]
    format = "%e %b %Y %H:%M"
    time-format = "24h"
    icon-show = false
    button-bg-color = "transparent"

    [modules.dashboard]
    dropdown-lock-command = "${lockCommand}"

    [modules.media]
    icon-type = "default"
    hide-when-nothing-playing = true

    [wallpaper]
    cycling-directory = "${config.home.homeDirectory}/Pictures/Wallpapers"

    [[wallpaper.monitors]]
    name = "eDP-1"
    fit-mode = "fill"
    wallpaper = "${config.home.homeDirectory}/Pictures/Wallpapers/snow-capped-mountains-with-full-moon-lo.jpg"
  '';
in
{
  home.packages = with pkgs; [
    awww
    elephant
    fuzzel
    hyprlock
    matugen
    walker
    lumen
  ];

    xdg.configFile."lumen/config.toml.default".source = configFile;
    xdg.configFile."lumen/runtime.toml.default".source = runtimeConfig;

    xdg.configFile."lumen/styles/index.scss".text = ''
      // Custom Lumen styles. Anything here overrides the built-in styling.
      // Use @import "name" to bring in _name.scss from this folder.

      .model-usage.ok menubutton.bar-button {
        color: #46b576;
      }

      .model-usage.warning menubutton.bar-button {
        color: #e0a93e;
      }

      .model-usage.critical menubutton.bar-button {
        color: #e2604f;
      }

      .model-usage.offline menubutton.bar-button {
        opacity: 0.65;
      }
    '';

    home.activation.seedLumenConfig = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      seed_mutable_config() {
        source_file="$1"
        target_file="$2"

        if [ -L "$target_file" ] || [ ! -e "$target_file" ]; then
          mkdir -p "$(dirname "$target_file")"
          rm -f "$target_file"
          cp "$source_file" "$target_file"
          chmod u+w "$target_file"
        fi
      }

      seed_mutable_config ${configFile} "${config.xdg.configHome}/lumen/config.toml"
      seed_mutable_config ${runtimeConfig} "${config.xdg.configHome}/lumen/runtime.toml"
    '';

    xdg.configFile."walker/config.toml".text = ''
      theme = "lumen"
      close_when_open = true
      click_to_close = true
      single_click_activation = true
      selection_wrap = true
      hide_quick_activation = true
      hide_action_hints = true
      keybind_symbols = true
      resume_last_query = false
      ext_background_effect_blur = true

      [placeholders]
      "default" = { input = "Search apps, commands, files", list = "No results" }
      "desktopapplications" = { input = "Search apps", list = "No apps found" }
      "calc" = { input = "Calculate", list = "Type an expression" }
      "files" = { input = "Search files", list = "No files found" }
      "websearch" = { input = "Search the web", list = "Type a search" }

      [providers]
      default = [
        "desktopapplications",
        "calc",
        "websearch",
      ]
      empty = ["desktopapplications"]
      max_results = 24
      ignore_preview = ["desktopapplications", "calc", "websearch"]

      [[providers.prefixes]]
      prefix = ";"
      provider = "providerlist"

      [[providers.prefixes]]
      prefix = ">"
      provider = "runner"

      [[providers.prefixes]]
      prefix = "/"
      provider = "files"

      [[providers.prefixes]]
      prefix = "."
      provider = "symbols"

      [[providers.prefixes]]
      prefix = "="
      provider = "calc"

      [[providers.prefixes]]
      prefix = "@"
      provider = "websearch"

      [[providers.prefixes]]
      prefix = ":"
      provider = "clipboard"

      [[providers.prefixes]]
      prefix = "$"
      provider = "windows"
    '';

    xdg.configFile."walker/themes/lumen/style.css".text = ''
      @define-color window_bg_color rgba(14, 18, 24, 0.86);
      @define-color panel_bg_color rgba(21, 27, 36, 0.78);
      @define-color field_bg_color rgba(245, 247, 250, 0.08);
      @define-color accent_bg_color rgba(122, 162, 247, 0.45);
      @define-color accent_strong_color #7aa2f7;
      @define-color border_color rgba(232, 236, 243, 0.20);
      @define-color theme_fg_color #f5f7fa;
      @define-color muted_fg_color #d2d8e2;
      @define-color error_bg_color rgba(255, 107, 107, 0.70);
      @define-color error_fg_color #f5f7fa;

      * {
        all: unset;
        font-family: Inter, "JetBrainsMono Nerd Font", sans-serif;
        font-size: 14px;
      }

      popover {
        background: @panel_bg_color;
        border: 1px solid @border_color;
        border-radius: 12px;
        padding: 8px;
      }

      .normal-icons {
        -gtk-icon-size: 22px;
      }

      .large-icons {
        -gtk-icon-size: 34px;
      }

      scrollbar {
        opacity: 0;
      }

      .box-wrapper {
        background: @window_bg_color;
        border: 1px solid @border_color;
        border-radius: 16px;
        box-shadow:
          0 24px 60px rgba(0, 0, 0, 0.42),
          0 6px 20px rgba(0, 0, 0, 0.30);
        padding: 16px;
      }

      .box {
        color: @theme_fg_color;
      }

      .search-container {
        background: @field_bg_color;
        border: 1px solid rgba(232, 236, 243, 0.14);
        border-radius: 12px;
      }

      .input {
        background: transparent;
        color: @theme_fg_color;
        caret-color: @accent_strong_color;
        padding: 13px 15px;
        font-size: 16px;
      }

      .input placeholder {
        color: @muted_fg_color;
        opacity: 0.62;
      }

      .input selection {
        background: alpha(@accent_bg_color, 0.55);
      }

      .content-container {
        margin-top: 8px;
      }

      .preview-box,
      .elephant-hint,
      .placeholder,
      .list {
        color: @theme_fg_color;
      }

      .placeholder,
      .elephant-hint {
        opacity: 0.72;
      }

      .item-box {
        border-radius: 10px;
        padding: 10px 11px;
      }

      child:selected .item-box,
      row:selected .item-box,
      child:hover .item-box {
        background: alpha(@accent_bg_color, 0.26);
      }

      child:selected .item-box {
        box-shadow: inset 0 0 0 1px rgba(122, 162, 247, 0.28);
      }

      .item-text {
        color: @theme_fg_color;
        font-weight: 500;
      }

      .item-subtext {
        color: @muted_fg_color;
        font-size: 12px;
        opacity: 0.68;
      }

      .item-image-text {
        color: @accent_strong_color;
        font-size: 25px;
      }

      .item-quick-activation {
        background: alpha(@accent_bg_color, 0.22);
        border-radius: 6px;
        padding: 6px 8px;
      }

      .providerlist .item-subtext {
        font-size: 12px;
        opacity: 0.82;
      }

      .calc .item-text {
        color: @accent_strong_color;
        font-size: 22px;
      }

      .symbols .item-image {
        color: @accent_strong_color;
        font-size: 24px;
      }

      .preview {
        border: 1px solid @border_color;
        border-radius: 12px;
        color: @theme_fg_color;
      }

      .keybinds {
        border-top: 1px solid rgba(232, 236, 243, 0.12);
        color: @muted_fg_color;
        font-size: 12px;
        margin-top: 4px;
        padding-top: 10px;
      }

      .keybind-button {
        opacity: 0.58;
      }

      .keybind-label {
        border: 1px solid rgba(232, 236, 243, 0.18);
        border-radius: 6px;
        padding: 2px 5px;
      }

      .keybind-bind {
        opacity: 0.46;
      }

      .error {
        background: @error_bg_color;
        border-radius: 10px;
        color: @error_fg_color;
        padding: 10px;
      }
    '';

    xdg.configFile."hypr/hyprlock.conf".text = ''
      $font = Inter
      $wallpaper = ${config.home.homeDirectory}/Pictures/Wallpapers/snow-capped-mountains-with-full-moon-lo.jpg

      general {
          hide_cursor = true
          ignore_empty_input = true
      }

      animations {
          enabled = true
          bezier = ease, 0.25, 0.1, 0.25, 1.0
          animation = fadeIn, 1, 3, ease
          animation = fadeOut, 1, 3, ease
          animation = inputFieldDots, 1, 2, ease
      }

      background {
          monitor =
          path = $wallpaper
          blur_passes = 2
          blur_size = 6
          noise = 0.02
          contrast = 0.95
          brightness = 0.65
          vibrancy = 0.18
          vibrancy_darkness = 0.25
      }

      label {
          monitor =
          text = cmd[update:1000] date +"%H:%M"
          color = rgba(245, 247, 250, 0.95)
          font_size = 84
          font_family = $font
          position = 0, 150
          halign = center
          valign = center
      }

      label {
          monitor =
          text = cmd[update:60000] date +"%A, %d %B"
          color = rgba(210, 216, 226, 0.88)
          font_size = 24
          font_family = $font
          position = 0, 85
          halign = center
          valign = center
      }

      input-field {
          monitor =
          size = 300, 54
          outline_thickness = 2
          dots_size = 0.28
          dots_spacing = 0.22
          dots_center = true
          outer_color = rgba(232, 236, 243, 0.35)
          inner_color = rgba(14, 18, 24, 0.72)
          font_color = rgba(245, 247, 250, 0.96)
          fade_on_empty = false
          placeholder_text = <span foreground="##d2d8e2">Password</span>
          fail_text = <span foreground="##ff8b8b">$PAMFAIL</span>
          check_color = rgba(122, 162, 247, 0.55)
          fail_color = rgba(255, 107, 107, 0.70)
          rounding = 12
          font_family = $font
          position = 0, -18
          halign = center
          valign = center
      }
    '';

    systemd.user.services.lumen = {
      Unit = {
        Description = "Lumen desktop shell";
        PartOf = [ config.wayland.systemd.target ];
      };
      Service = {
        Type = "simple";
        ExecStart = "${pkgs.lumen}/bin/lumen shell";
        Environment = [
          "PATH=${config.home.homeDirectory}/.local/bin:${config.home.homeDirectory}/.npm-global/bin:/etc/profiles/per-user/${config.home.username}/bin:/run/current-system/sw/bin:${lib.makeBinPath [ pkgs.awww pkgs.bash pkgs.coreutils pkgs.hyprlock pkgs.matugen pkgs.python3 pkgs.lumen ]}"
        ];
        Restart = "on-failure";
        RestartSec = 2;
      };
      Install.WantedBy = [ config.wayland.systemd.target ];
    };

    systemd.user.services.elephant = {
      Unit = {
        Description = "Elephant launcher backend";
        PartOf = [ config.wayland.systemd.target ];
      };
      Service = {
        Type = "simple";
        ExecStart = "${pkgs.elephant}/bin/elephant";
        Restart = "on-failure";
        RestartSec = 1;
        ExecStopPost = "${pkgs.coreutils}/bin/rm -f /tmp/elephant.sock";
      };
      Install.WantedBy = [ config.wayland.systemd.target ];
    };

    systemd.user.services.walker = {
      Unit = {
        Description = "Walker launcher service";
        After = [ "elephant.service" ];
        Requires = [ "elephant.service" ];
        PartOf = [ config.wayland.systemd.target ];
      };
      Service = {
        Type = "simple";
        ExecStart = "${pkgs.walker}/bin/walker --gapplication-service";
        Environment = [
          "PATH=${lib.makeBinPath [ pkgs.elephant pkgs.walker ]}"
        ];
        Restart = "on-failure";
        RestartSec = 1;
      };
      Install.WantedBy = [ config.wayland.systemd.target ];
    };
}
