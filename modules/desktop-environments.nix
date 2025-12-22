# Desktop environment configuration shared across machines
{ config, pkgs, lib, ... }:

let
  # Import Hyprland session packages
  hyprlandSessions = pkgs.callPackage ../packages/hyprland-sessions { };
in
{
  # Auto-login directly to Hyprland with Noctalia (no session selector)
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${hyprlandSessions.script}/bin/hyprland-noctalia";
        user = "john";
      };
    };
  };

  # Prevent greetd from cluttering TTY with logs
  systemd.services.greetd.serviceConfig = {
    Type = "idle";
    StandardInput = "tty";
    StandardOutput = "tty";
    StandardError = "journal";
    TTYReset = true;
    TTYVHangup = true;
    TTYVTDisallocate = true;
  };

  # Hyprland at system level (for session registration)
  programs.hyprland = {
    enable = true;
    xwayland.enable = true;
  };

  # XDG Portal for Hyprland (screen sharing, file dialogs)
  xdg.portal = {
    enable = true;
    extraPortals = [ pkgs.xdg-desktop-portal-hyprland ];
  };

  # Register Hyprland session with display manager (for fallback/GNOME login)
  services.displayManager.sessionPackages = hyprlandSessions.sessions;

  # Hyprland wrapper script in PATH
  environment.systemPackages = [ hyprlandSessions.script ];
}
