{ pkgs }:

let
  # Wrapper script for Hyprland with Noctalia Desktop Shell
  hyprland-noctalia-bin = pkgs.writeShellScriptBin "hyprland-noctalia" ''
    # Required environment variables for Wayland session
    # XDG_SESSION_TYPE must be set early (Hyprland 0.47+ regression fix)
    export XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=Hyprland
    export DESKTOP_SHELL=noctalia

    # Create runtime directory and mark desktop shell
    mkdir -p "$XDG_RUNTIME_DIR"
    echo "noctalia" > "$XDG_RUNTIME_DIR/desktop-shell"

    # Launch Hyprland (uses default ~/.config/hypr/hyprland.conf)
    exec Hyprland "$@"
  '';

  # Session package with .desktop file for Noctalia
  hyprland-noctalia-session = pkgs.stdenvNoCC.mkDerivation {
    pname = "hyprland-noctalia-session";
    version = "1.0.0";
    dontUnpack = true;

    passthru.providedSessions = [ "hyprland-noctalia" ];

    installPhase = ''
      mkdir -p $out/share/wayland-sessions
      mkdir -p $out/bin

      # Symlink the wrapper script
      ln -s ${hyprland-noctalia-bin}/bin/hyprland-noctalia $out/bin/hyprland-noctalia

      # Create .desktop file
      cat > $out/share/wayland-sessions/hyprland-noctalia.desktop << EOF
      [Desktop Entry]
      Name=Hyprland (Noctalia)
      Comment=Hyprland with Noctalia Desktop Shell
      Exec=$out/bin/hyprland-noctalia
      Type=Application
      DesktopNames=Hyprland
      EOF
    '';
  };

in {
  # Session package for display manager registration
  noctalia = hyprland-noctalia-session;

  # All session packages as a list
  sessions = [ hyprland-noctalia-session ];

  # Wrapper script for PATH
  script = hyprland-noctalia-bin;
}
