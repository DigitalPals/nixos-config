# Noctalia v5 test specialisation
#
# Adds a "noctalia-v5" entry to the boot menu on every host.
# Booting that entry runs Noctalia v5 (early alpha) instead of the
# current quickshell-based stable shell. The default boot entry is
# completely unaffected.
#
# Known limitations in the v5 specialisation:
#  - The Hyprland autostart still tries to exec "noctalia-shell" (silently
#    fails because the binary is not in PATH). Noctalia v5 starts instead
#    via its systemd user service.
#  - On laptop hosts (G1a, xps, proart) the display-change restart helper
#    in the Hyprland monitor scripts still references the old noctalia-shell
#    store path. Monitor hot-plug restarts will not work correctly.
{ config, lib, inputs, ... }:

let
  username = config.forge.installer.username;
  homeDir = "/home/${username}";
in
{
  specialisation.noctalia-v5.configuration = {
    # Make the v5 Home Manager module available to all users.
    home-manager.sharedModules = [
      inputs.noctalia-v5.homeModules.default
    ];

    # Per-user configuration for the v5 specialisation.
    home-manager.users.${username} = { lib, ... }: {
      # Disable the current quickshell-based shell.
      # lib.mkForce overrides the `enable = true` set in shells/noctalia/shell.nix.
      programs.noctalia-shell.enable = lib.mkForce false;

      # Enable Noctalia v5 and let systemd start it with the Wayland session.
      programs.noctalia = {
        enable = true;
        systemd.enable = true;

        # Minimal TOML config — v5 is alpha so only stable keys are set here.
        # All other options can be changed at runtime via v5's settings UI.
        settings = {
          theme = {
            mode = "dark";
            source = "builtin";
            builtin = "Catppuccin";
          };
          shell = {
            font = "JetBrainsMono Nerd Font";
          };
          wallpaper = {
            directory = "${homeDir}/Pictures/Wallpapers";
            enabled = true;
          };
        };
      };
    };
  };
}
