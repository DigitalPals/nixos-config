{ lib, rustPlatform, pkg-config }:

rustPlatform.buildRustPackage {
  pname = "forge";
  version = "1.0.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ pkg-config ];

  meta = {
    description = "NixOS Configuration Tool - Copyright Cybex B.V.";
    homepage = "https://github.com/DigitalPals/nixos-config";
    license = lib.licenses.mit;
    mainProgram = "forge";
  };
}
