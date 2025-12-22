# Home Manager configuration for john
{ config, pkgs, inputs, lib, ... }:

{
  imports = [
    ./hyprland        # Modular Hyprland config (includes hypridle)
    ./fish.nix
    ./noctalia.nix
    ./ghostty.nix
  ];

  home.username = "john";
  home.homeDirectory = "/home/john";

  # Let Home Manager manage itself
  programs.home-manager.enable = true;

  # XDG user directories
  xdg.userDirs = {
    enable = true;
    createDirectories = true;
    desktop = null;  # Don't create Desktop
    documents = "${config.home.homeDirectory}/Documents";
    download = "${config.home.homeDirectory}/Downloads";
    music = null;
    pictures = "${config.home.homeDirectory}/Pictures";
    publicShare = null;
    templates = null;
    videos = null;
    extraConfig = {
      XDG_CODE_DIR = "${config.home.homeDirectory}/Code";
    };
  };

  # Ensure custom directories exist
  home.file."Code/.keep".text = "";

  # Desktop entry overrides for Wayland
  xdg.desktopEntries.termius-app = {
    name = "Termius";
    exec = "termius-app --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform=wayland %U";
    icon = "termius-app";
    comment = "SSH platform for Mobile and Desktop";
    categories = [ "Network" "Security" ];
    mimeType = [ "x-scheme-handler/termius" "x-scheme-handler/ssh" ];
  };

  xdg.desktopEntries."1password" = {
    name = "1Password";
    exec = "1password --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform=wayland %U";
    icon = "1password";
    comment = "Password Manager";
    categories = [ "Office" "Security" ];
  };

  # Wallpapers
  home.file."Pictures/Wallpapers/01-black-widow-warrior-with-katana-ks.jpg".source = ../wallpapers/01-black-widow-warrior-with-katana-ks.jpg;
  home.file."Pictures/Wallpapers/05-joker-chaos-in-a-purple-suit-nq.jpg".source = ../wallpapers/05-joker-chaos-in-a-purple-suit-nq.jpg;

  # User packages
  home.packages = with pkgs; [
    # Quickshell (for Noctalia IPC commands)
    quickshell

    # Screenshot tools
    grim
    slurp
    swappy

    # File management
    nautilus

    # Theming
    nwg-look

    # Browser
    chromium

    # Media control
    brightnessctl
    playerctl

    # Applications
    signal-desktop
    obsidian
    spotify
    lazydocker
    btop
    gnome-calculator
    jq
    nodejs
    termius

    # Fonts
    font-awesome
    noto-fonts
    noto-fonts-color-emoji
    nerd-fonts.jetbrains-mono
    nerd-fonts.fira-code
  ];

  # GTK theming
  gtk = {
    enable = true;
    theme = {
      name = "Adwaita-dark";
      package = pkgs.gnome-themes-extra;
    };
    iconTheme = {
      name = "Adwaita";
      package = pkgs.adwaita-icon-theme;
    };
  };

  # Cursor theme
  home.pointerCursor = {
    name = "Adwaita";
    package = pkgs.adwaita-icon-theme;
    size = 24;
    gtk.enable = true;
  };

  # Add npm global bin and Claude Code to PATH
  home.sessionPath = [
    "$HOME/.npm-global/bin"
    "$HOME/.local/bin"
  ];

  # Install Claude Code native binary if not present
  home.activation.installClaudeCode = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    if [ ! -x "$HOME/.local/bin/claude" ]; then
      PATH="${pkgs.curl}/bin:${pkgs.coreutils}/bin:${pkgs.gnutar}/bin:${pkgs.gzip}/bin:$PATH" \
        $DRY_RUN_CMD ${pkgs.bash}/bin/bash -c "curl -fsSL https://claude.ai/install.sh | bash"
    fi
  '';

  # Environment variables
  home.sessionVariables = {
    EDITOR = "nano";
    BROWSER = "chromium";
    TERMINAL = "ghostty";

    # Wayland-specific (NIXOS_OZONE_WL is set in configuration.nix)
    MOZ_ENABLE_WAYLAND = "1";
    QT_QPA_PLATFORM = "wayland";
    SDL_VIDEODRIVER = "wayland";
    XDG_SESSION_TYPE = "wayland";
  };

  # State version (should match NixOS)
  home.stateVersion = "24.11";
}
