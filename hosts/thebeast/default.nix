# thebeast - Threadripper workstation with AMD Radeon RX 7700 XT / 7800 XT
{ config, pkgs, lib, username, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../modules/boot/limine-plymouth.nix
    ../../modules/hardware/amd.nix
    ../../modules/virtualisation/qemu.nix
  ];

  networking.hostName = "thebeast";

  # Remote access for this always-on workstation. Only public-key auth is
  # accepted; the authorized key is derived from ~/.ssh/id_ed25519.
  services.openssh = {
    enable = true;
    openFirewall = true;
    settings = {
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      PermitRootLogin = "no";
    };
  };

  users.users.${username}.openssh.authorizedKeys.keys = [
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAsuJT0cHYYQxy46HB21Ja/jEJwKwrBL3DBSzb1CgvWu john@cybex.net"
  ];

  # Keep SSH reachable: the display may sleep via Hypridle DPMS, but the system
  # itself should not enter a sleep state.
  systemd.sleep.settings.Sleep = {
    AllowSuspend = "no";
    AllowHibernation = "no";
    AllowHybridSleep = "no";
    AllowSuspendThenHibernate = "no";
  };

  services.logind.settings.Login = {
    HandleSuspendKey = "ignore";
    HandleHibernateKey = "ignore";
    IdleAction = "ignore";
  };

  # Enable official amdgpu initrd support for early KMS and Plymouth.
  hardware.amdgpu.initrd.enable = true;

  # Early boot kernel modules:
  # - amdgpu: enables early KMS for high-res Plymouth/console
  # - HID modules: ensures keyboard works for LUKS passphrase entry
  boot.initrd.kernelModules = lib.mkForce [
    "amdgpu"
    "hid-generic"
    "usbhid"
  ];

  # Realtek RTL8922AE Wi-Fi 7.
  boot.kernelModules = [ "rtw89_8922ae" ];

  networking.wireless.iwd = {
    enable = true;
    settings = {
      General = {
        EnableNetworkConfiguration = false;
      };
      Settings = {
        AutoConnect = true;
      };
    };
  };

  networking.networkmanager.wifi.backend = "iwd";
  networking.networkmanager.wifi.scanRandMacAddress = false;
}
