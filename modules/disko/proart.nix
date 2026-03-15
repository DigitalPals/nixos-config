# Disko configuration for proart (ASUS ProArt P16 OLED)
# Uses LUKS → LVM → (swap + btrfs) for reliable hibernate support
#
# Partition layout:
# - 2GB EFI partition (FAT32, /boot)
# - Remaining space: LUKS2 encrypted LVM
#   - 66GB swap logical volume (for hibernate with 64GB RAM)
#   - Remaining: Btrfs logical volume with subvolumes
#
# Subvolumes:
# - @         -> /
# - @home     -> /home
# - @nix      -> /nix
# - @var-log  -> /var/log
{ lib, ... }:

{
  disko.devices = {
    disk.main = {
      type = "disk";
      device = "/dev/nvme0n1";
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
                "--allow-discards"
                "--perf-no_read_workqueue"
                "--perf-no_write_workqueue"
              ];
              settings = {
                allowDiscards = true;
                bypassWorkqueues = true;
              };
              content = {
                type = "lvm_pv";
                vg = "vg";
              };
            };
          };
        };
      };
    };

    # LVM Volume Group
    lvm_vg.vg = {
      type = "lvm_vg";
      lvs = {
        # Swap logical volume for hibernate (64GB RAM + 2GB headroom)
        swap = {
          size = "66G";
          content = {
            type = "swap";
            discardPolicy = "both";  # Enable TRIM for SSD
            # Note: boot.resumeDevice is set manually in hosts/proart/default.nix
            # to avoid potential path conflicts with disko's auto-generated path
          };
        };
        # Root logical volume with btrfs and subvolumes
        root = {
          size = "100%FREE";
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

  # Mount all subvolumes in initrd to avoid race conditions
  fileSystems."/".neededForBoot = true;
  fileSystems."/home".neededForBoot = true;
  fileSystems."/nix".neededForBoot = true;
  fileSystems."/var/log".neededForBoot = true;

  # Kernel modules needed for LUKS, LVM, and device mapper
  boot.initrd.availableKernelModules = [ "dm_mod" "dm_crypt" "dm-snapshot" "cryptd" "aesni_intel" ];

  # Ensure btrfs is supported in initrd
  boot.initrd.supportedFilesystems = [ "btrfs" ];

  # Enable emergency shell access for debugging boot issues
  boot.initrd.systemd.emergencyAccess = true;
}
