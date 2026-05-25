# Noctalia Desktop Shell configuration
#
# Settings are managed with a hybrid approach:
# - Configs are seeded from repo on first run
# - GUI changes persist locally across rebuilds
# - When repo configs are updated (hash changes), local files are overwritten
# - To sync local changes back to repo, ask Claude to copy the files
{ config, pkgs, lib, inputs, hostname, osConfig ? null, ... }:

let
  # Load base settings from JSON
  baseSettings = builtins.fromJSON (builtins.readFile ./settings.json);
  noctaliaPackage = inputs.noctalia.packages.${pkgs.stdenv.hostPlatform.system}.default;
  hasFingerprintAuth =
    if osConfig == null then false else osConfig.services.fprintd.enable or false;

  # Filter out Battery widget for hosts without a battery (desktop PCs)
  hostsWithoutBattery = [ "kraken" ];
  hasBattery = !builtins.any (host: lib.hasPrefix host hostname) hostsWithoutBattery;

  # Generate host-specific settings
  settings = baseSettings // {
    general = baseSettings.general // {
      autoStartAuth = hasFingerprintAuth;
      allowPasswordWithFprintd = hasFingerprintAuth;
    };

    bar = baseSettings.bar // {
      widgets = baseSettings.bar.widgets // {
        right = builtins.filter
          (widget: hasBattery || widget.id != "Battery")
          baseSettings.bar.widgets.right;
      };
    };
  };

  settingsJson = pkgs.writeText "noctalia-settings.json" (builtins.toJSON settings);
