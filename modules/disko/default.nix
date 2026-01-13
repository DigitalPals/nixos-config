# Common disko configuration for declarative disk partitioning
# Partition layout (Omarchy-inspired):
# - 2GB EFI partition (FAT32, /boot)
# - Remaining space: LUKS2 encrypted Btrfs with subvolumes
#
# Subvolumes:
# - @         -> /
# - @home     -> /home
# - @nix      -> /nix
# - @var-log  -> /var/log
#
# No swap partition - using zram only (configured in common.nix)
{ lib, ... }:

{
  disko.devices = {
    disk.main = {
      type = "disk";
      # device is set by host-specific module (kraken.nix, G1a.nix)
      content = {
        type = "gpt";
        partitions = {
          ESP = {
            label = "ESP";
            size = "2G";
            type = "EF00";
            content = {
              type = "filesystem";
              format = "vfat";
              mountpoint = "/boot";
              mountOptions = [ "umask=0077" "nofail" "x-systemd.device-timeout=30s" ];
            };
          };
          luks = {
            label = "cryptroot";
            size = "100%";
            content = {
              type = "luks";
              name = "cryptroot";
              # No keyFile or passwordFile = interactive passphrase prompt
              extraOpenArgs = [
                "--allow-discards"
                "--perf-no_read_workqueue"
                "--perf-no_write_workqueue"
              ];
              settings = {
                allowDiscards = true;
                bypassWorkqueues = true;
              };
              content = {
                type = "btrfs";
                extraArgs = [ "-f" "-L" "nixos" ];
                subvolumes = {
                  "@" = {
                    mountpoint = "/";
                    # x-systemd.device-timeout=0 disables the 90s default timeout (NixOS#250003)
                    # x-systemd.after ensures mount waits for udev to settle after LUKS decryption
                    mountOptions = [ "compress=zstd" "noatime" "x-systemd.device-timeout=0" "x-systemd.after=systemd-udev-settle.service" ];
                  };
                  "@home" = {
                    mountpoint = "/home";
                    mountOptions = [ "compress=zstd" "noatime" "x-systemd.device-timeout=0" "x-systemd.after=systemd-udev-settle.service" ];
                  };
                  "@nix" = {
                    mountpoint = "/nix";
                    mountOptions = [ "compress=zstd" "noatime" "x-systemd.device-timeout=0" "x-systemd.after=systemd-udev-settle.service" ];
                  };
                  "@var-log" = {
                    mountpoint = "/var/log";
                    mountOptions = [ "compress=zstd" "noatime" "x-systemd.device-timeout=0" "x-systemd.after=systemd-udev-settle.service" ];
                  };
                };
              };
            };
          };
        };
      };
    };
  };

  # Mount all subvolumes in initrd to avoid race conditions with LUKS device
  # Without this, systemd may try to mount before /dev/mapper/cryptroot is ready
  fileSystems."/".neededForBoot = true;
  fileSystems."/home".neededForBoot = true;
  fileSystems."/nix".neededForBoot = true;
  fileSystems."/var/log".neededForBoot = true;

  # Kernel modules needed for LUKS and device mapper
  # dm_mod and dm_crypt are essential for /dev/mapper/cryptroot
  boot.initrd.availableKernelModules = [ "dm_mod" "dm_crypt" "cryptd" "aesni_intel" ];

  # Ensure btrfs is supported in initrd
  boot.initrd.supportedFilesystems = [ "btrfs" ];

  # Enable emergency shell access for debugging boot issues
  # If boot hangs, add rd.systemd.unit=rescue.target to kernel params at boot
  boot.initrd.systemd.emergencyAccess = true;

  # Fix for udev race condition with systemd initrd + LUKS
  # After LUKS decryption, udev needs to process the new /dev/mapper/cryptroot device.
  # Without this, systemd may not recognize the device as ready, causing boot to hang
  # with "A start job is running for /dev/mapper/cryptroot".
  # Reference: https://github.com/NixOS/nixpkgs/issues/42165
  boot.initrd.systemd.services.cryptsetup-udev-settle = {
    description = "Wait for udev after LUKS decryption";
    wantedBy = [ "cryptsetup.target" ];
    after = [ "systemd-cryptsetup@cryptroot.service" ];
    before = [ "cryptsetup.target" ];
    unitConfig.DefaultDependencies = "no";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "/bin/udevadm settle";
      RemainAfterExit = true;
    };
  };
}
