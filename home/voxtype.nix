# Voxtype dictation integration (Omarchy-style)
{ pkgs, lib, ... }:

{
  # Dictation engine and Wayland typing backend
  home.packages = with pkgs; [
    voxtype-vulkan
    wtype
  ];

  # Omarchy-style voxtype defaults: compositor handles the hotkey.
  xdg.configFile."voxtype/config.toml".text = ''
    state_file = "auto"

    [hotkey]
    enabled = false

    [audio]
    device = "default"
    sample_rate = 16000
    max_duration_secs = 60

    [whisper]
    model = "tiny.en"
    language = "en"
    translate = false

    [output]
    mode = "type"
    fallback_to_clipboard = true
    type_delay_ms = 1
    driver_order = ["wtype", "clipboard"]

    [output.notification]
    on_recording_start = false
    on_recording_stop = false
    on_transcription = false
  '';

  # Run voxtype as a user daemon so keybindings can call `voxtype record toggle`.
  systemd.user.services.voxtype = {
    Unit = {
      Description = "Voxtype push-to-talk dictation daemon";
      After = [ "graphical-session.target" "pipewire.service" "pipewire-pulse.service" ];
      Wants = [ "pipewire.service" "pipewire-pulse.service" ];
      PartOf = [ "graphical-session.target" ];
    };
    Service = {
      Type = "simple";
      ExecStart = "${pkgs.voxtype-vulkan}/bin/voxtype daemon";
      Restart = "on-failure";
      RestartSec = 5;
    };
    Install = {
      WantedBy = [ "graphical-session.target" ];
    };
  };

  # Ensure the selected model exists locally (downloads once when missing).
  home.activation.setupVoxtypeModel = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    model="$HOME/.local/share/voxtype/models/ggml-tiny.en.bin"

    if [ ! -f "$model" ]; then
      mkdir -p "$(dirname "$model")"
      if ${pkgs.curl}/bin/curl -m 5 -fsSL https://voxtype.io >/dev/null 2>&1; then
        ${pkgs.voxtype-vulkan}/bin/voxtype setup --download --no-post-install >/dev/null 2>&1 || true
      fi
    fi
  '';
}