in
{
  # Noctalia Desktop Shell
  # The module is loaded via conditional import in home/home.nix
  programs.noctalia-shell = {
    enable = true;

    # Add QtWebSockets for Claw plugin's WebSocket support
    package = noctaliaPackage.overrideAttrs (old: {
      postPatch = (old.postPatch or "") + ''
        substituteInPlace Modules/LockScreen/LockContext.qml \
          --replace-fail $'  function tryUnlock() {\n    if (!pamReady) {' \
                         $'  function tryUnlock() {\n    Logger.i("LockContext", "Unlock submit requested; text length:", currentText.length, "waiting:", waitingForPassword, "inProgress:", unlockInProgress, "pamReady:", pamReady);\n    if (unlockInProgress) {\n      Logger.i("LockContext", "Unlock already in progress, ignoring duplicate submit");\n      return;\n    }\n\n    if (!pamReady) {'

        substituteInPlace Modules/LockScreen/LockScreen.qml \
          --replace-fail $'      }\n\n      // Whether any monitor from the user'\'''s lockScreenMonitors list is currently connected.' \
                         $'      }\n\n      Shortcut {\n        sequences: ["Return", "Enter"]\n        context: Qt.ApplicationShortcut\n        enabled: root.active && !lockContext.unlockInProgress\n        onActivated: lockContext.tryUnlock()\n      }\n\n      // Whether any monitor from the user'\'''s lockScreenMonitors list is currently connected.' \
          --replace-fail $'                  onTextChanged: {\n                    if (lockContext.currentText !== text)\n                      lockContext.currentText = text;\n                  }\n                  Connections {' \
                         $'                  onTextChanged: {\n                    if (lockContext.currentText !== text)\n                      lockContext.currentText = text;\n                  }\n                  onAccepted: lockContext.tryUnlock()\n                  Connections {' \
          --replace-fail $'                onTextChanged: {\n                  if (lockContext.currentText !== text)\n                    lockContext.currentText = text;\n                }\n                Connections {' \
                         $'                onTextChanged: {\n                  if (lockContext.currentText !== text)\n                    lockContext.currentText = text;\n                }\n                onAccepted: lockContext.tryUnlock()\n                Connections {' \
          --replace-fail $'                  Component.onCompleted: forceActiveFocus()\n                }' \
                         $'                  Component.onCompleted: forceActiveFocus()\n\n                  Connections {\n                    target: Time\n                    function onResumed() {\n                      if (passwordInput && !passwordInput.activeFocus) {\n                        passwordInput.forceActiveFocus();\n                      }\n                    }\n                  }\n                }' \
          --replace-fail $'                Component.onCompleted: forceActiveFocus()\n              }' \
                         $'                Component.onCompleted: forceActiveFocus()\n\n                Connections {\n                  target: Time\n                  function onResumed() {\n                    if (blackScreenPasswordInput && !blackScreenPasswordInput.activeFocus) {\n                      blackScreenPasswordInput.forceActiveFocus();\n                    }\n                  }\n                }\n              }' \
          --replace-fail "if (Keybinds.checkKey(event, 'enter', Settings))" \
                         "if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || Keybinds.checkKey(event, 'enter', Settings))"

        substituteInPlace Modules/LockScreen/LockScreenPanel.qml \
          --replace-fail "enabled: !lockContext || !lockContext.unlockInProgress" \
                         "enabled: !root.lockControl || !root.lockControl.unlockInProgress" \
          --replace-fail $'              cursorShape: Qt.PointingHandCursor\n              onClicked: root.doUnlock()' \
                         $'              cursorShape: Qt.PointingHandCursor\n              enabled: !root.lockControl || !root.lockControl.unlockInProgress\n              onClicked: root.doUnlock()'

        substituteInPlace Services/Power/IdleInhibitorService.qml \
          --replace-fail $'  function startInhibition(newReason) {\n    reason = newReason;\n\n    if (nativeInhibitorAvailable) {' \
                         $'  function startInhibition(newReason) {\n    reason = newReason;\n    startSleepInhibition();\n\n    if (nativeInhibitorAvailable) {' \
          --replace-fail $'    if (!nativeInhibitorAvailable && inhibitorProcess.running) {\n      inhibitorProcess.signal(15); // SIGTERM\n    }\n\n    isInhibited = false;' \
                         $'    if (!nativeInhibitorAvailable && inhibitorProcess.running) {\n      inhibitorProcess.signal(15); // SIGTERM\n    }\n\n    if (sleepInhibitorProcess.running) {\n      sleepInhibitorProcess.signal(15); // SIGTERM\n    }\n\n    isInhibited = false;' \
          --replace-fail $'  // Process for maintaining the inhibition (subprocess fallback only)' \
                         $'  // Block explicit sleep requests such as Hypridle systemctl suspend.\n  function startSleepInhibition() {\n    sleepInhibitorProcess.command = ["systemd-inhibit", "--what=sleep", "--why=" + reason, "--mode=block", "sleep", "infinity"];\n    sleepInhibitorProcess.running = true;\n  }\n\n  // Process for maintaining the inhibition (subprocess fallback only)' \
          --replace-fail $'      Logger.d("IdleInhibitor", "Inhibitor process started successfully");\n    }\n  }\n\n  Timer {' \
                         $'      Logger.d("IdleInhibitor", "Inhibitor process started successfully");\n    }\n  }\n\n  Process {\n    id: sleepInhibitorProcess\n    running: false\n\n    onExited: function (exitCode, exitStatus) {\n      if (isInhibited) {\n        Logger.w("IdleInhibitor", "Sleep inhibitor process exited unexpectedly:", exitCode);\n      }\n    }\n\n    onStarted: function () {\n      Logger.d("IdleInhibitor", "Sleep inhibitor process started successfully");\n    }\n  }\n\n  Timer {'

      '';
      buildInputs = (old.buildInputs or []) ++ [ pkgs.qt6.qtwebsockets ];
    });

    # Disable automatic systemd service - we control startup via Hyprland autostart
    systemd.enable = false;
  };

  # Noctalia configuration files - hybrid approach
  # Seeds from repo on first run, preserves local GUI changes,
  # but overwrites when repo configs are updated (hash changes)
  home.activation.noctaliaConfig = lib.hm.dag.entryAfter ["writeBoundary"] ''
    NOCTALIA_DIR="$HOME/.config/noctalia"
    HASH_FILE="$NOCTALIA_DIR/.deployed-hash"

    mkdir -p "$NOCTALIA_DIR"

    # Calculate hash of repo configs
    REPO_HASH=$(cat ${settingsJson} ${./gui-settings.json} ${./colors.json} ${./plugins.json} | ${pkgs.coreutils}/bin/sha256sum | cut -d' ' -f1)

    # Check if we should deploy (first run OR repo updated)
    SHOULD_DEPLOY=false
    if [ ! -f "$NOCTALIA_DIR/settings.json" ]; then
      SHOULD_DEPLOY=true
    elif [ -f "$HASH_FILE" ] && [ "$(cat "$HASH_FILE")" != "$REPO_HASH" ]; then
      SHOULD_DEPLOY=true
      echo "Noctalia: Repo configs updated, syncing..."
    fi

    if [ "$SHOULD_DEPLOY" = true ]; then
      cp ${settingsJson} "$NOCTALIA_DIR/settings.json"
      cp ${./gui-settings.json} "$NOCTALIA_DIR/gui-settings.json"
      cp ${./colors.json} "$NOCTALIA_DIR/colors.json"
      cp ${./plugins.json} "$NOCTALIA_DIR/plugins.json"
      chmod 644 "$NOCTALIA_DIR"/*.json
      echo "$REPO_HASH" > "$HASH_FILE"
    fi
  '';

  # Noctalia-specific packages
  home.packages = with pkgs; [
    quickshell  # For Noctalia IPC commands
  ];

  # Create symlink for Quickshell IPC config lookup
  # This allows "qs -c noctalia-shell ipc ..." commands to find the running instance
  home.activation.noctaliaQuickshellSymlink = lib.hm.dag.entryAfter ["writeBoundary"] ''
    QS_CONFIG_DIR="$HOME/.config/quickshell"
    mkdir -p "$QS_CONFIG_DIR"
    rm -f "$QS_CONFIG_DIR/noctalia-shell"
    ln -s "${config.programs.noctalia-shell.package}/share/noctalia-shell" "$QS_CONFIG_DIR/noctalia-shell"
  '';
}
