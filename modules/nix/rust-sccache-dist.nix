# Rust compiler cache/distributed compilation client
#
# The default cache is local-only until a shared sccache-dist client token is
# installed at /etc/sccache/client-token. With the token present, activation
# writes /etc/sccache/client.conf with the Beast rust-builder scheduler.
{ config, lib, pkgs, username, ... }:

let
  cfg = config.digitalpals.rustSccacheDist;
  sccacheBin = "${pkgs.sccache}/bin/sccache";
  cacheDir = "/home/${username}/.cache/sccache";
in
{
  options.digitalpals.rustSccacheDist = {
    enable = lib.mkEnableOption "Rust sccache defaults with optional Beast sccache-dist offload" // {
      default = true;
    };

    schedulerHost = lib.mkOption {
      type = lib.types.str;
      default = "rust-builder";
      description = "Hostname of the sccache-dist scheduler.";
    };

    schedulerPort = lib.mkOption {
      type = lib.types.port;
      default = 10600;
      description = "Port of the sccache-dist scheduler.";
    };

    tokenFile = lib.mkOption {
      type = lib.types.path;
      default = "/etc/sccache/client-token";
      description = "Runtime-only file containing the shared sccache-dist client token.";
    };
  };

  config = lib.mkIf cfg.enable {
    networking.hosts."10.10.0.233" = [
      "rust-builder"
      "beast-rust-builder"
    ];

    environment.systemPackages = [
      pkgs.sccache
      pkgs.mold
    ];

    environment.sessionVariables = {
      RUSTC_WRAPPER = sccacheBin;
      SCCACHE_CONF = "/etc/sccache/client.conf";
      SCCACHE_DIR = cacheDir;
      # sccache cannot cache or distribute incremental rustc invocations.
      CARGO_INCREMENTAL = "0";
    };

    systemd.tmpfiles.rules = [
      "d /etc/sccache 0755 root root -"
      "d ${cacheDir} 0755 ${username} users -"
    ];

    system.activationScripts.rustSccacheClientConfig = {
      deps = [ "users" ];
      text = ''
        install -d -m 0755 -o root -g root /etc/sccache
        install -d -m 0755 -o ${username} -g users ${cacheDir}

        if [ -s ${lib.escapeShellArg cfg.tokenFile} ]; then
          token="$(${pkgs.coreutils}/bin/tr -d '\n\r' < ${lib.escapeShellArg cfg.tokenFile})"
          cat >/etc/sccache/client.conf <<EOF
[dist]
scheduler_url = "http://${cfg.schedulerHost}:${toString cfg.schedulerPort}"
toolchains = []
toolchain_cache_size = 10737418240

[dist.auth]
type = "token"
token = "$token"

[cache.disk]
dir = "${cacheDir}"
size = 107374182400
EOF
          chown root:root /etc/sccache/client.conf
          chmod 0644 /etc/sccache/client.conf
        else
          cat >/etc/sccache/client.conf <<EOF
# Local fallback. Install the shared Beast sccache-dist token at:
#   ${cfg.tokenFile}
# then rebuild/switch to enable distributed Rust compilation.
[cache.disk]
dir = "${cacheDir}"
size = 107374182400
EOF
          chown root:root /etc/sccache/client.conf
          chmod 0644 /etc/sccache/client.conf
        fi
      '';
    };

    home-manager.users.${username} = {
      home.file.".cargo/config.toml".text = ''
        [build]
        rustc-wrapper = "${sccacheBin}"

        [env]
        SCCACHE_CONF = "/etc/sccache/client.conf"
        SCCACHE_DIR = "${cacheDir}"
        CARGO_INCREMENTAL = "0"

        [target.x86_64-unknown-linux-gnu]
        rustflags = ["-C", "link-arg=-fuse-ld=mold"]
      '';
    };
  };
}
