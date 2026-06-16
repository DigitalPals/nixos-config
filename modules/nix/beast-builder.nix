# The Beast Nix remote builder
#
# The builder is a high-CPU Proxmox LXC on The Beast:
#   - host/IP: beast-nix-builder / 10.10.0.230
#   - SSH user: nixremote
#   - Nix protocol: ssh-ng
#   - capacity: 64 logical CPUs, 128 GiB RAM, 400 GiB rootfs
#
# Authentication is intentionally not declared here because private keys should
# not live in this public repo. Ensure the rebuilding root user can SSH to the
# builder, for example by placing a key at /root/.ssh/nix-builder_ed25519 or by
# otherwise making `ssh nixremote@beast-nix-builder nix --version` work.
{ lib, ... }:

{
  networking.hosts."10.10.0.230" = [
    "beast-nix-builder"
    "nix-builder"
  ];

  programs.ssh.knownHosts.beast-nix-builder = {
    hostNames = [
      "beast-nix-builder"
      "nix-builder"
      "10.10.0.230"
    ];
    publicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICqIQ8DwY927HF+O1wrrsx6J8P+Rmns5ArqQo3Sg00vr";
  };

  nix = {
    distributedBuilds = true;

    settings = {
      builders-use-substitutes = true;

      # Do not make mobile/off-LAN rebuilds hang for a long time when The Beast
      # is unreachable; Nix can continue with local builds if the builder cannot
      # be contacted.
      connect-timeout = lib.mkDefault 5;
    };

    buildMachines = [
      {
        hostName = "beast-nix-builder";
        sshUser = "nixremote";
        sshKey = "/root/.ssh/nix-builder_ed25519";
        protocol = "ssh-ng";
        system = "x86_64-linux";

        # Kernel builds tend to be one huge derivation. Prefer one remote job
        # that can use all cores over several heavyweight jobs fighting each
        # other on the same builder.
        maxJobs = 1;
        speedFactor = 100;
        supportedFeatures = [ "big-parallel" ];
      }
    ];
  };
}
