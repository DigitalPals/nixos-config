# Autostart configuration
# Programs to run at Hyprland startup
{}:

''
  # Systemd integration - export environment for user services
  exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
  exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP

  # Start Noctalia desktop shell
  exec-once = noctalia-shell
''
