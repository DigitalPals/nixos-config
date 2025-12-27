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

    # Home Manager configuration (shell-agnostic - shell comes from osConfig)
    mkHomeManagerConfig = { hostname }: {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;
      home-manager.backupFileExtension = "backup";
      home-manager.extraSpecialArgs = { inherit inputs hostname dots-hyprland rounded-polygon-qmljs quickshell forge; };
      home-manager.users.john = import ./home/home.nix;
      # sharedModules removed - external modules now imported conditionally in home.nix
    };

    # Helper to create NixOS configurations with shell specialisations
    mkNixosSystem = { hostname, extraModules ? [] }:
      nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex forge; };
        modules = [
          # Apply overlay for patched xdg-desktop-portal-gtk
          { nixpkgs.overlays = [ gtkPortalOverlay ]; }
          # Disko for declarative disk partitioning
          disko.nixosModules.disko
          ./modules/disko/${hostname}.nix

          ./hosts/${hostname}
          ./modules/common.nix
          ./modules/shell-config.nix
          ./modules/desktop-environments.nix

          # Home Manager
          home-manager.nixosModules.home-manager
          (mkHomeManagerConfig { inherit hostname; })

          # Shell specialisations (boot menu entries)
          {
            specialisation = {
              illogical.configuration.desktop.shell = "illogical";
              caelestia.configuration.desktop.shell = "caelestia";
            };
          }
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
      # Default: Noctalia | Specialisations: illogical, caelestia
      kraken = mkNixosSystem {
        hostname = "kraken";
        extraModules = [ ./modules/hardware/nvidia.nix ];
      };

      # HP ZBook Ultra G1a (AMD Strix Halo)
      # Default: Noctalia | Specialisations: illogical, caelestia
      G1a = mkNixosSystem {
        hostname = "G1a";
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
