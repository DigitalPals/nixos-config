{ lib, rustPlatform, pkg-config, makeWrapper, openssl, nvd }:

rustPlatform.buildRustPackage {
  pname = "forge";
  version = "1.0.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ pkg-config makeWrapper ];
  buildInputs = [ openssl ];

  postInstall = ''
    wrapProgram $out/bin/forge \
      --prefix PATH : ${lib.makeBinPath [ nvd ]}
  '';

  meta = {
    description = "NixOS Configuration Tool - Copyright Cybex B.V.";
    homepage = "https://github.com/DigitalPals/nixos-config";
    license = lib.licenses.mit;
    mainProgram = "forge";
  };
}
