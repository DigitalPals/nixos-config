# Monitor configuration
# Host-specific display setup
{ hostname, lib ? builtins }:

let
  # Check base hostname (handles -illogical suffix)
  isKraken = lib.hasPrefix "kraken" hostname;
  isG1a = lib.hasPrefix "G1a" hostname;
  isXps = lib.hasPrefix "xps" hostname;
  isZ2MiniG1a = hostname == "z2-mini-g1a";
  isExternalDisplayLaptop = isG1a || isXps;
  monitorConfig = if isKraken then ''
    -- Kraken: 4K display at 165Hz with 1.5x scaling.
    hl.monitor({
      output = "",
      mode = "3840x2160@165",
      position = "auto",
      scale = 1.5,
    })

    hl.env("GDK_SCALE", "1.5")
  '' else if isZ2MiniG1a then ''
    -- HP Z2 Mini G1a: external desktop display. Keep VRR disabled; AMDGPU
    -- warned in the VRR transition path when resuming the Studio Display.
    hl.monitor({
      output = "",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      vrr = 0,
    })
    hl.monitor({
      output = "desc:Apple Computer Inc Studio XDR",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      bitdepth = 10,
      cm = "auto",
      vrr = 0,
    })
    hl.monitor({
      output = "desc:Apple Computer Inc Pro Display XDR",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      bitdepth = 10,
      cm = "auto",
      vrr = 0,
    })

    hl.workspace_rule({ workspace = "1", monitor = "desc:Apple Computer Inc Studio XDR", default = true })
    hl.workspace_rule({ workspace = "1", monitor = "desc:Apple Computer Inc Pro Display XDR", default = true })

    hl.env("GDK_SCALE", "2")
  '' else if isExternalDisplayLaptop then ''
    -- Laptop: eDP-1 is auto-disabled by external-monitor-toggle when a
    -- known external display is connected (XDR, AORUS, XREAL).
    hl.monitor({ output = "eDP-1", mode = "preferred", position = "0x0", scale = "auto" })
    hl.monitor({
      output = "desc:GIGA-BYTE TECHNOLOGY CO. LTD. AORUS FO32U2",
      mode = "3840x2160@120",
      position = "auto",
      scale = "auto",
    })
    hl.monitor({
      output = "desc:Apple Computer Inc Studio XDR",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      bitdepth = 10,
      cm = "auto",
      vrr = 0,
    })
    hl.monitor({
      output = "desc:Apple Computer Inc Pro Display XDR",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      bitdepth = 10,
      cm = "auto",
      vrr = 0,
    })
    hl.monitor({
      output = "desc:Nreal XREAL One Pro",
      mode = "1920x1080@120",
      position = "auto",
      scale = "auto",
      vrr = 0,
    })
    hl.monitor({
      output = "desc:XREAL XREAL One Pro",
      mode = "1920x1080@120",
      position = "auto",
      scale = "auto",
      vrr = 0,
    })

    -- Default workspace on external monitors.
    hl.workspace_rule({ workspace = "1", monitor = "desc:GIGA-BYTE TECHNOLOGY CO. LTD. AORUS FO32U2", default = true })
    hl.workspace_rule({ workspace = "1", monitor = "desc:Apple Computer Inc Studio XDR", default = true })
    hl.workspace_rule({ workspace = "1", monitor = "desc:Apple Computer Inc Pro Display XDR", default = true })
    hl.workspace_rule({ workspace = "1", monitor = "desc:Nreal XREAL One Pro", default = true })
    hl.workspace_rule({ workspace = "1", monitor = "desc:XREAL XREAL One Pro", default = true })

    hl.env("GDK_SCALE", "2")
  '' else ''
    -- Laptop: Auto-detect with native scaling.
    hl.monitor({
      output = "",
      mode = "preferred",
      position = "auto",
      scale = "auto",
      vrr = 1,
    })

    hl.env("GDK_SCALE", "2")
  '';
in ''
  -- See https://wiki.hyprland.org/Configuring/Basics/Monitors/
  -- List current monitors and resolutions: hyprctl monitors all

  ${monitorConfig}
''
