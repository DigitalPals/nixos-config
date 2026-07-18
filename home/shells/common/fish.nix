# Fish shell configuration
# Custom prompt theme and shell tools
{ config, pkgs, lib, ... }:

let
  ohMyPoshConfigHash = builtins.hashFile "sha256" ./EDM115-newline2.omp.json;
  ohMyPoshCacheHash = builtins.hashString "sha256" "${toString pkgs.oh-my-posh}:${ohMyPoshConfigHash}";
in
{
  programs.fish = {
    enable = true;

    # Interactive shell initialization
    interactiveShellInit = ''
      # Disable greeting
      set -g fish_greeting

      # Set cursor to underscore
      set -g fish_cursor_default underscore
      set -g fish_cursor_insert underscore
      set -g fish_cursor_replace_one underscore
      set -g fish_cursor_visual underscore

      # Add ~/.local/bin to PATH if not already present
      if not contains ~/.local/bin $PATH
        set -gx PATH ~/.local/bin $PATH
      end

      # Add ~/.kimi-code/bin (Kimi Code CLI) to PATH if not already present
      if not contains ~/.kimi-code/bin $PATH
        set -gx PATH ~/.kimi-code/bin $PATH
      end

      # VISUAL for programs that distinguish from EDITOR
      set -gx VISUAL nvim

      # Oh My Posh runs from the host Nix store even inside Distrobox. Derive
      # the prompt OS icon from the active environment's /etc/os-release.
      set -l posh_os_id (sh -c '. /etc/os-release 2>/dev/null; printf "%s" "$ID"' 2>/dev/null)
      set -l posh_os_like (sh -c '. /etc/os-release 2>/dev/null; printf "%s" "$ID_LIKE"' 2>/dev/null)
      switch $posh_os_id
        case nixos
          set -gx POSH_OS_ICON ""
        case fedora
          set -gx POSH_OS_ICON ""
        case debian
          set -gx POSH_OS_ICON ""
        case ubuntu
          set -gx POSH_OS_ICON ""
        case arch
          set -gx POSH_OS_ICON ""
        case alpine
          set -gx POSH_OS_ICON ""
        case opensuse-tumbleweed opensuse-leap opensuse
          set -gx POSH_OS_ICON ""
        case '*'
          if string match -q "*debian*" $posh_os_like
            set -gx POSH_OS_ICON ""
          else if string match -q "*rhel*" $posh_os_like; or string match -q "*fedora*" $posh_os_like
            set -gx POSH_OS_ICON ""
          else if string match -q "*arch*" $posh_os_like
            set -gx POSH_OS_ICON ""
          else
            set -gx POSH_OS_ICON ""
          end
      end

      # Initialize oh-my-posh prompt
      ${pkgs.oh-my-posh}/bin/oh-my-posh init fish --config ~/.config/oh-my-posh/EDM115-newline2.omp.json | source
    '';

    # Shell aliases
    shellAliases = {
      # eza for better ls
      ls = "eza --icons";

      # Nix shortcuts (auto-detects hostname from flake)
      rebuild = "sudo nixos-rebuild switch --flake /etc/nixos";
      rebuild-test = "sudo nixos-rebuild test --flake /etc/nixos";
      rebuild-boot = "sudo nixos-rebuild boot --flake /etc/nixos";
      update = "nix flake update /etc/nixos";

      # Common shortcuts
      ll = "ls -la";
      la = "ls -A";
      l = "ls -CF";

      # Navigation
      ".." = "cd ..";
      "..." = "cd ../..";

      # Git shortcuts
      gs = "git status";
      ga = "git add";
      gc = "git commit";
      gp = "git push";
      gl = "git log --oneline";
      lg = "lazygit";

      # Hyprland shortcuts
      hypr-reload = "hyprctl reload";
      hypr-monitors = "hyprctl monitors";
      hypr-workspaces = "hyprctl workspaces";

      # System info
      fastfetch = "command fastfetch";
    };

    # Fish functions
    functions = {
      nixedit = {
        body = ''
          cd /etc/nixos
          $EDITOR .
        '';
        description = "Open NixOS configuration in editor";
      };

      nixgc = {
        body = ''
          echo "Removing old generations..."
          sudo nix-collect-garbage -d
          echo "Optimizing store..."
          nix store optimise
        '';
        description = "Clean up Nix store";
      };

      nixgen = {
        body = ''
          sudo nix-env --list-generations --profile /nix/var/nix/profiles/system
        '';
        description = "List NixOS generations";
      };
    };

    # Fish plugins
    plugins = [
      {
        name = "colored-man-pages";
        src = pkgs.fishPlugins.colored-man-pages.src;
      }
    ];
  };

  # Disable Starship (using oh-my-posh instead)
  programs.starship.enable = false;

  # oh-my-posh theme file
  xdg.configFile."oh-my-posh/EDM115-newline2.omp.json" = {
    source = ./EDM115-newline2.omp.json;
  };

  xdg.configFile."fastfetch/config.jsonc" = {
    source = ./fastfetch/config.jsonc;
  };

  home.file.".local/bin/fastfetch-link-speed" = {
    executable = true;
    text = ''
      #!/usr/bin/env sh

      iface="$(${pkgs.iproute2}/bin/ip route get 1.1.1.1 2>/dev/null | ${pkgs.gnused}/bin/sed -n 's/.* dev \([^ ]*\).*/\1/p' | ${pkgs.coreutils}/bin/head -n1)"

      if [ -z "$iface" ]; then
        iface="$(${pkgs.iproute2}/bin/ip route show default 2>/dev/null | ${pkgs.gnused}/bin/sed -n 's/.* dev \([^ ]*\).*/\1/p' | ${pkgs.coreutils}/bin/head -n1)"
      fi

      if [ -z "$iface" ]; then
        printf '%s\n' "Unavailable"
        exit 0
      fi

      speed=""
      if [ -r "/sys/class/net/$iface/speed" ]; then
        speed="$(${pkgs.coreutils}/bin/cat "/sys/class/net/$iface/speed" 2>/dev/null)"
      fi

      case "$speed" in
        ""|-*|*[!0-9]*)
          ;;
        *)
          ${pkgs.gawk}/bin/awk -v iface="$iface" -v speed="$speed" 'BEGIN {
            if (speed >= 1000) {
              printf "%s: %g Gbps\n", iface, speed / 1000
            } else {
              printf "%s: %d Mbps\n", iface, speed
            }
          }'
          exit 0
          ;;
      esac

      wifi="$(${pkgs.iw}/bin/iw dev "$iface" link 2>/dev/null | ${pkgs.gawk}/bin/awk -v iface="$iface" -F': ' '/tx bitrate:/ { print iface ": " $2; found=1; exit }')"
      if [ -n "$wifi" ]; then
        printf '%s\n' "$wifi"
      else
        printf '%s: %s\n' "$iface" "unknown"
      fi
    '';
  };

  # Keep the key binding mode explicit in managed config. This avoids fish
  # needing to preserve an old universal fish_key_bindings value.
  xdg.configFile."fish/conf.d/00-key-bindings.fish".text = ''
    set -g fish_key_bindings fish_default_key_bindings
  '';

  # oh-my-posh caches its generated Fish init script and embeds the binary's
  # absolute Nix store path in that cache. When the package path changes after
  # a rebuild, clear the cache so new shells regenerate it with the new path.
  home.activation.clearOhMyPoshCacheOnStorePathChange = lib.hm.dag.entryAfter ["writeBoundary"] ''
    HASH_FILE="$HOME/.local/state/oh-my-posh-store-hash"
    CACHE_DIR="$HOME/.cache/oh-my-posh"

    mkdir -p "$(dirname "$HASH_FILE")"

    NEW_HASH="${ohMyPoshCacheHash}"
    OLD_HASH=""
    [ -f "$HASH_FILE" ] && OLD_HASH=$(cat "$HASH_FILE")

    # Only clear cache if the package path changed and this isn't the first run.
    if [ -n "$OLD_HASH" ] && [ "$OLD_HASH" != "$NEW_HASH" ] && [ -d "$CACHE_DIR" ]; then
      echo "oh-my-posh store path changed, clearing cached init scripts..."
      $DRY_RUN_CMD ${pkgs.coreutils}/bin/rm -rf "$CACHE_DIR"
    fi

    # Recreate the cache directory so the next shell startup can repopulate it.
    $DRY_RUN_CMD ${pkgs.coreutils}/bin/mkdir -p "$CACHE_DIR"

    run echo "$NEW_HASH" > "$HASH_FILE"
  '';

  # Fish 4.3 generated this temporary migration file after moving
  # fish_key_bindings out of universal scope. Do not invoke fish here: starting
  # fish can recreate the migration notice before this cleanup runs.
  home.activation.removeFishFrozenKeyBindingsMigration = lib.hm.dag.entryAfter ["writeBoundary"] ''
    FISH_VARIABLES="$HOME/.config/fish/fish_variables"

    $DRY_RUN_CMD ${pkgs.coreutils}/bin/rm -f \
      "$HOME/.config/fish/conf.d/fish_frozen_key_bindings.fish" \
      "$HOME/.config/fish/conf.d/fish_frozen_key_bindings.fish.bak"

    if [ -f "$FISH_VARIABLES" ]; then
      $DRY_RUN_CMD ${pkgs.gnused}/bin/sed -i \
        -e '/^SETUVAR fish_key_bindings:/d' \
        -e '/^SETUVAR __fish_initialized:/d' \
        "$FISH_VARIABLES"
    else
      $DRY_RUN_CMD ${pkgs.coreutils}/bin/mkdir -p "$HOME/.config/fish"
      if [ -n "''${DRY_RUN:-}" ]; then
        echo "${pkgs.coreutils}/bin/printf ... > $FISH_VARIABLES"
      else
        ${pkgs.coreutils}/bin/printf '%s\n' \
          '# This file contains fish universal variable definitions.' \
          '# VERSION: 3.0' > "$FISH_VARIABLES"
      fi
    fi

    if [ -n "''${DRY_RUN:-}" ]; then
      echo "${pkgs.coreutils}/bin/printf ... >> $FISH_VARIABLES"
    else
      ${pkgs.coreutils}/bin/printf '%s\n' 'SETUVAR __fish_initialized:4300' >> "$FISH_VARIABLES"
    fi
  '';

  # Zoxide (smart cd)
  programs.zoxide = {
    enable = true;
    enableFishIntegration = true;
  };

  # fzf for fuzzy finding
  programs.fzf = {
    enable = true;
    enableFishIntegration = true;
  };

  # Required CLI tools
  home.packages = with pkgs; [
    eza
    oh-my-posh
  ];
}
