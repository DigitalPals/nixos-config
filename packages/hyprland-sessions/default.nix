{ pkgs }:

let
  hyprland-wayle-bin = pkgs.writeShellScriptBin "hyprland-wayle" ''
    # Required environment variables for Wayland session
    # XDG_SESSION_TYPE must be set early (Hyprland 0.47+ regression fix)
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=Hyprland
    export DESKTOP_SHELL=wayle

    # Create runtime directory and mark desktop shell
    mkdir -p "$XDG_RUNTIME_DIR"
    echo "wayle" > "$XDG_RUNTIME_DIR/desktop-shell"

    # Set up log directory
    mkdir -p "''${XDG_STATE_HOME:-$HOME/.local/state}/hyprland"
    HYPRLAND_LOG="''${XDG_STATE_HOME:-$HOME/.local/state}/hyprland/session.log"

    # Launch Hyprland via start-hyprland (required since Hyprland 0.53)
    # start-hyprland provides crash recovery and safe mode
    # Redirect output to log file to prevent TTY clutter during boot
    exec start-hyprland -- --config "$HOME/.config/hypr/hyprland.lua" "$@" > "$HYPRLAND_LOG" 2>&1
  '';

  hyprland-wayle-session = pkgs.stdenvNoCC.mkDerivation {
    pname = "hyprland-wayle-session";
    version = "1.0.0";
    dontUnpack = true;

    passthru.providedSessions = [ "hyprland-wayle" ];

    installPhase = ''
      mkdir -p $out/share/wayland-sessions
      mkdir -p $out/bin

      # Symlink the wrapper script
      ln -s ${hyprland-wayle-bin}/bin/hyprland-wayle $out/bin/hyprland-wayle

      # Create .desktop file
      cat > $out/share/wayland-sessions/hyprland-wayle.desktop << EOF
      [Desktop Entry]
      Name=Hyprland (Wayle)
      Comment=Hyprland with Wayle Desktop Shell
      Exec=$out/bin/hyprland-wayle
      Type=Application
      DesktopNames=Hyprland
      EOF
    '';
  };
in {
  # Session package for display manager registration
  wayle = hyprland-wayle-session;

  # All session packages as a list
  sessions = [ hyprland-wayle-session ];

  # Wrapper script for PATH
  script = hyprland-wayle-bin;
}
