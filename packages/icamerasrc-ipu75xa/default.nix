{
  lib,
  stdenv,
  fetchFromGitHub,
  autoreconfHook,
  pkg-config,
  gst_all_1,
  ipu7CameraHal,
  libdrm,
  libva,
  apple-sdk_gstreamer ? null,
}:

stdenv.mkDerivation {
  pname = "icamerasrc-${ipu7CameraHal.ipuVersion}";
  version = "unstable-2026-02-17";

  src = fetchFromGitHub {
    owner = "intel";
    repo = "icamerasrc";
    rev = "4fb31db76b618aae72184c59314b839dedb42689";
    hash = "sha256-BYURJfNz4D8bXbSeuWyUYnoifozFOq6rSfG9GBKVoHo=";
  };

  nativeBuildInputs = [
    autoreconfHook
    pkg-config
  ];

  preConfigure = ''
    export CHROME_SLIM_CAMHAL=ON
    export STRIP_VIRTUAL_CHANNEL_CAMHAL=ON
  '';

  buildInputs = [
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-base
    gst_all_1.gstreamer
    ipu7CameraHal
    libdrm
    libva
  ]
  ++ lib.optionals stdenv.hostPlatform.isDarwin [
    apple-sdk_gstreamer
  ];

  NIX_CFLAGS_COMPILE = [
    "-Wno-error"
    "-I${gst_all_1.gst-plugins-base.dev}/include/gstreamer-1.0"
  ];

  enableParallelBuilding = true;

  passthru = {
    inherit (ipu7CameraHal) ipuVersion;
  };

  meta = {
    description = "GStreamer plugin for Intel IPU7 cameras";
    homepage = "https://github.com/intel/icamerasrc/tree/icamerasrc_slim_api";
    license = lib.licenses.lgpl21Plus;
    maintainers = [ ];
    platforms = [ "x86_64-linux" ];
  };
}
