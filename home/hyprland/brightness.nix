# Brightness control script
# Routes brightness keys to the correct tool based on which monitor the cursor is on:
# - Apple Studio Display (XDR): uses asdcontrol via USB HID
# - Laptop/other displays: uses brightnessctl
{ pkgs, lib, hostname ? "" }:

let
  stopXpsAutoBrightness = if lib.hasPrefix "xps" hostname then ''
    ${pkgs.systemd}/bin/systemctl --user stop wluma.service 2>/dev/null || true
  '' else "";
in
pkgs.writeShellScript "brightness-control" ''
  direction="$1"  # "up" or "down"
  step="''${2:-5}" # percentage step, default 5

  # Get the focused monitor's make (focused follows cursor in Hyprland)
  monitor_make=$(${pkgs.hyprland}/bin/hyprctl monitors -j 2>/dev/null | ${pkgs.jq}/bin/jq -r '.[] | select(.focused == true) | .make // ""')

  if echo "$monitor_make" | grep -qi "apple"; then
    # Find the Apple display HID device
    hiddev=""
    for dev in /dev/usb/hiddev*; do
      [ -e "$dev" ] || continue
      if asdcontrol --silent --detect "$dev" 2>/dev/null | grep -q "SUPPORTED"; then
        hiddev="$dev"
        break
      fi
    done

    if [ -z "$hiddev" ]; then
      exit 1
    fi

    if [ "$direction" = "up" ]; then
      asdcontrol --silent --brief "$hiddev" "+$step%"
    else
      asdcontrol --silent --brief "$hiddev" -- "-$step%"
    fi
  else
    # Laptop or other display: use brightnessctl
    ${stopXpsAutoBrightness}
    if [ "$direction" = "up" ]; then
      ${pkgs.brightnessctl}/bin/brightnessctl set "$step%+"
    else
      ${pkgs.brightnessctl}/bin/brightnessctl set "$step%-"
    fi
  fi
''
