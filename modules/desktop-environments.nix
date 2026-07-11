# Desktop environment configuration shared across machines
{ config, pkgs, username, ... }:

let
  # Import Hyprland session packages
  hyprlandSessions = pkgs.callPackage ../packages/hyprland-sessions { };
  hyprlandLumen = "${hyprlandSessions.script}/bin/hyprland-lumen";
  isG1a = config.networking.hostName == "G1a";
  greetdSessionSettings = command:
    if isG1a then
      {
        initial_session = {
          inherit command;
          user = username;
        };
        default_session.command = "${pkgs.greetd}/bin/agreety --cmd ${command}";
      }
    else
      {
        default_session = {
          inherit command;
          user = username;
        };
      };
in
{
  # Auto-login directly to Hyprland with Lumen shell (no session selector)
  services.greetd = {
    enable = true;
    useTextGreeter = isG1a;
    settings = greetdSessionSettings hyprlandLumen;
  };

  systemd.services.greetd = {
    # Ensure Home Manager has populated Hyprland config before greetd autologin
    after = [ "home-manager-${username}.service" ];
    wants = [ "home-manager-${username}.service" ];
    # Prevent greetd from cluttering TTY with logs
    serviceConfig = {
      Type = "idle";
      StandardInput = "tty";
      StandardOutput = "tty";
      StandardError = "journal";
      TTYReset = true;
      TTYVHangup = true;
      TTYVTDisallocate = true;
    };
  };

  # Hyprland at system level (for session registration)
  programs.hyprland = {
    enable = true;
    xwayland.enable = true;
  };

  # XDG Portal for Hyprland (screen sharing, file dialogs, dark mode)
  # GTK portal is patched via overlay to include Hyprland in UseIn.
  xdg.portal = {
    enable = true;
    extraPortals = [
      pkgs.xdg-desktop-portal-hyprland
      pkgs.xdg-desktop-portal-gtk
    ];
    config.common.default = [ "hyprland" "gtk" ];
  };

  # Register sessions with display manager (for fallback/GNOME login)
  services.displayManager.sessionPackages = [
    hyprlandSessions.lumen
  ];

  # Hyprland wrapper script in PATH
  environment.systemPackages = [
    hyprlandSessions.script
  ];

}
