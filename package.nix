with import (fetchTarball {
  url = https://github.com/NixOS/nixpkgs/archive/9691c53afc8f09a5d5a8acb50a2c2d56c0da6e10.tar.gz;
  sha256 = "0j295k1gxnmx1wc88b4gn869qih6nnrkrqzj3kmp04cz2dk6ds50";
}) {};
rec {
  bin = rustPlatform.buildRustPackage rec {
    name = "nixcr-${version}";
    version = "0.0.1";

    cargoSha256 = "1r204xda35yqp19w29w1w7l63bpqrhw0g31scj3n37j71hxqm7is";
    src = nix-gitignore.gitignoreSource [] ./.;
  };
  # nix-build package.nix -A image
  # docker load <result
  # docker push eu.gcr.io/mm-boogle/nixcr:commit
  image = dockerTools.buildLayeredImage {
    name = "eu.gcr.io/mm-boogle/nixcr";
    tag = "c3ee455";
    contents = [
      bin
      bash
    ];
  };
}
