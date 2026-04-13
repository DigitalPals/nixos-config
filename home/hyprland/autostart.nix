# Autostart configuration
# Programs to run at Hyprland startup
{ pkgs, lib, osConfig }:

let
  # GTK portal executable path (provides Settings interface for dark mode)
  gtkPortal = "${pkgs.xdg-desktop-portal-gtk}/libexec/xdg-desktop-portal-gtk";

  # Noctalia's Hyprland template processing expects to be able to write this file.
  # Ensure it's a regular, user-writable file before starting the shell.
  ensureNoctaliaHyprColorsWritable = pkgs.writeShellScript "ensure-noctalia-hypr-colors-writable" ''
    set -eu
    dir="$HOME/.config/hypr/noctalia"
    file="$dir/noctalia-colors.conf"

    mkdir -p "$dir"

    # If something (e.g. Home Manager) created a symlink here, replace it.
    if [ -L "$file" ]; then
      rm -f "$file"
    fi

    touch "$file"
    chmod 0644 "$file"
  '';
in
''
  # PAM service for Noctalia lock screen auth.
  env = NOCTALIA_PAM_SERVICE,noctalia

  # Systemd integration - export environment for user services
  # Include HYPRLAND_INSTANCE_SIGNATURE so portal services can connect
  exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE NOCTALIA_PAM_SERVICE
  exec-once = dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE NOCTALIA_PAM_SERVICE

  # Portal setup: GTK portal provides Settings interface (dark mode)
  # Start GTK portal first, wait for D-Bus registration, then restart main portal
  exec-once = sleep 1 && ${gtkPortal} &
  exec-once = sleep 2 && systemctl --user restart xdg-desktop-portal-hyprland xdg-desktop-portal

  # Polkit agent: badged supports fingerprint, hyprpolkitagent is password-only
  exec-once = ${if osConfig.services.fprintd.enable then "badged" else "systemctl --user start hyprpolkitagent"}

  # Start desktop shell
  exec-once = ${ensureNoctaliaHyprColorsWritable}
  exec-once = noctalia-shell
''
