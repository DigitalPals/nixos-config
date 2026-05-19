# Autostart configuration
# Programs to run at Hyprland startup
{ pkgs, lib, osConfig, preShellCommand ? null }:

let
  # GTK portal executable path (provides Settings interface for dark mode)
  gtkPortal = "${pkgs.xdg-desktop-portal-gtk}/libexec/xdg-desktop-portal-gtk";

  shellStartup = lib.concatStringsSep " && " (
    lib.optional (preShellCommand != null) "${preShellCommand}"
    ++ [ "noctalia-shell" ]
  );
in
''
  -- PAM service for Noctalia lock screen auth.
  hl.env("NOCTALIA_PAM_SERVICE", "noctalia")

  hl.on("hyprland.start", function()
    -- Systemd integration - export environment for user services.
    -- Include HYPRLAND_INSTANCE_SIGNATURE so portal services can connect.
    hl.exec_cmd([[systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE NOCTALIA_PAM_SERVICE]])
    hl.exec_cmd([[dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE NOCTALIA_PAM_SERVICE]])

    -- Portal setup: GTK portal provides Settings interface (dark mode).
    -- Start GTK portal first, wait for D-Bus registration, then restart main portal.
    hl.exec_cmd([[sleep 1 && ${gtkPortal} &]])
    hl.exec_cmd([[sleep 2 && systemctl --user restart xdg-desktop-portal-hyprland xdg-desktop-portal]])

    -- Polkit agent: badged supports fingerprint, hyprpolkitagent is password-only.
    hl.exec_cmd([[${if osConfig.services.fprintd.enable then "badged" else "systemctl --user start hyprpolkitagent"}]])

    -- Start desktop shell.
    hl.exec_cmd([[${shellStartup}]])
  end)
''
