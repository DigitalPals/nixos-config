# Hypridle configuration
# Screen locking and power management
{ config, pkgs, lib, hostname, ... }:

let
  isLaptop = hostname == "G1a";

  # Laptop-specific suspend listener
  suspendListener = if isLaptop then ''

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
      lock_cmd = pidof -q noctalia-shell && noctalia-shell ipc call lockScreen lock
      before_sleep_cmd = pidof -q noctalia-shell && noctalia-shell ipc call lockScreen lock
      after_sleep_cmd = hyprctl dispatch dpms on
    }

    listener {
      timeout = 300                    # 5 minutes
      on-timeout = pidof -q noctalia-shell && noctalia-shell ipc call lockScreen lock
    }

    listener {
      timeout = 600                    # 10 minutes
      on-timeout = hyprctl dispatch dpms off
      on-resume = hyprctl dispatch dpms on
    }
    ${suspendListener}
  '';
}
