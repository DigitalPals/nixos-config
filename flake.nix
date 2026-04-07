{
  description = "NixOS configuration with Home Manager, Hyprland, and Noctalia Desktop Shell";

  inputs = {
    # Use nixpkgs-unstable for compatibility
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Home Manager following nixpkgs-unstable
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Noctalia Desktop Shell
    noctalia = {
      url = "github:noctalia-dev/noctalia-shell";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Disko for declarative disk partitioning
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };

  };

  outputs = { self, nixpkgs, home-manager, noctalia, disko, ... }@inputs:
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

    xpsHardwareOverlay = final: prev: {
      intelLpmd = final.callPackage ./packages/intel-lpmd { };
      ipu7CameraBins = final.callPackage ./packages/ipu7-camera-bins { };
      ipu7CameraHal = final.callPackage ./packages/ipu7-camera-hal { };
      icamerasrcIpu75xa = final.callPackage ./packages/icamerasrc-ipu75xa {
        ipu7CameraHal = final.ipu7CameraHal;
      };
    };

    pkgs = import nixpkgs {
      inherit system;
      config.allowUnfree = true;
      overlays = [ gtkPortalOverlay xpsHardwareOverlay ];
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
        specialArgs = { inherit inputs plymouth-cybex forge; };
        modules = [
          # Apply overlays to NixOS (for patched xdg-desktop-portal-gtk)
          { nixpkgs.hostPlatform = system; nixpkgs.overlays = [ gtkPortalOverlay xpsHardwareOverlay ]; }
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
          ./modules/shell-config.nix
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
      # Desktop with NVIDIA RTX 5090
      kraken = mkNixosSystem {
        hostname = "kraken";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # HP ZBook Ultra G1a (AMD Strix Halo)
      G1a = mkNixosSystem {
        hostname = "G1a";
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
        specialArgs = { inherit inputs plymouth-cybex; };
        modules = [
          ./modules/iso
        ];
      };
    };

    packages.${system} = {
      disko = disko.packages.${system}.disko;
      forge = forge;
      intelLpmd = pkgs.intelLpmd;
      ipu7CameraBins = pkgs.ipu7CameraBins;
      ipu7CameraHal = pkgs.ipu7CameraHal;
      icamerasrcIpu75xa = pkgs.icamerasrcIpu75xa;
      default = forge;
    };
  };
}
