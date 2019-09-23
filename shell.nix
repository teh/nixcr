with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = [
    rustc cargo
  ];
  buildInputs = [
    openssl
  ];
  RUST_BACKTRACE = 1;
}