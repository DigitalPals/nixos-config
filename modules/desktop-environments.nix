# Desktop environment configuration shared across machines
{ config, pkgs, lib, username, ... }:

let
  # Import Hyprland session packages
  hyprlandSessions = pkgs.callPackage ../packages/hyprland-sessions { };
in
{
  # Auto-login directly to Hyprland with Noctalia shell (no session selector)
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${hyprlandSessions.script}/bin/hyprland-noctalia";
        user = username;
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

  # Ensure Home Manager has populated Hyprland config before greetd autologin.
  systemd.services.greetd = {
    after = [ "home-manager-${username}.service" ];
    wants = [ "home-manager-${username}.service" ];
  };

  # Hyprland at system level (for session registration)
  programs.hyprland = {
    enable = true;
    xwayland.enable = true;
  };

  # XDG Portal for Hyprland (screen sharing, file dialogs, dark mode)
  # GTK portal is patched via overlay to include Hyprland in UseIn
  xdg.portal = {
    enable = true;
    extraPortals = [
      pkgs.xdg-desktop-portal-hyprland
      pkgs.xdg-desktop-portal-gtk
    ];
    config.common.default = [ "hyprland" "gtk" ];
  };

  # Register Hyprland session with display manager (for fallback/GNOME login)
  services.displayManager.sessionPackages = [ hyprlandSessions.noctalia ];

  # Hyprland wrapper script in PATH
  environment.systemPackages = [ hyprlandSessions.script ];
}
