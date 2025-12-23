# Autostart configuration
# Programs to run at Hyprland startup (shell-aware)
{ shell ? "noctalia" }:

let
  # Shell-specific autostart commands
  illogicalAutostart = ''
    # Systemd integration - export environment for user services
    exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
    exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
    exec-once = dbus-update-activation-environment --all

    # Core components
    exec-once = gnome-keyring-daemon --start --components=secrets
    exec-once = hypridle

    # Start Quickshell with illogical-impulse config
    exec-once = quickshell -c ~/.config/quickshell/ii

    # Clipboard history with quickshell integration
    exec-once = wl-paste --type text --watch cliphist store
    exec-once = wl-paste --type image --watch cliphist store

    # Set cursor theme
    exec-once = hyprctl setcursor Bibata-Modern-Classic 24
  '';

  noctaliaAutostart = ''
    # Systemd integration - export environment for user services
    exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
    exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP

    # Start desktop shell
    exec-once = noctalia-shell
  '';

  caelestiaAutostart = ''
    # Systemd integration - export environment for user services
    exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
    exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP

    # Start Caelestia shell (uses its own quickshell config)
    exec-once = caelestia-shell
  '';
in
  if shell == "illogical" then illogicalAutostart
  else if shell == "caelestia" then caelestiaAutostart
  else noctaliaAutostart
