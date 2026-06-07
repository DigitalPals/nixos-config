# Noctalia v5 as the default shell
#
# Noctalia v5 (early alpha) is the DEFAULT boot configuration on every host.
# A "noctalia-stable" entry is added to the boot menu; booting that entry runs
# the previous quickshell-based stable shell instead.
#
# Why this is structured as an inversion rather than a v5 specialisation:
# the Limine bootloader always boots the base (top-level) configuration by
# default and only ever exposes specialisations as *extra* menu entries.
# To make v5 the default boot option it must therefore be the base config,
# with stable demoted to the specialisation.
#
# Known limitations while running v5 (now the default):
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
  # --- Default configuration: Noctalia v5 -----------------------------------

  # Make the v5 Home Manager module available to all users.
  home-manager.sharedModules = [
    inputs.noctalia-v5.homeModules.default
  ];

  # Per-user configuration for the default (v5) boot entry.
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

  # --- "noctalia-stable" boot entry: the previous stable shell --------------

  specialisation.noctalia-stable.configuration = {
    home-manager.users.${username} = { lib, ... }: {
      # Turn Noctalia v5 back off and restore the quickshell stable shell.
      # mkOverride 40 outranks the base config's mkForce (priority 50) so these
      # win over the default (v5) definitions the specialisation inherits.
      programs.noctalia.enable = lib.mkOverride 40 false;
      programs.noctalia.systemd.enable = lib.mkOverride 40 false;
      programs.noctalia-shell.enable = lib.mkOverride 40 true;
    };
  };
}
