{
  lib,
  stdenv,
  fetchFromGitHub,
  autoreconfHook,
  autoconf-archive,
  gtk-doc,
  pkg-config,
  glib,
  libxml2,
  libnl,
  systemd,
  upower,
}:

stdenv.mkDerivation {
  pname = "intel-lpmd";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "intel";
    repo = "intel-lpmd";
    tag = "v0.1.0";
    hash = "sha256-eZBgWpR2tdSDeqYV4Y2h2j5UeJebQg2tXlXcUywwZEA=";
  };

  nativeBuildInputs = [
    autoreconfHook
    autoconf-archive
    gtk-doc
    pkg-config
  ];

  buildInputs = [
    glib
    libnl
    libxml2
    systemd
    upower
  ];

  configureFlags = [
    "--sbindir=${placeholder "out"}/bin"
    "--with-systemdsystemunitdir=${placeholder "out"}/lib/systemd/system"
    "--with-dbus-sys-dir=${placeholder "out"}/etc/dbus-1/system.d"
  ];

  meta = {
    description = "Intel Low Power Mode Daemon";
    homepage = "https://github.com/intel/intel-lpmd";
    license = lib.licenses.gpl2Plus;
    maintainers = [ ];
    platforms = [ "x86_64-linux" ];
  };
}
