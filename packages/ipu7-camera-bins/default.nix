{
  lib,
  stdenv,
  fetchFromGitHub,
  autoPatchelfHook,
  expat,
  zlib,
}:

stdenv.mkDerivation {
  pname = "ipu7-camera-bins";
  version = "unstable-2026-01-29";

  src = fetchFromGitHub {
    owner = "intel";
    repo = "ipu7-camera-bins";
    tag = "20260129_1800_11";
    hash = "sha256-Sj1jBOOegTk8tdmDN06MYEa7KmutnfSb5AEhXhoQkSc=";
  };

  nativeBuildInputs = [
    autoPatchelfHook
    (lib.getLib stdenv.cc.cc)
    expat
    zlib
  ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out
    cp --no-preserve=mode --recursive \
      include \
      lib \
      $out/

    runHook postInstall
  '';

  postFixup = ''
    for lib in $out/lib/lib*.so.*; do
      [ -f "$lib" ] || continue
      base="''${lib##*/}"
      ln -sf "$base" "$out/lib/''${base%.*}"
    done

    for pcfile in $out/lib/pkgconfig/*.pc; do
      [ -f "$pcfile" ] || continue
      substituteInPlace "$pcfile" --replace 'prefix=/usr' "prefix=$out"
    done
  '';

  meta = {
    description = "Intel IPU7 firmware and proprietary image processing libraries";
    homepage = "https://github.com/intel/ipu7-camera-bins";
    license = lib.licenses.issl;
    sourceProvenance = with lib.sourceTypes; [ binaryFirmware ];
    maintainers = [ ];
    platforms = [ "x86_64-linux" ];
  };
}
