{
  description = "NixOS configuration with Home Manager, Hyprland, and multi-shell support";

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

    # Illogical Impulse dotfiles (direct from upstream)
    dots-hyprland = {
      url = "github:end-4/dots-hyprland";
      flake = false;
    };

    # Rounded polygon shapes submodule for dots-hyprland
    rounded-polygon-qmljs = {
      url = "github:end-4/rounded-polygon-qmljs";
      flake = false;
    };

    # Caelestia Desktop Shell
    caelestia = {
      url = "github:caelestia-dots/shell";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Quickshell (latest git for IdleInhibitor support)
    quickshell = {
      url = "github:quickshell-mirror/quickshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # COSMIC Desktop Environment (temporarily disabled - upstream hash mismatch)
    # nixos-cosmic = {
    #   url = "github:lilyinstarlight/nixos-cosmic";
    #   inputs.nixpkgs.follows = "nixpkgs";
    # };

    # Disko for declarative disk partitioning
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, home-manager, noctalia, caelestia, dots-hyprland, rounded-polygon-qmljs, disko, quickshell, ... }@inputs:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};

    # Custom packages
    plymouth-cybex = pkgs.callPackage ./packages/plymouth-cybex { };

    # Shell-aware Home Manager configuration
    mkHomeManagerConfig = { hostname, shell ? "noctalia" }: {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;
      home-manager.backupFileExtension = "backup";
      home-manager.extraSpecialArgs = { inherit inputs hostname shell dots-hyprland rounded-polygon-qmljs quickshell; };
      home-manager.users.john = import ./home/home.nix;
      home-manager.sharedModules =
        if shell == "illogical" then [
          # Illogical Impulse is self-contained in home/shells/illogical/
        ] else if shell == "caelestia" then [
          caelestia.homeManagerModules.default
        ] else [
          noctalia.homeModules.default
        ];
    };

    # Helper to create NixOS configurations for host+shell combinations
    mkNixosSystem = { hostname, shell ? "noctalia", extraModules ? [] }:
      nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex shell; };
        modules = [
          # Disko for declarative disk partitioning
          disko.nixosModules.disko
          ./modules/disko/${hostname}.nix

          ./hosts/${hostname}
          ./modules/common.nix
          ./modules/desktop-environments.nix

          # Home Manager
          home-manager.nixosModules.home-manager
          (mkHomeManagerConfig { inherit hostname shell; })
        ] ++ extraModules;
      };
  in
  {
    apps.${system}.disko = {
      type = "app";
      program = "${disko.packages.${system}.disko}/bin/disko";
    };

    nixosConfigurations = {
      # Desktop with NVIDIA RTX 5090 - Noctalia (default)
      kraken = mkNixosSystem {
        hostname = "kraken";
        shell = "noctalia";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # Desktop with NVIDIA RTX 5090 - Illogical Impulse
      kraken-illogical = mkNixosSystem {
        hostname = "kraken";
        shell = "illogical";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # HP ZBook Ultra G1a - Noctalia (default)
      G1a = mkNixosSystem {
        hostname = "G1a";
        shell = "noctalia";
      };

      # HP ZBook Ultra G1a - Illogical Impulse
      G1a-illogical = mkNixosSystem {
        hostname = "G1a";
        shell = "illogical";
      };

      # Desktop with NVIDIA RTX 5090 - Caelestia
      kraken-caelestia = mkNixosSystem {
        hostname = "kraken";
        shell = "caelestia";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # HP ZBook Ultra G1a - Caelestia
      G1a-caelestia = mkNixosSystem {
        hostname = "G1a";
        shell = "caelestia";
      };
    };

    packages.${system}.disko = disko.packages.${system}.disko;
  };
}
