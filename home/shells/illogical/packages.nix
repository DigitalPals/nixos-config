# Packages required for Illogical Impulse
{ config, pkgs, lib, ... }:

let
  # Python environment for wallpaper color extraction
  pythonEnv = pkgs.python3.withPackages (ps: with ps; [
    pillow
    numpy
  ]);
in
{
  home.packages = with pkgs; [
    # Quickshell
    quickshell

    # Qt theming
    kdePackages.qt6ct
    libsForQt5.qtstyleplugin-kvantum
    kdePackages.qtstyleplugin-kvantum

    # Audio visualization
    cava

    # Wayland utilities
    hyprlock
    wl-clipboard
    cliphist
    hyprsunset

    # Screenshot and recording
    hyprshot
    wf-recorder
    imagemagick
    ffmpeg

    # Brightness control
    ddcutil

    # Launchers
    fuzzel
    wlogout

    # Python environment for color extraction
    pythonEnv

    # Fonts for Material Design
    material-symbols
    rubik
    nerd-fonts.ubuntu
    nerd-fonts.ubuntu-mono
    nerd-fonts.caskaydia-cove
    nerd-fonts.fantasque-sans-mono
    nerd-fonts.mononoki
    nerd-fonts.space-mono
  ];

  # Qt/KDE environment configuration
  qt = {
    enable = true;
    platformTheme.name = "qt6ct";
    style.name = "kvantum";
  };
}
