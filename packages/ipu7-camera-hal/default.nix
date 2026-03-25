{
  lib,
  stdenv,
  fetchFromGitHub,
  cmake,
  pkg-config,
  expat,
  gst_all_1,
  ipu7CameraBins,
  jsoncpp,
  libdrm,
  libtool,
}:

stdenv.mkDerivation {
  pname = "ipu7-camera-hal";
  version = "unstable-2026-01-29";

  src = fetchFromGitHub {
    owner = "intel";
    repo = "ipu7-camera-hal";
    tag = "20260129_1800_11";
    hash = "sha256-fz3ALh2F57NWYU6D1XuKfAzES2754GfZr1xQBwfkG3U=";
  };

  nativeBuildInputs = [
    cmake
    pkg-config
  ];

  buildInputs = [
    expat
    gst_all_1.gst-plugins-base
    gst_all_1.gstreamer
    ipu7CameraBins
    jsoncpp
    libdrm
    libtool
  ];

  cmakeFlags = [
    "-DCMAKE_BUILD_TYPE=Release"
    "-DCMAKE_INSTALL_PREFIX=${placeholder "out"}"
    "-DCMAKE_INSTALL_LIBDIR=lib"
    "-DCMAKE_POLICY_VERSION_MINIMUM=3.5"
    "-DBUILD_CAMHAL_ADAPTOR=ON"
    "-DBUILD_CAMHAL_PLUGIN=ON"
    "-DIPU_VERSIONS=ipu75xa"
    "-DUSE_STATIC_GRAPH=ON"
    "-DUSE_STATIC_GRAPH_AUTOGEN=ON"
  ];

  NIX_CFLAGS_COMPILE = [
    "-Wno-error"
  ];

  enableParallelBuilding = true;

  postPatch = ''
    substituteInPlace src/platformdata/PlatformData.h \
      --replace '/usr/share/' "${placeholder "out"}/share/" \
      --replace '#define CAMERA_DEFAULT_CFG_PATH "/etc/camera/"' '#define CAMERA_DEFAULT_CFG_PATH "${placeholder "out"}/etc/camera/"'

    substituteInPlace src/platformdata/JsonParserBase.h \
      --replace '<jsoncpp/json/json.h>' '<json/json.h>'
  '';

  postInstall = ''
    mkdir -p $out/include/ipu75xa
    cp -r $src/include $out/include/ipu75xa/libcamhal

    sed -i 's|^includedir=.*|includedir=''${prefix}/include/libcamhal|' \
      $out/lib/pkgconfig/libcamhal.pc
  '';

  postFixup = ''
    for lib in $out/lib/*.so $out/lib/*.so.* $out/lib/libcamhal/plugins/*.so $out/lib/libcamhal/plugins/*.so.*; do
      [ -f "$lib" ] || continue
      patchelf --add-rpath "${ipu7CameraBins}/lib:$out/lib" "$lib"
    done
  '';

  passthru = {
    ipuVersion = "ipu75xa";
    ipuTarget = "ipu75xa";
  };

  meta = {
    description = "HAL for Intel IPU7 camera processing";
    homepage = "https://github.com/intel/ipu7-camera-hal";
    license = lib.licenses.asl20;
    maintainers = [ ];
    platforms = [ "x86_64-linux" ];
  };
}
