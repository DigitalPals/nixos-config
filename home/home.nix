# Home Manager configuration
{ config, pkgs, inputs, lib, osConfig, username, hostname, ... }:

let
  # Dynamically load all wallpapers from ../wallpapers directory
  wallpapersDir = ../wallpapers;
  wallpaperFiles = builtins.readDir wallpapersDir;
  wallpaperEntries = lib.mapAttrs' (name: _: {
    name = "Pictures/Wallpapers/${name}";
    value = { source = wallpapersDir + "/${name}"; };
  }) (lib.filterAttrs (name: type: type == "regular") wallpaperFiles);
  hostsWithMicMuteLed = [ "G1a" "proart" "xps" ];
  papirusAppIcon = name: "${pkgs.papirus-icon-theme}/share/icons/Papirus/64x64/apps/${name}.svg";
  devIcon = name: papirusAppIcon "distributor-logo-${name}";
  localSendIcon = "${pkgs.localsend}/share/icons/hicolor/256x256/apps/localsend.png";
in
{
  imports = [
    ./hyprland        # Modular Hyprland config (includes hypridle)
    ./voxtype.nix     # Push-to-talk dictation daemon + config
    ./ghostty.nix
    ./neovim.nix      # Neovim with LazyVim dependencies
    ./1password-secrets.nix  # 1Password SSH agent integration
    ./app-backup  # App profile backup/restore (browsers)
    # Noctalia Desktop Shell
    inputs.noctalia.homeModules.default
    ./shells/noctalia
  ];

  home.username = username;
  home.homeDirectory = "/home/${username}";
  xdg.enable = true;

  # System rebuilds and first-boot activation can run without a graphical user
  # bus. Do not let Home Manager fail activation just because user units cannot
  # be switched from that context; Hyprland autostart imports the session
  # environment and starts/restarts the needed user units after login.
  systemd.user.startServices = "suggest";

  # Let Home Manager manage itself
  programs.home-manager.enable = true;

  # Git configuration
  programs.git = {
    enable = true;
    signing.format = "openpgp";
    settings.user = {
      name = "John";
      email = "john@cybex.net";
    };
  };

  # XDG user directories
  xdg.userDirs = {
    enable = true;
    createDirectories = true;
    setSessionVariables = true;
    desktop = null;  # Don't create Desktop
    documents = "${config.home.homeDirectory}/Documents";
    download = "${config.home.homeDirectory}/Downloads";
    music = null;
    pictures = "${config.home.homeDirectory}/Pictures";
    publicShare = null;
    templates = null;
    videos = null;
    extraConfig = {
      CODE = "${config.home.homeDirectory}/Code";
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
    # LocalSend share helper
    ".local/bin/localsend-share" = {
      source = ./scripts/localsend-share;
      executable = true;
    };
    # Screen OCR script
    ".local/bin/screen-ocr" = {
      source = ./scripts/screen-ocr;
      executable = true;
    };
    # Clipboard image -> file helper (for CLI tools expecting file URLs)
    ".local/bin/clipboard-image-to-file" = {
      source = ./scripts/clipboard-image-to-file;
      executable = true;
    };
    # Open or create the default Fedora Distrobox development shell
    ".local/bin/dev-fedora-shell" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        set -euo pipefail

        box_name="dev-fedora"
        image="registry.fedoraproject.org/fedora-toolbox:latest"

        if ! /run/current-system/sw/bin/distrobox list | ${pkgs.gawk}/bin/awk -F'|' 'NR > 1 { gsub(/^[ \t]+|[ \t]+$/, "", $2); print $2 }' | ${pkgs.gnugrep}/bin/grep -Fxq "$box_name"; then
          /run/current-system/sw/bin/distrobox create --yes --name "$box_name" --image "$image"
        fi

        exec /run/current-system/sw/bin/distrobox enter --name "$box_name" -- bash -lc '
          printf "\033]0;dev-fedora\007"
          printf "Fedora Distrobox: dev-fedora\n"
          exec "''${SHELL:-/bin/bash}" -l
        '
      '';
    };
    # Open or create the default Arch Linux Distrobox development shell
    ".local/bin/dev-arch-shell" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        set -euo pipefail

        box_name="dev-arch"
        image="docker.io/library/archlinux:latest"

        if ! /run/current-system/sw/bin/distrobox list | ${pkgs.gawk}/bin/awk -F'|' 'NR > 1 { gsub(/^[ \t]+|[ \t]+$/, "", $2); print $2 }' | ${pkgs.gnugrep}/bin/grep -Fxq "$box_name"; then
          /run/current-system/sw/bin/distrobox create --yes --name "$box_name" --image "$image"
        fi

        exec /run/current-system/sw/bin/distrobox enter --name "$box_name" -- bash -lc '
          printf "\033]0;dev-arch\007"
          printf "Arch Distrobox: dev-arch\n"
          exec "''${SHELL:-/bin/bash}" -l
        '
      '';
    };
    # Open or create the default Debian Distrobox development shell
    ".local/bin/dev-debian-shell" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        set -euo pipefail

        box_name="dev-debian"
        image="docker.io/library/debian:latest"

        if ! /run/current-system/sw/bin/distrobox list | ${pkgs.gawk}/bin/awk -F'|' 'NR > 1 { gsub(/^[ \t]+|[ \t]+$/, "", $2); print $2 }' | ${pkgs.gnugrep}/bin/grep -Fxq "$box_name"; then
          /run/current-system/sw/bin/distrobox create --yes --name "$box_name" --image "$image"
        fi

        exec /run/current-system/sw/bin/distrobox enter --name "$box_name" -- bash -lc '
          printf "\033]0;dev-debian\007"
          printf "Debian Distrobox: dev-debian\n"
          exec "''${SHELL:-/bin/bash}" -l
        '
      '';
    };
    # Distrobox init hook shared by newly-created Fedora, Debian, Arch, etc. boxes.
    ".local/bin/dev-distrobox-init" = {
      executable = true;
      text = ''
        #!/usr/bin/env sh

        # Keep sudo frictionless inside dev containers, matching the host.
        if command -v sudo >/dev/null 2>&1 && [ -d /etc/sudoers.d ]; then
          printf '%s ALL=(ALL) NOPASSWD:ALL\n' ${username} > /etc/sudoers.d/99-${username}-nopasswd
          chmod 0440 /etc/sudoers.d/99-${username}-nopasswd
        fi

        marker=/etc/distrobox-dev-init-v2

        have_dev_tools() {
          for command in fish sudo curl ssh git git-lfs fzf eza zoxide gh nvim rg jq yq node npm python3 gcc g++ cmake make cargo btop fastfetch direnv unzip tar gzip xz find less; do
            command -v "$command" >/dev/null 2>&1 || return 1
          done

          { command -v fd >/dev/null 2>&1 || command -v fdfind >/dev/null 2>&1; } || return 1
          { command -v bat >/dev/null 2>&1 || command -v batcat >/dev/null 2>&1; } || return 1
          { command -v pip3 >/dev/null 2>&1 || command -v pip >/dev/null 2>&1; } || return 1
          return 0
        }

        mark_provisioned() {
          : > "$marker" 2>/dev/null || true
        }

        host_has_network() {
          if [ -x /run/host/run/current-system/sw/bin/nm-online ]; then
            /run/host/run/current-system/sw/bin/nm-online -q -t 2 >/dev/null 2>&1
            return $?
          fi

          return 0
        }

        if [ -f "$marker" ] || have_dev_tools; then
          mark_provisioned
          exit 0
        fi

        if ! host_has_network; then
          printf '%s\n' "dev-distrobox-init: offline; skipping package bootstrap for now."
          exit 0
        fi

        if command -v dnf >/dev/null 2>&1; then
          packages="fish sudo curl ca-certificates openssh-clients git git-lfs fzf eza zoxide gh neovim ripgrep fd-find bat jq yq nodejs npm python3-pip gcc gcc-c++ cmake make rustup cargo btop fastfetch direnv oh-my-posh unzip tar gzip xz findutils util-linux procps-ng less"
          dnf install -y ''${packages} || for package in ''${packages}; do dnf install -y "$package" || true; done
        elif command -v apt-get >/dev/null 2>&1; then
          packages="fish sudo curl ca-certificates openssh-client git git-lfs fzf eza zoxide gh neovim ripgrep fd-find bat jq yq nodejs npm python3-pip gcc g++ cmake make cargo btop fastfetch direnv unzip tar gzip xz-utils findutils util-linux procps less"
          apt-get update || true
          apt-get install -y ''${packages} || for package in ''${packages}; do apt-get install -y "$package" || true; done
        elif command -v pacman >/dev/null 2>&1; then
          packages="fish sudo curl ca-certificates openssh git git-lfs fzf eza zoxide github-cli neovim ripgrep fd bat jq yq nodejs npm python python-pip gcc cmake make rustup cargo btop fastfetch direnv oh-my-posh lazygit lazydocker unzip tar gzip xz findutils util-linux procps-ng less"
          pacman -Sy --needed --noconfirm ''${packages} || for package in ''${packages}; do pacman -Sy --needed --noconfirm "$package" || true; done
        elif command -v zypper >/dev/null 2>&1; then
          packages="fish sudo curl ca-certificates openssh git git-lfs fzf eza zoxide gh neovim ripgrep fd bat jq yq nodejs npm python3-pip gcc gcc-c++ cmake make cargo btop fastfetch direnv unzip tar gzip xz findutils util-linux procps less"
          zypper install -y ''${packages} || for package in ''${packages}; do zypper install -y "$package" || true; done
        elif command -v apk >/dev/null 2>&1; then
          packages="fish sudo curl ca-certificates openssh-client git git-lfs fzf eza zoxide github-cli neovim ripgrep fd bat jq yq nodejs npm py3-pip gcc g++ cmake make cargo btop fastfetch direnv unzip tar gzip xz findutils util-linux procps less"
          apk update || true
          apk add ''${packages} || for package in ''${packages}; do apk add "$package" || true; done
        fi

        if have_dev_tools; then
          mark_provisioned
        fi

        true
      '';
    };
    # Wrapper for Satty copy command (copies image + converts for CLI tools)
    ".local/bin/clipboard-copy-image" = {
      source = ./scripts/clipboard-copy-image;
      executable = true;
    };
    # NPX wrapper for ghui, matching Omarchy's lazy-install approach.
    ".local/bin/ghui" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.nodejs}/bin/npx --yes --prefer-online --package @kitlangton/ghui -- ghui "$@"
      '';
    };
    # TUI desktop launcher wrappers
    ".local/bin/tui-btop" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${pkgs.btop}/bin/btop
      '';
    };
    ".local/bin/tui-dust" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${pkgs.bash}/bin/bash -lc 'cd "$HOME"; ${pkgs.dust}/bin/dust -r; printf "\nPress any key to close..."; read -r -n 1 -s'
      '';
    };
    ".local/bin/tui-ghui" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${config.home.homeDirectory}/.local/bin/ghui
      '';
    };
    ".local/bin/tui-lazydocker" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${pkgs.lazydocker}/bin/lazydocker
      '';
    };
    ".local/bin/localsend-share-file" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${config.home.homeDirectory}/.local/bin/localsend-share file
      '';
    };
    ".local/bin/localsend-share-folder" = {
      executable = true;
      text = ''
        #!/usr/bin/env bash
        exec ${pkgs.ghostty}/bin/ghostty -e ${config.home.homeDirectory}/.local/bin/localsend-share folder
      '';
    };

    # User profile picture (used by GDM, SDDM, etc.)
    ".face".source = ../face;

    # npm config for global packages (avoids permission issues)
    ".npmrc".text = ''
      prefix=''${HOME}/.npm-global
    '';
  };

  xdg.configFile."distrobox/distrobox.conf".text = ''
    container_init_hook="${config.home.homeDirectory}/.local/bin/dev-distrobox-init"
  '';

  # Distrobox can export Terminal=true desktop files for these boxes. Noctalia
  # launches those through its terminalCommand, so stale exports can point at a
  # missing terminal or old distrobox store path. Replace them declaratively.
  home.activation.removeStaleDistroboxDevDesktopEntries =
    lib.hm.dag.entryBefore [ "checkLinkTargets" ] ''
      $DRY_RUN_CMD ${pkgs.coreutils}/bin/rm -f \
        "$HOME/.local/share/applications/dev-fedora.desktop" \
        "$HOME/.local/share/applications/dev-arch.desktop" \
        "$HOME/.local/share/applications/dev-debian.desktop"
    '';

  xdg.dataFile."nautilus-python/extensions/localsend.py".source = ./nautilus-localsend.py;

  # Desktop entry overrides for Wayland
  xdg.desktopEntries."1password" = {
    name = "1Password";
    exec = "1password --enable-features=UseOzonePlatform,WaylandWindowDecorations --ozone-platform=wayland %U";
    icon = "1password";
    comment = "Password Manager";
    categories = [ "Office" "Security" ];
  };


  # Neovim wrapper that launches in Ghostty terminal
  xdg.desktopEntries.nvim-ghostty = {
    name = "Neovim";
    exec = "ghostty -e nvim %F";
    icon = "nvim";
    comment = "Edit text files in Neovim";
    categories = [ "Utility" "TextEditor" ];
    mimeType = [
      "text/plain"
      "text/x-csrc"
      "text/x-chdr"
      "text/x-c++src"
      "text/x-c++hdr"
      "text/x-java"
      "text/x-python"
      "text/x-shellscript"
      "application/json"
      "application/x-yaml"
      "application/xml"
      "text/markdown"
    ];
  };

  xdg.desktopEntries."dev-fedora" = {
    name = "Dev Fedora";
    exec = "ghostty -e ${config.home.homeDirectory}/.local/bin/dev-fedora-shell";
    icon = devIcon "fedora";
    comment = "Open the Fedora Distrobox development shell";
    categories = [ "Development" "System" ];
  };

  xdg.desktopEntries."dev-arch" = {
    name = "Dev Arch";
    exec = "ghostty -e ${config.home.homeDirectory}/.local/bin/dev-arch-shell";
    icon = devIcon "archlinux";
    comment = "Open the Arch Distrobox development shell";
    categories = [ "Development" "System" ];
  };

  xdg.desktopEntries."dev-debian" = {
    name = "Dev Debian";
    exec = "ghostty -e ${config.home.homeDirectory}/.local/bin/dev-debian-shell";
    icon = devIcon "debian";
    comment = "Open the Debian Distrobox development shell";
    categories = [ "Development" "System" ];
  };

  xdg.desktopEntries."tui-btop" = {
    name = "System Monitor";
    exec = "${config.home.homeDirectory}/.local/bin/tui-btop";
    icon = "${pkgs.btop}/share/icons/hicolor/scalable/apps/btop.svg";
    comment = "Open btop in Ghostty";
    categories = [ "System" "Monitor" ];
  };

  xdg.desktopEntries."tui-disk-usage" = {
    name = "Disk Usage";
    exec = "${config.home.homeDirectory}/.local/bin/tui-dust";
    icon = papirusAppIcon "disk-usage-analyzer";
    comment = "Open dust in Ghostty";
    categories = [ "System" "Utility" ];
  };

  xdg.desktopEntries."tui-ghui" = {
    name = "GitHub";
    exec = "${config.home.homeDirectory}/.local/bin/tui-ghui";
    icon = papirusAppIcon "github";
    comment = "Open ghui in Ghostty";
    categories = [ "Development" ];
  };

  xdg.desktopEntries."tui-lazydocker" = {
    name = "Docker";
    exec = "${config.home.homeDirectory}/.local/bin/tui-lazydocker";
    icon = papirusAppIcon "docker-desktop";
    comment = "Open lazydocker in Ghostty";
    categories = [ "Development" "System" ];
  };

  xdg.desktopEntries."localsend-share-clipboard" = {
    name = "Share Clipboard";
    exec = "${config.home.homeDirectory}/.local/bin/localsend-share clipboard";
    icon = localSendIcon;
    comment = "Share clipboard text with LocalSend";
    categories = [ "Utility" ];
  };

  xdg.desktopEntries."localsend-share-file" = {
    name = "Share File";
    exec = "${config.home.homeDirectory}/.local/bin/localsend-share-file";
    icon = localSendIcon;
    comment = "Pick files to share with LocalSend";
    categories = [ "Utility" ];
  };

  xdg.desktopEntries."localsend-share-folder" = {
    name = "Share Folder";
    exec = "${config.home.homeDirectory}/.local/bin/localsend-share-folder";
    icon = localSendIcon;
    comment = "Pick a folder to share with LocalSend";
    categories = [ "Utility" ];
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
    tesseract

    # File management
    nautilus
    nautilus-python
    sushi # Quick preview for Nautilus (press SPACE)

    # Theming
    nwg-look

    # Polkit agent: badged supports fingerprint, hyprpolkitagent is password-only
    (if osConfig.services.fprintd.enable
     then pkgs.callPackage ../packages/badged {}
     else pkgs.hyprpolkitagent)

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
    lazygit
    ripgrep
    fd
    dust

    # CLI enhancements
    bat              # cat with syntax highlighting
    eza              # modern ls with tree view
    tree             # directory structure
    zip              # create zip archives
    yq-go            # jq for YAML/TOML
    delta            # better git diffs

    # Development
    python3          # scripting, AI agent helpers

    # System diagnostics (helps AI agents)
    pciutils         # lspci - PCI devices
    usbutils         # lsusb - USB devices
    file             # determine file types
    duf              # modern df (disk usage)
    strace           # trace syscalls
    nix-tree         # visualize nix derivations
    net-tools        # ifconfig, netstat, etc.

    # Media
    ffmpeg           # audio/video processing
    mpv-unwrapped    # video player
    yt-dlp-light     # cached video URL helper for mpv
    imv              # image viewer
    pinta            # image editor

    # Productivity
    evince           # document/PDF viewer
    file-roller      # archive manager (Nautilus integration)
    localsend        # local file sharing
    libreoffice-fresh  # office suite (using fresh variant to work around nixpkgs#495219)

    # Fonts
    font-awesome
    noto-fonts
    noto-fonts-color-emoji
    nerd-fonts.jetbrains-mono
    nerd-fonts.fira-code
  ];

  # Web browsers
  programs.google-chrome = {
    enable = true;
    commandLineArgs = [
      # Enable trackpad swipe gestures for back/forward navigation
      "--enable-features=TouchpadOverscrollHistoryNavigation"
    ] ++ lib.optionals (hostname == "G1a") [
      # Stability workaround (G1a / Strix Halo + amdgpu):
      # We've seen GPU hangs/page-faults in the amdgpu gfx ring attributed to Chrome's GPU process,
      # which then causes Hyprland to crash when its GL context becomes unusable after a GPU reset.
      # Disabling VA-API (hw video decode/encode) avoids a common trigger path.
      "--disable-features=VaapiVideoDecodeLinuxGL,VaapiVideoEncoder"
    ] ++ [
      # Suppress "Chrome didn't shut down correctly" crash restore bubble
      "--hide-crash-restore-bubble"
    ];
  };

  programs.firefox = {
    enable = true;
    # Keep the existing profile location explicit. Home Manager 26.05 changes
    # the default to the XDG config directory and warns for older state versions.
    configPath = ".mozilla/firefox";
  };

  # Direnv - auto-activate nix develop shells when entering directories
  # Add `.envrc` with `use flake` to your Rust projects
  programs.direnv = {
    enable = true;
    nix-direnv.enable = true;  # Caches dev shell evaluation
  };

  # App profile backup/restore (browsers - encrypted, synced via GitHub)
  # Age uses a local key with 1Password fallback. SSH can be materialized locally
  # for tools that expect ~/.ssh/id_ed25519, with 1Password as the source.
  programs.app-backup = {
    enable = true;
    ageRecipient = "age160gkdyge3henu4r643066rnkwnfqc4xhzx47tprcmqj9lxcr9cuqvvw4qu";
    # Age key - for encrypting/decrypting app backups
    ageKey1Password = "op://Private/age-key/private-key";
    ageKeyPath = "~/.config/age/key.txt";
    # SSH key - retrieved from 1Password by `forge keys setup` when missing
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

      # Text files (Neovim in Ghostty)
      "text/plain" = "nvim-ghostty.desktop";
      "application/json" = "nvim-ghostty.desktop";
      "application/x-yaml" = "nvim-ghostty.desktop";
      "application/xml" = "nvim-ghostty.desktop";
      "text/markdown" = "nvim-ghostty.desktop";
      "text/x-python" = "nvim-ghostty.desktop";
      "text/x-shellscript" = "nvim-ghostty.desktop";
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

  # Install opencode via npm so `opencode upgrade --method npm` can keep it current
  home.activation.installOpencodeCLI = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    if [ -L "$HOME/.npm-global/bin/opencode" ] && [ ! -e "$HOME/.npm-global/bin/opencode" ]; then
      $DRY_RUN_CMD ${pkgs.coreutils}/bin/rm -f "$HOME/.npm-global/bin/opencode"
    fi

    if [ ! -x "$HOME/.npm-global/bin/opencode" ]; then
      # Use 3 second timeout for connectivity check
      if ${pkgs.curl}/bin/curl -m 3 -fsSL https://registry.npmjs.org/ >/dev/null 2>&1; then
        $DRY_RUN_CMD ${pkgs.nodejs}/bin/npm --prefix "$HOME/.npm-global" install -g opencode-ai || \
          echo "opencode install failed (offline or npm issue)"
      else
        echo "opencode install skipped (offline)"
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
    XDG_SESSION_TYPE = "wayland";
    NOCTALIA_PAM_SERVICE = "noctalia";
  };

  # === Mic mute LED sync service ===
  # The kernel's audio-micmute LED trigger doesn't sync with WirePlumber/PipeWire.
  # This service polls the mic mute state and updates the LED accordingly.
  systemd.user.services.mic-led-sync = lib.mkIf (builtins.elem osConfig.networking.hostName hostsWithMicMuteLed) {
    Unit = {
      Description = "Sync mic mute LED with WirePlumber state";
      After = [ "pipewire.service" "wireplumber.service" ];
      PartOf = [ "graphical-session.target" ];
    };
    Service = {
      Type = "simple";
      ExecStart = pkgs.writeShellScript "mic-led-sync" ''
        LED_PATH=""

        # Wait for LED interface to be available
        while [ -z "$LED_PATH" ]; do
          for candidate in /sys/class/leds/*::micmute/brightness; do
            if [ -w "$candidate" ]; then
              LED_PATH="$candidate"
              break
            fi
          done
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
