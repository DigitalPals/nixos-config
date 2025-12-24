# Home Manager configuration for john
{ config, pkgs, inputs, lib, osConfig, ... }:

let
  # Get shell from NixOS config (set by specialisations)
  shell = osConfig.desktop.shell;
in
{
  imports = [
    ./hyprland        # Modular Hyprland config (includes hypridle)
    ./ghostty.nix
    ./neovim.nix      # Neovim with LazyVim dependencies
    ./1password-secrets.nix  # 1Password SSH agent integration
    ./browser-backup  # Browser profile backup/restore (disabled by default)
  ] ++ (if shell == "illogical" then [
    ./shells/illogical
  ] else if shell == "caelestia" then [
    inputs.caelestia.homeManagerModules.default
    ./shells/caelestia
  ] else [
    inputs.noctalia.homeModules.default
    ./shells/noctalia
  ]);

  home.username = "john";
  home.homeDirectory = "/home/john";

  # Let Home Manager manage itself
  programs.home-manager.enable = true;

  # Git configuration
  programs.git = {
    enable = true;
    settings.user = {
      name = "John";
      email = "john@cybex.net";
    };
  };

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

  # User profile picture (used by GDM, SDDM, etc.)
  home.file.".face".source = ../face;

  # npm config for global packages (avoids permission issues)
  home.file.".npmrc".text = ''
    prefix=''${HOME}/.npm-global
  '';

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
    # Portal for GTK apps (dark mode, file dialogs)
    xdg-desktop-portal-gtk

    # Screenshot tools
    grim
    slurp
    swappy

    # File management
    nautilus

    # Theming
    nwg-look

    # Media control
    brightnessctl
    playerctl

    # Applications
    spotify
    lazydocker
    btop
    gnome-calculator
    gnome-text-editor
    fastfetch
    jq
    nodejs
    termius
    lazygit
    ripgrep
    fd

    # CLI enhancements
    bat              # cat with syntax highlighting

    # Media
    mpv              # video player
    imv              # image viewer
    pinta            # image editor

    # Productivity
    evince           # document/PDF viewer
    localsend        # local file sharing

    # Fonts
    font-awesome
    noto-fonts
    noto-fonts-color-emoji
    nerd-fonts.jetbrains-mono
    nerd-fonts.fira-code
  ];

  # Web browsers
  programs.google-chrome.enable = true;

  programs.firefox.enable = true;

  # Browser profile backup/restore (encrypted, synced via GitHub)
  programs.browser-backup = {
    enable = true;
    ageRecipient = "age160gkdyge3henu4r643066rnkwnfqc4xhzx47tprcmqj9lxcr9cuqvvw4qu";
    ageKey1Password = "op://Private/age-key/private-key";
  };

  # Set Google Chrome as default browser
  xdg.mimeApps = {
    enable = true;
    defaultApplications = {
      "text/html" = "google-chrome.desktop";
      "x-scheme-handler/http" = "google-chrome.desktop";
      "x-scheme-handler/https" = "google-chrome.desktop";
      "x-scheme-handler/about" = "google-chrome.desktop";
      "x-scheme-handler/unknown" = "google-chrome.desktop";
    };
  };


  # Add npm global bin and Claude Code to PATH
  home.sessionPath = [
    "$HOME/.npm-global/bin"
    "$HOME/.local/bin"
  ];

  # Install Claude Code native binary if not present
  home.activation.installClaudeCode = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    if [ ! -x "$HOME/.local/bin/claude" ]; then
      if ${pkgs.curl}/bin/curl -fsSL https://claude.ai/ >/dev/null 2>&1; then
        PATH="${pkgs.curl}/bin:${pkgs.coreutils}/bin:${pkgs.gnutar}/bin:${pkgs.gzip}/bin:$PATH" \
          $DRY_RUN_CMD ${pkgs.bash}/bin/bash -c "curl -fsSL https://claude.ai/install.sh | bash" || \
          echo "Claude Code install failed (offline or installer issue)"
      else
        echo "Claude Code install skipped (offline)"
      fi
    fi
  '';

  # Install OpenAI Codex CLI via npm if not present
  home.activation.installCodexCLI = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    if [ ! -x "$HOME/.npm-global/bin/codex" ]; then
      if ${pkgs.curl}/bin/curl -fsSL https://registry.npmjs.org/ >/dev/null 2>&1; then
        $DRY_RUN_CMD ${pkgs.nodejs}/bin/npm install -g @openai/codex || \
          echo "Codex CLI install failed (offline or npm issue)"
      else
        echo "Codex CLI install skipped (offline)"
      fi
    fi
  '';

  # GTK theme settings (affects Nautilus and other GTK apps)
  dconf.settings = {
    "org/gnome/desktop/interface" = {
      color-scheme = "prefer-dark";
    };
  };

  # Environment variables
  home.sessionVariables = {
    EDITOR = "nvim";
    BROWSER = "google-chrome-stable";
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
