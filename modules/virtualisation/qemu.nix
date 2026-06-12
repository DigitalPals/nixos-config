# Complete local QEMU/libvirt workstation setup.
{ pkgs, username, ... }:

{
  virtualisation.libvirtd = {
    enable = true;
    onBoot = "ignore";
    onShutdown = "shutdown";
    qemu = {
      package = pkgs.qemu_full;
      swtpm.enable = true;
    };
  };

  programs.virt-manager.enable = true;
  virtualisation.spiceUSBRedirection.enable = true;

  users.users.${username}.extraGroups = [
    "kvm"
    "libvirtd"
  ];

  environment.systemPackages = with pkgs; [
    qemu_full
    virt-manager
    virt-viewer
    spice-gtk
    swtpm
    OVMF
    virtiofsd
    guestfs-tools
    libguestfs-with-appliance
    quickemu
    virtio-win
  ];
}
