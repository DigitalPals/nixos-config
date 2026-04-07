# Forge Installer ISO
# Boots directly into Forge TUI with automatic network setup
{ config, lib, pkgs, modulesPath, plymouth-cybex, ... }:

let
  # Startup script that checks connectivity and launches Forge
  forgeStartup = pkgs.writeShellScriptBin "forge-startup" ''
    # Drop to shell on Ctrl+C
    trap 'echo ""; echo "  Interrupted. Type \"forge-startup\" to restart."; exit 0' INT

    # Connectivity check using curl (works even if ICMP is blocked)
    check_internet() {
      ${pkgs.curl}/bin/curl -sf --max-time 5 https://github.com > /dev/null 2>&1
    }

    clear
    echo ""
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║     Forge NixOS Installer             ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo ""
    echo "  Press Ctrl+C for a shell. Other TTYs: Ctrl+Alt+F2"
    echo ""

    # Wait for NetworkManager to activate startup connections
    # Ethernet: 2-5s. WiFi (unconfigured): times out at 15s.
    echo "  Waiting for network..."
    ${pkgs.networkmanager}/bin/nm-online -s -q --timeout=15

    # Check actual internet connectivity
    while ! check_internet; do
      echo ""
      echo "  No internet connection detected."
      echo "  Opening network configuration..."
      echo ""
      sleep 1
      ${pkgs.networkmanager}/bin/nmtui
      clear
      echo ""
      echo "  ╔═══════════════════════════════════════╗"
      echo "  ║     Forge NixOS Installer             ║"
      echo "  ╚═══════════════════════════════════════╝"
      echo ""
      echo "  Checking connection..."
    done

    echo ""
    echo "  Connected! Starting Forge..."
    echo ""
    sleep 1

    # Run Forge with error recovery
    while true; do
      sudo nix run github:DigitalPals/nixos-config
      exit_code=$?
      if [ $exit_code -eq 0 ]; then
        break
      fi
      echo ""
      echo "  Forge exited with an error (code: $exit_code)."
      echo ""
      echo "  [r] Retry"
      echo "  [s] Drop to shell"
      echo ""
      read -r -p "  Choice [r/s]: " choice
      case "$choice" in
        s|S) echo "  Type 'forge-startup' to restart."; break ;;
        *) echo "  Retrying..."; echo "" ;;
      esac
    done
  '';
in
{
  imports = [
    "${modulesPath}/installer/cd-dvd/installation-cd-minimal.nix"
  ];

  # Use latest kernel for best hardware support
  boot.kernelPackages = pkgs.linuxPackages_latest;

  # Disable ZFS (broken with kernel 6.18.x, not needed for installer)
  boot.supportedFilesystems.zfs = lib.mkForce false;

  # ISO naming - includes kernel version
  image.baseName = lib.mkForce "NixOS-Cybex-${config.boot.kernelPackages.kernel.version}";
  isoImage.volumeID = "NIXOS_CYBEX";

  # Auto-login to nixos user
  services.getty.autologinUser = "nixos";

  # Enable flakes (required for nix run)
  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  # NetworkManager for WiFi
  networking.networkmanager.enable = true;
  networking.wireless.enable = lib.mkForce false;

  # Plymouth boot splash with cybex theme
  boot.plymouth = {
    enable = true;
    themePackages = [ plymouth-cybex ];
    theme = "cybex";
  };
  boot.kernelParams = [ "quiet" "splash" ];
  boot.consoleLogLevel = 0;
  boot.initrd.verbose = false;

  # Debugging and utility packages
  environment.systemPackages = with pkgs; [
    # Startup script
    forgeStartup

    # Networking
    networkmanager
    iputils
    curl
    wget

    # Editors
    neovim
    nano

    # System monitoring
    btop
    htop

    # Disk utilities
    parted
    gptfdisk
    smartmontools
    nvme-cli

    # File utilities
    file
    less
    tree
    unzip

    # Git (for nix run)
    git
  ];

  # Run forge-startup on login to tty1
  programs.bash.loginShellInit = ''
    if [ "$(tty)" = "/dev/tty1" ] && [ ! -f /tmp/forge-started ]; then
      touch /tmp/forge-started
      exec forge-startup
    fi
  '';

  # Allow passwordless sudo for nixos user
  security.sudo.wheelNeedsPassword = false;

  # Disable unnecessary services for faster boot
  documentation.enable = false;
  documentation.nixos.enable = false;
}
