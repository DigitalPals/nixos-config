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
                    mountOptions = [ "compress=zstd" "noatime" ];
                  };
                  "@home" = {
                    mountpoint = "/home";
                    mountOptions = [ "compress=zstd" "noatime" ];
                  };
                  "@nix" = {
                    mountpoint = "/nix";
                    mountOptions = [ "compress=zstd" "noatime" ];
                  };
                  "@var-log" = {
                    mountpoint = "/var/log";
                    mountOptions = [ "compress=zstd" "noatime" ];
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

  # Kernel modules needed for LUKS decryption performance
  boot.initrd.availableKernelModules = [ "cryptd" "aesni_intel" ];
}
