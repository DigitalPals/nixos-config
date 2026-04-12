# Fish shell configuration for Noctalia
# Custom prompt theme and shell tools
{ config, pkgs, lib, ... }:

let
  ohMyPoshPackageHash = builtins.hashString "sha256" (toString pkgs.oh-my-posh);
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

      # VISUAL for programs that distinguish from EDITOR
      set -gx VISUAL nvim

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
      fastfetch = "fastfetch -c archey";
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

  # oh-my-posh caches its generated Fish init script and embeds the binary's
  # absolute Nix store path in that cache. When the package path changes after
  # a rebuild, clear the cache so new shells regenerate it with the new path.
  home.activation.clearOhMyPoshCacheOnStorePathChange = lib.hm.dag.entryAfter ["writeBoundary"] ''
    HASH_FILE="$HOME/.local/state/oh-my-posh-store-hash"
    CACHE_DIR="$HOME/.cache/oh-my-posh"

    mkdir -p "$(dirname "$HASH_FILE")"

    NEW_HASH="${ohMyPoshPackageHash}"
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
  # fish_key_bindings out of universal scope. The migration is complete.
  home.activation.removeFishFrozenKeyBindingsMigration = lib.hm.dag.entryAfter ["writeBoundary"] ''
    $DRY_RUN_CMD ${pkgs.coreutils}/bin/rm -f \
      "$HOME/.config/fish/conf.d/fish_frozen_key_bindings.fish" \
      "$HOME/.config/fish/conf.d/fish_frozen_key_bindings.fish.bak"
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
