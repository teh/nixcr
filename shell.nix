with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = [
    rustc cargo rustPackages.clippy
    rustPackages.rustfmt
  ];
  buildInputs = [
    openssl
  ];
  RUST_BACKTRACE = 1;
}