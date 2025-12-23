# Desktop environment configuration shared across machines
{ config, pkgs, lib, shell ? "noctalia", ... }:

let
  # Import Hyprland session packages
  hyprlandSessions = pkgs.callPackage ../packages/hyprland-sessions { };

  # Select session script based on shell
  sessionScript = if shell == "illogical"
    then "${hyprlandSessions.illogicalScript}/bin/hyprland-illogical"
    else "${hyprlandSessions.noctaliaScript}/bin/hyprland-noctalia";

  # Select wrapper script for PATH based on shell
  wrapperScript = if shell == "illogical"
    then hyprlandSessions.illogicalScript
    else hyprlandSessions.noctaliaScript;
in
{
  # Auto-login directly to Hyprland with selected shell (no session selector)
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = sessionScript;
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

  # Hyprland wrapper script in PATH (shell-specific)
  environment.systemPackages = [ wrapperScript ];
}
