# Automatic shell restart on store path change
#
# After nixos-rebuild, the running quickshell process may have old store paths
# while IPC commands point to the new path. This activation hook detects when
# the shell package has changed and restarts the shell via hyprctl.
#
# See CLAUDE.md for details on the store path persistence issue.
{ config, pkgs, lib, ... }:

let
  # Hash the noctalia-shell package path to detect changes
  shellPackageHash = builtins.hashString "sha256" (
    toString config.programs.noctalia-shell.package
  );
in
{
  home.activation.restartShellOnStorePathChange = lib.hm.dag.entryAfter ["writeBoundary"] ''
    HASH_FILE="$HOME/.local/state/shell-store-hash"
    mkdir -p "$(dirname "$HASH_FILE")"

    NEW_HASH="${shellPackageHash}"
    OLD_HASH=""
    [ -f "$HASH_FILE" ] && OLD_HASH=$(cat "$HASH_FILE")

    # Only restart if hash changed AND we had a previous hash (not first run)
    if [ -n "$OLD_HASH" ] && [ "$OLD_HASH" != "$NEW_HASH" ]; then
      # Check if quickshell is running and Hyprland is available
      if ${pkgs.procps}/bin/pgrep -x quickshell >/dev/null 2>&1; then
        if command -v hyprctl >/dev/null 2>&1 && hyprctl version >/dev/null 2>&1; then
          echo "Shell store path changed, restarting Noctalia..."

          # Kill old processes
          $DRY_RUN_CMD ${pkgs.procps}/bin/pkill -x quickshell || true
          sleep 0.5

          # Restart via hyprctl for proper Wayland integration
          $DRY_RUN_CMD hyprctl dispatch exec "noctalia-shell"
        fi
      fi
    fi

    # Record current hash (use run for dry-run support)
    run echo "$NEW_HASH" > "$HASH_FILE"
  '';
}
