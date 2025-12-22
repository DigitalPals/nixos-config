# Common NixOS configuration shared across all machines
{ config, pkgs, lib, claude-code-pkg, ... }:

{
  # Enable flakes
  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  # Increase download buffer size for faster fetches
  nix.settings.download-buffer-size = 256 * 1024 * 1024; # 256 MiB

  # Automatic garbage collection to prevent store bloat
  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 14d";
  };

  # Optimize store automatically
  nix.settings.auto-optimise-store = true;

  # Create /bin/bash symlink for script compatibility
  environment.binsh = "${pkgs.bash}/bin/bash";

  # Enable nix-ld for running dynamically linked executables
  programs.nix-ld = {
    enable = true;
    libraries = with pkgs; [
      stdenv.cc.cc.lib
      zlib
      openssl
      curl
      glib
      gtk3
      libGL
    ];
  };

  # Networking
  networking.networkmanager.enable = true;

  # Firewall
  networking.firewall = {
    enable = true;
    allowedTCPPorts = [ ];
    allowedUDPPorts = [ ];
  };

  # Disable NetworkManager-wait-online to speed up boot
  systemd.services.NetworkManager-wait-online.enable = lib.mkForce false;

  # Timezone and locale
  time.timeZone = "Europe/Amsterdam";
  i18n.defaultLocale = "en_US.UTF-8";
  i18n.extraLocaleSettings = {
    LC_ADDRESS = "nl_NL.UTF-8";
    LC_IDENTIFICATION = "nl_NL.UTF-8";
    LC_MEASUREMENT = "nl_NL.UTF-8";
    LC_MONETARY = "nl_NL.UTF-8";
    LC_NAME = "nl_NL.UTF-8";
    LC_NUMERIC = "nl_NL.UTF-8";
    LC_PAPER = "nl_NL.UTF-8";
    LC_TELEPHONE = "nl_NL.UTF-8";
    LC_TIME = "nl_NL.UTF-8";
  };

  # Enable OpenGL (works for all GPUs)
  hardware.graphics = {
    enable = true;
    enable32Bit = true;
  };

  # AMD CPU microcode updates
  hardware.cpu.amd.updateMicrocode = true;

  # Firmware for AMD GPUs and other hardware
  hardware.enableRedistributableFirmware = true;

  # Thermal monitoring
  boot.kernelModules = [ "k10temp" ];

  # Bluetooth
  hardware.bluetooth.enable = true;

  # Power management (use mkDefault so laptop can override with TLP)
  services.power-profiles-daemon.enable = lib.mkDefault true;
  services.upower.enable = true;

  # Swap (zram for memory pressure handling)
  zramSwap = {
    enable = true;
    memoryPercent = 25;
  };

  # Docker
  virtualisation.docker = {
    enable = true;
    enableOnBoot = false;
  };

  # Audio (PipeWire)
  services.pulseaudio.enable = false;
  security.rtkit.enable = true;
  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };

  # User configuration
  # mutableUsers allows setting password with passwd after installation
  users.mutableUsers = true;
  users.users.john = {
    isNormalUser = true;
    description = "John";
    extraGroups = [ "networkmanager" "wheel" "video" "input" "docker" ];
    shell = pkgs.fish;
    # No initialPassword - password set manually with passwd after first boot
  };

  # Enable Fish system-wide (required for login shell)
  programs.fish.enable = true;

  # Programs and packages
  services.printing.enable = true;
  programs.firefox.enable = true;

  nixpkgs.config.allowUnfree = true;

  programs._1password.enable = true;
  programs._1password-gui = {
    enable = true;
    polkitPolicyOwners = [ "john" ];
  };

  environment.systemPackages = with pkgs; [
    git
    wl-clipboard
    xdg-utils
    efibootmgr
    lm_sensors
    powertop
    claude-code-pkg
  ];

  # Security
  security.sudo.extraRules = [
    {
      users = [ "john" ];
      commands = [
        {
          command = "ALL";
          options = [ "NOPASSWD" ];
        }
      ];
    }
  ];

  # GNOME Keyring - Auto-unlock on login
  services.gnome.gnome-keyring.enable = true;
  security.pam.services.greetd.enableGnomeKeyring = true;
  security.pam.services.login.enableGnomeKeyring = true;

  # Kernel hardening
  boot.kernel.sysctl = {
    # Restrict kernel pointer exposure
    "kernel.kptr_restrict" = 2;
    # Restrict dmesg access to root
    "kernel.dmesg_restrict" = 1;
    # Disable unprivileged BPF
    "kernel.unprivileged_bpf_disabled" = 1;
    # Restrict perf events
    "kernel.perf_event_paranoid" = 3;
    # Prevent null pointer dereference exploits
    "vm.mmap_min_addr" = 65536;
    # Restrict ptrace scope
    "kernel.yama.ptrace_scope" = 1;
    # Network hardening
    "net.ipv4.conf.all.rp_filter" = 1;
    "net.ipv4.conf.default.rp_filter" = 1;
    "net.ipv4.icmp_echo_ignore_broadcasts" = 1;
    "net.ipv4.conf.all.accept_redirects" = 0;
    "net.ipv4.conf.default.accept_redirects" = 0;
    "net.ipv6.conf.all.accept_redirects" = 0;
    "net.ipv6.conf.default.accept_redirects" = 0;
  };

  # I/O scheduler tuning for NVMe (use none/mq-deadline for best performance)
  services.udev.extraRules = ''
    # NVMe drives - use none scheduler (lowest latency)
    ACTION=="add|change", KERNEL=="nvme[0-9]*", ATTR{queue/scheduler}="none"
    # SATA SSDs - use mq-deadline
    ACTION=="add|change", KERNEL=="sd[a-z]", ATTR{queue/rotational}=="0", ATTR{queue/scheduler}="mq-deadline"
  '';

  # System state version
  system.stateVersion = "24.11";
}
