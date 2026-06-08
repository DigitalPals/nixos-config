# Noctalia v5 desktop shell configuration.
{ config, inputs, ... }:

let
  username = config.forge.installer.username;
  homeDir = "/home/${username}";
in
{
  home-manager.sharedModules = [
    inputs.noctalia.homeModules.default
  ];

  home-manager.users.${username} = {
    programs.noctalia = {
      enable = true;
      systemd.enable = true;

      # Minimal TOML config: v5 is alpha, so only stable keys are set here.
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
}
