with import (fetchTarball {
  url = https://github.com/NixOS/nixpkgs/archive/9691c53afc8f09a5d5a8acb50a2c2d56c0da6e10.tar.gz;
  sha256 = "0j295k1gxnmx1wc88b4gn869qih6nnrkrqzj3kmp04cz2dk6ds50";
}) {};

stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = [
    rustc cargo rustPackages.clippy
    rustPackages.rustfmt
  ];
  buildInputs = [
    openssl
  ];
  propagatedBuildInputs = [
    nix
    git
    openssh
  ];
  RUST_BACKTRACE = 1;
}