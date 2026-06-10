# Look and feel configuration
# Animations, decorations, layout, and window rules
{ hostname, lib ? builtins }:

let
  shellColors = builtins.fromJSON (builtins.readFile ../shells/common/colors.json);
  rgb = hex: "rgb(${lib.removePrefix "#" hex})";

  # Check base hostname (handles -illogical suffix)
  _isG1a = lib.hasPrefix "G1a" hostname;
  _isKraken = lib.hasPrefix "kraken" hostname;
  _isProart = lib.hasPrefix "proart" hostname;
  softwareCursors = if _isProart then "true" else "false";

  # Dwindle layout on all hosts
  useDwindle = true;
  layoutType = if useDwindle then "dwindle" else "master";

  dwindleConfig = if useDwindle then ''
    -- Layout
    -- https://wiki.hyprland.org/Configuring/Layouts/Dwindle-Layout/
    hl.config({
      dwindle = {
        preserve_split = true,
        split_width_multiplier = 1.0,
      },
    })
  '' else "";

  masterConfig = if !useDwindle then ''
    -- Master layout for laptop
    -- https://wiki.hyprland.org/Configuring/Layouts/Master-Layout/
    hl.config({
      master = {
        mfact = 0.5, -- 50/50 split between master and stack
      },
    })
  '' else "";
in
''
  -- General appearance.
  -- https://wiki.hyprland.org/Configuring/Basics/Variables/#general
  hl.config({
    general = {
      gaps_in = 5,
      gaps_out = 10,
      border_size = 1,
      layout = "${layoutType}",
      col = {
        active_border = "${rgb shellColors.mPrimary}",
        inactive_border = "${rgb shellColors.mSurface}",
      },
    },

    -- https://wiki.hypr.land/Nvidia/ - software cursors prevent lag spikes on NVIDIA/hybrid.
    cursor = {
      no_hardware_cursors = ${softwareCursors},
    },

    decoration = {
      rounding = 10,

      blur = {
        enabled = true,
        size = 3,
        passes = 1,
      },

      shadow = {
        enabled = true,
        range = 4,
        render_power = 3,
        color = "rgba(1a1a1aee)",
      },
    },

    animations = {
      enabled = true,
    },

    misc = {
      force_default_wallpaper = 0,
      disable_hyprland_logo = true,
      focus_on_activate = false,
    },

    xwayland = {
      force_zero_scaling = true,
    },

    group = {
      col = {
        border_active = "${rgb shellColors.mSecondary}",
        border_inactive = "${rgb shellColors.mSurface}",
        border_locked_active = "${rgb shellColors.mError}",
        border_locked_inactive = "${rgb shellColors.mSurface}",
      },

      groupbar = {
        col = {
          active = "${rgb shellColors.mSecondary}",
          inactive = "${rgb shellColors.mSurface}",
          locked_active = "${rgb shellColors.mError}",
          locked_inactive = "${rgb shellColors.mSurface}",
        },
      },
    },
  })

  -- Animations.
  -- https://wiki.hyprland.org/Configuring/Advanced-and-Cool/Animations/
  hl.curve("myBezier", { type = "bezier", points = { { 0.05, 0.9 }, { 0.1, 1.05 } } })
  hl.animation({ leaf = "windows", enabled = true, speed = 7, bezier = "myBezier" })
  hl.animation({ leaf = "windowsOut", enabled = true, speed = 7, bezier = "default", style = "popin 80%" })
  hl.animation({ leaf = "border", enabled = true, speed = 10, bezier = "default" })
  hl.animation({ leaf = "borderangle", enabled = true, speed = 8, bezier = "default" })
  hl.animation({ leaf = "fade", enabled = true, speed = 7, bezier = "default" })
  hl.animation({ leaf = "workspaces", enabled = true, speed = 6, bezier = "default" })

${dwindleConfig}
${masterConfig}
  -- Window and workspace rules.
  -- https://wiki.hyprland.org/Configuring/Basics/Window-Rules/
  -- https://wiki.hyprland.org/Configuring/Basics/Workspace-Rules/

  -- File dialogs.
  hl.window_rule({ match = { class = [[xdg-desktop-portal-gtk]] }, float = true })
  hl.window_rule({ match = { class = [[org\.gnome\.Nautilus]], title = [[Properties]] }, float = true })
  hl.window_rule({ match = { class = [[org\.gnome\.Nautilus]], title = [[Open.*]] }, float = true })
  hl.window_rule({ match = { class = [[org\.gnome\.Nautilus]], title = [[Save.*]] }, float = true })

  -- On large external monitors, keep a lone tiled window narrower instead of edge-to-edge.
  hl.workspace_rule({ workspace = "m[desc:GIGA-BYTE TECHNOLOGY CO. LTD. AORUS FO32U2] w[t1]", gaps_out = { top = 10, right = 480, bottom = 10, left = 480 } })
  hl.workspace_rule({ workspace = "m[desc:ASUSTek COMPUTER INC XG27JCG] w[t1]", gaps_out = { top = 10, right = 320, bottom = 10, left = 320 } })
  hl.workspace_rule({ workspace = "m[desc:Apple Computer Inc Studio XDR] w[t1]", gaps_out = { top = 10, right = 320, bottom = 10, left = 320 } })

  -- Suppress maximize for all windows.
  hl.window_rule({ match = { class = [[.*]] }, suppress_event = "maximize" })

  -- Opaque by default (use SUPER+BACKSPACE to toggle transparency).
  hl.window_rule({ match = { class = [[.*]] }, opacity = "1.0 1.0" })

  -- Floating windows - 1Password.
  hl.window_rule({
    match = { class = [[1[pP]assword]] },
    float = true,
    center = true,
    size = { 875, 600 },
    no_screen_share = true,
  })

  -- Floating windows - Sushi (Nautilus quick preview).
  hl.window_rule({
    match = { class = [[org\.gnome\.NautilusPreviewer]] },
    float = true,
    center = true,
    size = { "60%", "70%" },
  })

  -- Floating windows - LocalSend.
  hl.window_rule({
    match = { class = [[localsend_app]] },
    float = true,
    center = true,
    size = { 875, 600 },
  })

  -- Floating windows - Calculator.
  hl.window_rule({ match = { class = [[org\.gnome\.Calculator]] }, float = true })

  -- Floating windows - Media viewers.
  hl.window_rule({ match = { class = [[(imv|mpv)]] }, float = true, center = true })

  -- Chrome notification popups - float and prevent focus stealing.
  hl.window_rule({
    match = { class = [[google-chrome]], title = [[^Notification.*]] },
    float = true,
    no_initial_focus = true,
    pin = true,
  })

  -- No transparency on media windows.
  hl.window_rule({ match = { class = [[(vlc|mpv|imv|zoom)]] }, opacity = "1 1" })
''
