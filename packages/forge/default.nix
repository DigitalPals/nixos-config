{ lib, rustPlatform, pkg-config, makeWrapper, dbus, nvd }:

rustPlatform.buildRustPackage {
  pname = "forge";
  version = "1.0.0";

  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ pkg-config makeWrapper ];
  buildInputs = [ dbus ];

  postInstall = ''
    # Wrap forge with nvd in PATH
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
