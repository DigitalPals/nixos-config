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
      # device is set by host-specific module (kraken.nix, G1a.nix, z2-mini-g1a.nix)
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
              # Forge installer injects passwordFile here during installation
              extraOpenArgs = [
                # Allow TRIM commands to pass through to SSD (safe for modern SSDs)
                # Slight theoretical info leak but major performance/longevity benefit
                "--allow-discards"
                # Bypass dm-crypt workqueues for better I/O performance on NVMe
                # Reduces latency by avoiding extra thread scheduling
                "--perf-no_read_workqueue"
                "--perf-no_write_workqueue"
              ];
              settings = {
                # Persistent settings (written to LUKS header)
                allowDiscards = true;      # Enable TRIM passthrough
                bypassWorkqueues = true;   # Bypass dm-crypt workqueues
              };
              content = {
                type = "btrfs";
                extraArgs = [ "-f" "-L" "nixos" ];
                subvolumes = {
                  "@" = {
                    mountpoint = "/";
                    mountOptions = [ "compress=zstd" "noatime" "nodiscard" ];
                  };
                  "@home" = {
                    mountpoint = "/home";
                    mountOptions = [ "compress=zstd" "noatime" "nodiscard" ];
                  };
                  "@nix" = {
                    mountpoint = "/nix";
                    mountOptions = [ "compress=zstd" "noatime" "nodiscard" ];
                  };
                  "@var-log" = {
                    mountpoint = "/var/log";
                    mountOptions = [ "compress=zstd" "noatime" "nodiscard" ];
                  };
                };
              };
            };
          };
        };
      };
    };
  };

  # Only the root filesystem needs to be mounted in initrd. Let the remaining
  # subvolumes come up during normal system boot to avoid fragile early-boot
  # ordering around encrypted-root activation.
  fileSystems."/".neededForBoot = true;

  # Kernel modules needed for LUKS and device mapper
  # dm_mod and dm_crypt are essential for /dev/mapper/cryptroot
  boot.initrd.availableKernelModules = [ "dm_mod" "dm_crypt" "cryptd" "aesni_intel" ];

  # Ensure btrfs is supported in initrd
  boot.initrd.supportedFilesystems = [ "btrfs" ];

  # Enable emergency shell access for debugging boot issues
  # If boot hangs, add rd.systemd.unit=rescue.target to kernel params at boot
  boot.initrd.systemd.emergencyAccess = true;

}
