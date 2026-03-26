{ lib, stdenv, fetchFromGitHub }:

stdenv.mkDerivation rec {
  pname = "asdcontrol";
  version = "0.4-xdr";

  src = fetchFromGitHub {
    owner = "nikosdion";
    repo = "asdcontrol";
    rev = "0ee8bd576d4e93513027d713e688988cb0d827ef";
    hash = "sha256-0BC1TKGRvCP62IhNmUcASZY+jzO1/O/Cupy0f7zqeBw=";
  };

  # Add support for Apple Studio Display XDR (USB product ID 0x1116)
  postPatch = ''
    substituteInPlace asdcontrol.cpp \
      --replace-fail \
        'const int PRO_XDR_DISPLAY_32              = 0x9243;' \
        'const int PRO_XDR_DISPLAY_32              = 0x9243;
const int STUDIO_DISPLAY_XDR              = 0x1116;'
    substituteInPlace asdcontrol.cpp \
      --replace-fail \
        'supportedDevices.insert ( DeviceId ( APPLE, STUDIO_DISPLAY_27,
                                         "Apple Studio Display (2022, 27\")", 400, 60000 ) );
}' \
        'supportedDevices.insert ( DeviceId ( APPLE, STUDIO_DISPLAY_27,
                                         "Apple Studio Display (2022, 27\")", 400, 60000 ) );

    supportedDevices.insert ( DeviceId ( APPLE, STUDIO_DISPLAY_XDR,
                                         "Apple Studio Display XDR", 400, 60000 ) );
}'
  '';

  buildPhase = ''
    runHook preBuild
    g++ -O2 asdcontrol.cpp -o asdcontrol
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    install -Dm755 asdcontrol $out/bin/asdcontrol
    runHook postInstall
  '';

  meta = with lib; {
    description = "Apple Studio Display brightness control for Linux";
    homepage = "https://github.com/nikosdion/asdcontrol";
    license = licenses.gpl2Plus;
    platforms = platforms.linux;
    mainProgram = "asdcontrol";
  };
}
