{
  description = "NixOS configuration with Home Manager, Hyprland, and Noctalia";

  inputs = {
    # Use nixpkgs-unstable for Noctalia compatibility
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

    # Claude Code (latest version, auto-updated hourly)
    claude-code = {
      url = "github:sadjow/claude-code-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, home-manager, noctalia, disko, claude-code, ... }@inputs:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};

    # Custom packages
    plymouth-cybex = pkgs.callPackage ./packages/plymouth-cybex { };

    # Shared Home Manager configuration (takes hostname as parameter)
    mkHomeManagerConfig = hostname: {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;
      home-manager.backupFileExtension = "backup";
      home-manager.extraSpecialArgs = { inherit inputs hostname; };
      home-manager.users.john = import ./home/home.nix;
      home-manager.sharedModules = [
        noctalia.homeModules.default
      ];
    };

    # Claude Code package
    claude-code-pkg = claude-code.packages.${system}.claude-code;
  in
  {
    nixosConfigurations = {
      # Desktop with NVIDIA RTX 5090
      kraken = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex claude-code-pkg; };
        modules = [
          # Disko for declarative disk partitioning
          disko.nixosModules.disko
          ./modules/disko/kraken.nix

          ./hosts/kraken
          ./modules/common.nix
          ./modules/desktop-environments.nix
          ./modules/hardware/nvidia.nix

          # Home Manager
          home-manager.nixosModules.home-manager
          (mkHomeManagerConfig "kraken")
        ];
      };

      # HP ZBook Ultra G1a
      G1a = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = { inherit inputs plymouth-cybex claude-code-pkg; };
        modules = [
          # Disko for declarative disk partitioning
          disko.nixosModules.disko
          ./modules/disko/G1a.nix

          ./hosts/G1a
          ./modules/common.nix
          ./modules/desktop-environments.nix
          # No nvidia.nix - uses AMD GPU via mesa

          # Home Manager
          home-manager.nixosModules.home-manager
          (mkHomeManagerConfig "G1a")
        ];
      };
    };
  };
}
