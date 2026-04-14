# Monitor configuration
# Host-specific display setup
{ hostname, lib ? builtins }:

let
  # Check base hostname (handles -illogical suffix)
  isKraken = lib.hasPrefix "kraken" hostname;
  isG1a = lib.hasPrefix "G1a" hostname;
  isXps = lib.hasPrefix "xps" hostname;
  isExternalDisplayLaptop = isG1a || isXps;
  monitorConfig = if isKraken then ''
    # Kraken: 4K display at 165Hz with 1.5x scaling
    monitor = ,3840x2160@165,auto,1.5
    env = GDK_SCALE,1.5
  '' else if isExternalDisplayLaptop then ''
    # Laptop: eDP-1 is auto-disabled by external-monitor-toggle when a
    # known external display is connected (XDR, AORUS, XREAL).
    monitor = eDP-1,preferred,0x0,auto
    monitor = desc:GIGA-BYTE TECHNOLOGY CO. LTD. AORUS FO32U2,3840x2160@120,auto,auto
    monitor = desc:Apple Computer Inc Studio XDR,preferred,auto,auto,bitdepth,10,cm,auto,vrr,0
    monitor = desc:Apple Computer Inc Pro Display XDR,preferred,auto,auto,bitdepth,10,cm,auto,vrr,0
    monitor = desc:Nreal XREAL One Pro,1920x1080@120,auto,auto,vrr,0
    monitor = desc:XREAL XREAL One Pro,1920x1080@120,auto,auto,vrr,0
    # Default workspace on external monitors
    workspace = 1, monitor:desc:GIGA-BYTE TECHNOLOGY CO. LTD. AORUS FO32U2, default:true
    workspace = 1, monitor:desc:Apple Computer Inc Studio XDR, default:true
    workspace = 1, monitor:desc:Apple Computer Inc Pro Display XDR, default:true
    workspace = 1, monitor:desc:Nreal XREAL One Pro, default:true
    workspace = 1, monitor:desc:XREAL XREAL One Pro, default:true
    env = GDK_SCALE,2
  '' else ''
    # Laptop: Auto-detect with native scaling
    monitor = ,preferred,auto,auto,vrr,1
    env = GDK_SCALE,2
  '';
in ''
  # See https://wiki.hyprland.org/Configuring/Monitors/
  # List current monitors and resolutions: hyprctl monitors
  # Format: monitor = [port], resolution, position, scale

  ${monitorConfig}
''
