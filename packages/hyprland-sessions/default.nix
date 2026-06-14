{ pkgs }:

let
  hyprland-lumen-bin = pkgs.writeShellScriptBin "hyprland-lumen" ''
    # Required environment variables for Wayland session
    # XDG_SESSION_TYPE must be set early (Hyprland 0.47+ regression fix)
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=Hyprland
    export DESKTOP_SHELL=lumen

    # Create runtime directory and mark desktop shell
    mkdir -p "$XDG_RUNTIME_DIR"
    echo "lumen" > "$XDG_RUNTIME_DIR/desktop-shell"

    # Set up log directory
    mkdir -p "''${XDG_STATE_HOME:-$HOME/.local/state}/hyprland"
    HYPRLAND_LOG="''${XDG_STATE_HOME:-$HOME/.local/state}/hyprland/session.log"

    # Launch Hyprland via start-hyprland (required since Hyprland 0.53)
    # start-hyprland provides crash recovery and safe mode
    # Redirect output to log file to prevent TTY clutter during boot
    exec start-hyprland -- "$@" > "$HYPRLAND_LOG" 2>&1
  '';

  hyprland-lumen-session = pkgs.stdenvNoCC.mkDerivation {
    pname = "hyprland-lumen-session";
    version = "1.0.0";
    dontUnpack = true;

    passthru.providedSessions = [ "hyprland-lumen" ];

    installPhase = ''
      mkdir -p $out/share/wayland-sessions
      mkdir -p $out/bin

      # Symlink the wrapper script
      ln -s ${hyprland-lumen-bin}/bin/hyprland-lumen $out/bin/hyprland-lumen

      # Create .desktop file
      cat > $out/share/wayland-sessions/hyprland-lumen.desktop << EOF
      [Desktop Entry]
      Name=Hyprland (Lumen)
      Comment=Hyprland with Lumen Desktop Shell
      Exec=$out/bin/hyprland-lumen
      Type=Application
      DesktopNames=Hyprland
      EOF
    '';
  };
in {
  # Session package for display manager registration
  lumen = hyprland-lumen-session;

  # All session packages as a list
  sessions = [ hyprland-lumen-session ];

  # Wrapper script for PATH
  script = hyprland-lumen-bin;
}
