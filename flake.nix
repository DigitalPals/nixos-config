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

    # Portal - SSH connection manager (uses own nixpkgs to match cachix builds)
    # Points to release branch for stable builds with cachix cache hits
    portal.url = "github:DigitalPals/portal/release";

  };

  outputs = { self, nixpkgs, home-manager, noctalia, disko, portal, ... }@inputs:
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

    pkgs = import nixpkgs {
      inherit system;
      overlays = [ gtkPortalOverlay ];
    };

    # Custom packages
    plymouth-cybex = pkgs.callPackage ./packages/plymouth-cybex { };
    forge = pkgs.callPackage ./packages/forge { };

    # Home Manager configuration
    mkHomeManagerConfig = { hostname, username }: {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;
      home-manager.backupFileExtension = "backup";
      # Avoid rebuild failures when a .backup file already exists.
      home-manager.overwriteBackup = true;
      home-manager.extraSpecialArgs = { inherit inputs hostname username forge portal; };
      home-manager.users.${username} = import ./home/home.nix;
    };

    # Helper to create NixOS configurations
    # Set useDisko = false for hosts with manual partition setup (e.g., hibernate swap)
    mkNixosSystem = { hostname, username ? "john", extraModules ? [], useDisko ? true }:
      nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex forge username; };
        modules = [
          # Apply overlays to NixOS (for patched xdg-desktop-portal-gtk)
          { nixpkgs.overlays = [ gtkPortalOverlay ]; }
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
          (mkHomeManagerConfig { inherit hostname username; })
        ] ++ extraModules;
      };
  in
  {
    apps.${system} = {
      disko = {
        type = "app";
        program = "${disko.packages.${system}.disko}/bin/disko";
      };
      forge = {
        type = "app";
        program = "${forge}/bin/forge";
      };
      default = {
        type = "app";
        program = "${forge}/bin/forge";
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
      default = forge;
    };
  };
}
