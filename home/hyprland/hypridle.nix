# Hypridle configuration
# Screen locking and power management
{ config, pkgs, lib, hostname, ... }:

let
  # Enable auto-suspend for known hosts.
  # z2-mini-g1a stays running and only turns the display off; s2idle resume
  # left the root NVMe unavailable after wake.
  shouldAutoSuspend = builtins.elem hostname [
    "G1a"
    "kraken"
    "proart"
  ];

  # Lock command using Noctalia
  lockCmd = "noctalia-shell ipc call lockScreen lock";
  # In Lua-mode Hyprland, hl.dsp.dpms builds a dispatcher; hl.dispatch runs it.
  dpmsOnCmd = "hyprctl eval 'hl.dispatch(hl.dsp.dpms({ action = \"on\" }))'";
  dpmsOffCmd = "hyprctl eval 'hl.dispatch(hl.dsp.dpms({ action = \"off\" }))'";

  # Auto-suspend listener
  suspendListener = if shouldAutoSuspend then ''

    listener {
      timeout = 1800                   # 30 minutes
      on-timeout = systemctl suspend   # suspend when timeout has passed
    }
  '' else "";
in
{
  # Enable hypridle service
  services.hypridle.enable = true;

  # Generate config file matching Arch style
  xdg.configFile."hypr/hypridle.conf".text = ''
    general {
      lock_cmd = ${lockCmd}
      before_sleep_cmd = ${lockCmd}
      after_sleep_cmd = ${dpmsOnCmd}
    }

    listener {
      timeout = 300                    # 5 minutes
      on-timeout = ${lockCmd}
    }

    listener {
      timeout = 600                    # 10 minutes
      on-timeout = ${dpmsOffCmd}
      on-resume = ${dpmsOnCmd}
    }
    ${suspendListener}
  '';
}
