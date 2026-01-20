# Autostart configuration
# Programs to run at Hyprland startup
{ pkgs }:

let
  # GTK portal executable path (provides Settings interface for dark mode)
  gtkPortal = "${pkgs.xdg-desktop-portal-gtk}/libexec/xdg-desktop-portal-gtk";
in
''
  # Systemd integration - export environment for user services
  # Include HYPRLAND_INSTANCE_SIGNATURE so portal services can connect
  exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE
  exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE

  # Portal setup: GTK portal provides Settings interface (dark mode)
  # Start GTK portal first, wait for D-Bus registration, then restart main portal
  exec-once = sleep 1 && ${gtkPortal} &
  exec-once = sleep 2 && systemctl --user restart xdg-desktop-portal-hyprland xdg-desktop-portal

  # Start desktop shell
  exec-once = noctalia-shell
''
