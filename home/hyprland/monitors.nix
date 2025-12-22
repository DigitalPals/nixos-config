# Monitor configuration
# Host-specific display setup
{ hostname }:

let
  monitorConfig = if hostname == "kraken" then ''
    # Kraken: 4K display at 165Hz with 1.5x scaling
    monitor = ,3840x2160@165,auto,1.5
    env = GDK_SCALE,1.5
  '' else ''
    # Laptop: Auto-detect with native scaling
    monitor = ,preferred,auto,auto
    env = GDK_SCALE,2
  '';
in ''
  # See https://wiki.hyprland.org/Configuring/Monitors/
  # List current monitors and resolutions: hyprctl monitors
  # Format: monitor = [port], resolution, position, scale

  ${monitorConfig}
''
