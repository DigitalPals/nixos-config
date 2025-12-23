# Fetch and copy Illogical Impulse dotfiles from upstream
{ config, pkgs, lib, dots-hyprland, rounded-polygon-qmljs, ... }:

let
  # Source paths from dots-hyprland
  configSource = "${dots-hyprland}/dots/.config";

  # Quickshell weather settings
  weatherSettings = {
    "options.bar.weather.city" = "Emmen, Drenthe, Netherlands";
    "options.bar.weather.enableGPS" = false;
  };
in
{
  # Copy Quickshell configuration to ~/.config/quickshell/ii/
  xdg.configFile."quickshell/ii" = {
    source = "${configSource}/quickshell/ii";
    recursive = true;
  };

  # Copy Starship prompt configuration
  xdg.configFile."starship.toml" = {
    source = "${configSource}/starship.toml";
  };

  # Create state directories for Material You theming and copy git submodule content
  home.activation.setupIllogicalDirs = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
    mkdir -p "$HOME/.local/state/quickshell/user/generated/terminal"

    # Copy the shapes submodule (not included in dots-hyprland due to git submodules)
    shapesDir="$HOME/.config/quickshell/ii/modules/common/widgets/shapes"
    if [ ! -f "$shapesDir/qmldir" ]; then
      run rm -rf "$shapesDir" 2>/dev/null || true
      run cp -r "${rounded-polygon-qmljs}" "$shapesDir"
      run chmod -R u+w "$shapesDir"
    fi

    # Apply quickshell weather settings to states.json
    statesFile="$HOME/.local/state/quickshell/states.json"
    if [ -f "$statesFile" ]; then
      run ${pkgs.jq}/bin/jq '. + ${builtins.toJSON weatherSettings}' "$statesFile" > "$statesFile.tmp" && run mv "$statesFile.tmp" "$statesFile"
    else
      run echo '${builtins.toJSON weatherSettings}' > "$statesFile"
    fi
  '';
}
