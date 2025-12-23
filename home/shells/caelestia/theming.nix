# Theming configuration for Caelestia
# GTK theme, cursor, and icons
{ config, pkgs, lib, ... }:

{
  # Cursor theme (Bibata-Modern-Classic - sweet-cursors not in nixpkgs)
  home.pointerCursor = {
    name = "Bibata-Modern-Classic";
    package = pkgs.bibata-cursors;
    size = 24;
    gtk.enable = true;
    x11.enable = true;
  };

  # GTK theming
  gtk = {
    enable = true;
    theme = {
      name = "adw-gtk3-dark";
      package = pkgs.adw-gtk3;
    };
    iconTheme = {
      name = "Papirus-Dark";
      package = pkgs.papirus-icon-theme;
    };
  };

  # Force GTK config files to avoid backup conflicts when switching shells
  xdg.configFile."gtk-4.0/gtk.css".force = true;

  # Icon packages (fallbacks)
  home.packages = with pkgs; [
    papirus-icon-theme
    adwaita-icon-theme
    hicolor-icon-theme
  ];
}
