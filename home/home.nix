# Home Manager configuration
{ config, pkgs, inputs, lib, osConfig, username, portal, ... }:

let
  # Get shell from NixOS config (set by specialisations)
  shell = osConfig.desktop.shell;

  # Dynamically load all wallpapers from ../wallpapers directory
  wallpapersDir = ../wallpapers;
  wallpaperFiles = builtins.readDir wallpapersDir;
  wallpaperEntries = lib.mapAttrs' (name: _: {
    name = "Pictures/Wallpapers/${name}";
    value = { source = wallpapersDir + "/${name}"; };
  }) (lib.filterAttrs (name: type: type == "regular") wallpaperFiles);
in
{
  imports = [
    ./hyprland        # Modular Hyprland config (includes hypridle)
    ./ghostty.nix
    ./neovim.nix      # Neovim with LazyVim dependencies
    ./1password-secrets.nix  # 1Password SSH agent integration
    ./app-backup  # App profile backup/restore (browsers)
    ./forge-notify.nix  # Background update checker
    # Always deploy Illogical Impulse dotfiles (Quickshell config)
    # Required because Home Manager evaluates with default shell at build time,
    # but specialisations need these files at boot time. See CLAUDE.md.
    ./shells/illogical/dotfiles-only.nix
  ] ++ (if shell == "illogical" then [
    ./shells/illogical
  ] else [
    inputs.noctalia.homeModules.default
    ./shells/noctalia
  ]);

  home.username = username;
  home.homeDirectory = "/home/${username}";

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

  # D-Bus service for Nautilus quick preview (sushi)
  xdg.dataFile."dbus-1/services/org.gnome.NautilusPreviewer.service".source =
    "${pkgs.sushi}/share/dbus-1/services/org.gnome.NautilusPreviewer.service";

  # Home file entries (merged with wallpapers)
  home.file = wallpaperEntries // {
    # Ensure custom directories exist
    "Code/.keep".text = "";
    "Pictures/Screenshots/.keep".text = "";

    # Screenshot script
    ".local/bin/screenshot" = {
      source = ./scripts/screenshot;
      executable = true;
    };
    # Clipboard image -> file helper (for CLI tools expecting file URLs)
    ".local/bin/clipboard-image-to-file" = {
      source = ./scripts/clipboard-image-to-file;
      executable = true;
    };
    # Wrapper for Satty copy command (copies image + converts for CLI tools)
    ".local/bin/clipboard-copy-image" = {
      source = ./scripts/clipboard-copy-image;
      executable = true;
    };

    # User profile picture (used by GDM, SDDM, etc.)
    ".face".source = ../face;

    # npm config for global packages (avoids permission issues)
    ".npmrc".text = ''
      prefix=''${HOME}/.npm-global
    '';
  };

  # Desktop entry overrides for Wayland
  xdg.desktopEntries."1password" = {
    name = "1Password";
    exec = "1password --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform=wayland %U";
    icon = "1password";
    comment = "Password Manager";
    categories = [ "Office" "Security" ];
  };

  xdg.desktopEntries.onlyoffice-desktopeditors = {
    name = "OnlyOffice Desktop Editors";
    exec = "onlyoffice-desktopeditors --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform=wayland %U";
    icon = "onlyoffice-desktopeditors";
    comment = "Office productivity suite";
    categories = [ "Office" ];
    mimeType = [
      "application/vnd.oasis.opendocument.text"
      "application/vnd.oasis.opendocument.spreadsheet"
      "application/vnd.oasis.opendocument.presentation"
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
      "application/vnd.openxmlformats-officedocument.presentationml.presentation"
      "application/msword"
      "application/vnd.ms-excel"
      "application/vnd.ms-powerpoint"
    ];
  };

  # User packages
  home.packages = with pkgs; [
    # XDG portal for GTK apps (dark mode, file dialogs)
    xdg-desktop-portal-gtk

    # Screenshot tools
    grim
    slurp
    satty
    wayfreeze
    wl-clipboard
    hyprpicker

    # File management
    nautilus
    sushi # Quick preview for Nautilus (press SPACE)

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
    portal.packages.${pkgs.system}.default  # SSH client
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
    onlyoffice-desktopeditors  # office suite

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

  # Direnv - auto-activate nix develop shells when entering directories
  # Add `.envrc` with `use flake` to your Rust projects
  programs.direnv = {
    enable = true;
    nix-direnv.enable = true;  # Caches dev shell evaluation
  };

  # App profile backup/restore (browsers - encrypted, synced via GitHub)
  # Keys are stored locally with 1Password as fallback
  programs.app-backup = {
    enable = true;
    ageRecipient = "age160gkdyge3henu4r643066rnkwnfqc4xhzx47tprcmqj9lxcr9cuqvvw4qu";
    # Age key - for encrypting/decrypting app backups
    ageKey1Password = "op://Private/age-key/private-key";
    ageKeyPath = "~/.config/age/key.txt";
    # SSH key - for GitHub authentication
    sshKey1Password = "op://Private/kuhnsbkyjjmpjtvgpeiqqlczeu/private key";
    sshKeyPath = "~/.ssh/id_ed25519";
  };

  # Default applications
  xdg.mimeApps = {
    enable = true;
    defaultApplications = {
      # Browser
      "text/html" = "google-chrome.desktop";
      "x-scheme-handler/http" = "google-chrome.desktop";
      "x-scheme-handler/https" = "google-chrome.desktop";
      "x-scheme-handler/about" = "google-chrome.desktop";
      "x-scheme-handler/unknown" = "google-chrome.desktop";

      # Images (imv)
      "image/png" = "imv.desktop";
      "image/jpeg" = "imv.desktop";
      "image/gif" = "imv.desktop";
      "image/webp" = "imv.desktop";
      "image/bmp" = "imv.desktop";
      "image/tiff" = "imv.desktop";

      # PDF (Evince)
      "application/pdf" = "org.gnome.Evince.desktop";

      # Videos (mpv)
      "video/mp4" = "mpv.desktop";
      "video/x-matroska" = "mpv.desktop";
      "video/webm" = "mpv.desktop";
      "video/x-msvideo" = "mpv.desktop";
      "video/quicktime" = "mpv.desktop";
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
      # Use 3 second timeout for connectivity check
      if ${pkgs.curl}/bin/curl -m 3 -fsSL https://claude.ai/install.sh >/dev/null 2>&1; then
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
      # Use 3 second timeout for connectivity check
      if ${pkgs.curl}/bin/curl -m 3 -fsSL https://registry.npmjs.org/ >/dev/null 2>&1; then
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

  # === Mic mute LED sync service (G1a only) ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This service polls the mic mute state and updates the LED accordingly.
  systemd.user.services.mic-led-sync = lib.mkIf (osConfig.networking.hostName == "G1a") {
    Unit = {
      Description = "Sync mic mute LED with WirePlumber state";
      After = [ "pipewire.service" "wireplumber.service" ];
      PartOf = [ "graphical-session.target" ];
    };
    Service = {
      Type = "simple";
      ExecStart = pkgs.writeShellScript "mic-led-sync" ''
        LED_PATH="/sys/class/leds/hda::micmute/brightness"

        # Wait for LED interface to be available
        while [ ! -w "$LED_PATH" ]; do
          sleep 1
        done

        # Sync loop
        while true; do
          if ${pkgs.wireplumber}/bin/wpctl get-volume @DEFAULT_AUDIO_SOURCE@ 2>/dev/null | grep -q MUTED; then
            echo 1 > "$LED_PATH" 2>/dev/null || true
          else
            echo 0 > "$LED_PATH" 2>/dev/null || true
          fi
          sleep 0.3
        done
      '';
      Restart = "always";
      RestartSec = 5;
    };
    Install = {
      WantedBy = [ "graphical-session.target" ];
    };
  };

  # State version (should match NixOS)
  home.stateVersion = "24.11";
}
