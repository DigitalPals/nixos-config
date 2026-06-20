{
  description = "NixOS configuration with Home Manager, Hyprland, and Lumen Desktop Shell";

  inputs = {
    # Use nixos-unstable so updates stay close to Hydra cache availability.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Track newest kernels for hardware that needs support before nixos-unstable catches up.
    nixpkgs-master.url = "github:NixOS/nixpkgs/master";

    # Home Manager following nixpkgs-unstable
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Disko for declarative disk partitioning
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Lumen desktop shell fork. This is pinned to the release tag whose source
    # includes the Nix package and local packaging workflow.
    lumen = {
      url = "github:DigitalPals/Lumen/v0.7.2";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    mdview = {
      url = "github:DigitalPals/mdview/0.0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    codex-desktop-linux = {
      url = "github:ilysenko/codex-desktop-linux";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    hermes-agent = {
      url = "github:NousResearch/hermes-agent";
      inputs.nixpkgs.follows = "nixpkgs";
    };

  };

  outputs = { self, nixpkgs, home-manager, disko, ... }@inputs:
  let
    system = "x86_64-linux";

    # Overlay to patch xdg-desktop-portal-gtk for Hyprland support
    gtkPortalOverlay = final: prev: {
      xdg-desktop-portal-gtk = prev.xdg-desktop-portal-gtk.overrideAttrs (old: {
        postInstall = (old.postInstall or "") + ''
          substituteInPlace $out/share/xdg-desktop-portal/portals/gtk.portal \
            --replace-fail "UseIn=gnome" "UseIn=gnome;Hyprland"
        '';
      });
    };

    localPackagesOverlay = final: prev: {
      conthrax = final.callPackage ./packages/conthrax { };
      intelLpmd = final.callPackage ./packages/intel-lpmd { };
      ipu7CameraBins = final.callPackage ./packages/ipu7-camera-bins { };
      ipu7CameraHal = final.callPackage ./packages/ipu7-camera-hal { };
      icamerasrcIpu75xa = final.callPackage ./packages/icamerasrc-ipu75xa {
        ipu7CameraHal = final.ipu7CameraHal;
      };
      lumen = inputs.lumen.packages.${system}.lumen;
      mdview = inputs.mdview.packages.${system}.mdview;
      codex-desktop = inputs.codex-desktop-linux.packages.${system}.default;
      hermes-desktop = inputs.hermes-agent.packages.${system}.desktop;

      # 1Password republished the 8.12.21 Linux tarball before nixpkgs caught up.
      _1password-gui =
        if final.lib.versionAtLeast prev._1password-gui.version "8.12.22" then
          prev._1password-gui
        else
          prev._1password-gui.overrideAttrs (old: {
            src = prev.fetchurl {
              url = builtins.head old.src.urls;
              hash = "sha256-JwiMi2iozP6jWSIUtgXla86aSAhuUob7snqtUbeXPpI=";
            };
          });
    };

    pkgs = import nixpkgs {
      inherit system;
      config.allowUnfree = true;
      overlays = [ gtkPortalOverlay localPackagesOverlay ];
    };
    pkgsMaster = import inputs.nixpkgs-master {
      inherit system;
      config.allowUnfree = true;
    };

    # Custom packages
    plymouth-cybex = pkgs.callPackage ./packages/plymouth-cybex { };
    forge = pkgs.callPackage ./packages/forge { };

    mkGeneratedHostModules = hostname:
      let
        hostDir = ./hosts + "/${hostname}";
      in
      builtins.filter builtins.pathExists [
        (hostDir + "/detected-hardware.nix")
        (hostDir + "/local.nix")
        (hostDir + "/installer.nix")
      ];

    mkInstallerProfileModule = { lib, config, ... }: {
      options.forge.installer.username = lib.mkOption {
        type = lib.types.str;
        default = "john";
        description = "Primary user name for the installed system.";
      };

      config._module.args.username = config.forge.installer.username;
    };

    # Home Manager configuration
    mkHomeManagerConfig = { hostname }: { config, ... }: {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;
      home-manager.backupFileExtension = "backup";
      # Avoid rebuild failures when a .backup file already exists.
      home-manager.overwriteBackup = true;
      home-manager.extraSpecialArgs = {
        inherit inputs hostname forge;
        username = config.forge.installer.username;
      };
      home-manager.users.${config.forge.installer.username} = import ./home/home.nix;
    };

    # Helper to create NixOS configurations
    # Set useDisko = false for hosts with manual partition setup (e.g., hibernate swap)
    mkNixosSystem = { hostname, extraModules ? [], useDisko ? true }:
      nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex forge pkgsMaster; };
        modules = [
          # Apply overlays to NixOS (for patched xdg-desktop-portal-gtk)
          { nixpkgs.overlays = [ gtkPortalOverlay localPackagesOverlay ]; }
          mkInstallerProfileModule
        ]
        # Disko for declarative disk partitioning (optional)
        ++ (if useDisko then [
          disko.nixosModules.disko
          ./modules/disko/${hostname}.nix
        ] else [])
        ++ [
          ./hosts/${hostname}
          ./modules/common.nix
          ./modules/desktop-environments.nix

          # Home Manager
          home-manager.nixosModules.home-manager
          (mkHomeManagerConfig { inherit hostname; })
        ]
        ++ (mkGeneratedHostModules hostname)
        ++ extraModules;
      };
  in
  {
    apps.${system} = {
      disko = {
        type = "app";
        program = "${disko.packages.${system}.disko}/bin/disko";
        meta.description = "Disko partitioning utility";
      };
      forge = {
        type = "app";
        program = "${forge}/bin/forge";
        meta.description = "Forge installer and system management tool";
      };
      default = {
        type = "app";
        program = "${forge}/bin/forge";
        meta.description = "Forge installer and system management tool";
      };
    };

    nixosConfigurations = {
      # Desktop with AMD Radeon RX 7700 XT / 7800 XT
      kraken = mkNixosSystem {
        hostname = "kraken";
      };

      # Threadripper workstation with AMD Radeon RX 7700 XT / 7800 XT
      thebeast = mkNixosSystem {
        hostname = "thebeast";
      };

      # HP ZBook Ultra G1a (AMD Strix Halo)
      G1a = mkNixosSystem {
        hostname = "G1a";
      };

      # HP Z2 Mini G1a Workstation (AMD Strix Halo)
      z2-mini-g1a = mkNixosSystem {
        hostname = "z2-mini-g1a";
      };

      # ASUS ProArt P16 OLED (AMD Ryzen AI 9 HX 370 + NVIDIA RTX 5090)
      proart = mkNixosSystem {
        hostname = "proart";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # Dell XPS 14 DA14260 (Intel Panther Lake)
      xps = mkNixosSystem {
        hostname = "xps";
        extraModules = [ ./modules/hardware/intel.nix ];
      };

      # Forge Installer ISO
      # Build: nix build .#nixosConfigurations.iso.config.system.build.isoImage
      iso = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex forge; };
        modules = [
          ./modules/iso
        ];
      };
    };

    packages.${system} = {
      disko = disko.packages.${system}.disko;
      forge = forge;
      conthrax = pkgs.conthrax;
      intelLpmd = pkgs.intelLpmd;
      ipu7CameraBins = pkgs.ipu7CameraBins;
      ipu7CameraHal = pkgs.ipu7CameraHal;
      icamerasrcIpu75xa = pkgs.icamerasrcIpu75xa;
      lumen = pkgs.lumen;
      mdview = pkgs.mdview;
      codex-desktop = pkgs.codex-desktop;
      hermes-desktop = pkgs.hermes-desktop;
      default = forge;
    };

    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [
        cargo
        rustc
        rustfmt
        clippy
        pkg-config
        dbus
      ];
    };
  };
}
