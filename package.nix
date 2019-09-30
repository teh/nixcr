with import (fetchTarball {
  url = https://github.com/NixOS/nixpkgs/archive/9691c53afc8f09a5d5a8acb50a2c2d56c0da6e10.tar.gz;
  sha256 = "0j295k1gxnmx1wc88b4gn869qih6nnrkrqzj3kmp04cz2dk6ds50";
}) {};
let
naersk = callPackage (fetchTarball {
  url = https://github.com/nmattia/naersk/archive/master.tar.gz;
  sha256 = "0890hg9ngyh4y7cy68ii0k3chicixcnbg0scdcr6c2ig35wl35z1";
});

baseLayout = runCommandNoCC "baseLayout" { nativeBuildInputs = [ shadow ]; } ''
  set -e
  mkdir -p $out
  mkdir $out/tmp && chmod 1777 $out/tmp
  mkdir $out/etc

  # https://nixos.wiki/wiki/Install_Nix_in_multi-user_mode_on_non-NixOS

  echo -en 'nixbld:x:30000:nixbld1' > $out/etc/group
  for i in $(seq 2 10); do
    echo -en ",nixbld$i" >> $out/etc/group
  done
  echo "" >> $out/etc/group

  echo 'root:x:0:0:System administrator:/:/bin/bash' > $out/etc/passwd
  for i in $(seq 1 10); do
    echo "nixbld$i:x:30001:30000:Nix build user $i:/empty:/bin/nologin" >> $out/etc/passwd
  done
'';
nixcr-source = (nix-gitignore.gitignoreSource [ "package.nix" "k8" "shell.nix" "README.md" ] ./.);
in rec {
  bin = naersk.buildPackage nixcr-source rec {
    name = "nixcr-${version}";
    version = "0.0.1";
    doCheck = false;
  };
  # nix-build package.nix -A image
  # docker load <result
  # docker push eu.gcr.io/mm-boogle/nixcr:commit
  image = dockerTools.buildLayeredImage {
    name = "eu.gcr.io/mm-boogle/nixcr";
    tag = "c3ee455";
    config = {
      Env = [
        "PATH=/bin/"
        "GIT_SSL_CAINFO=${cacert}/etc/ssl/certs/ca-bundle.crt"
        "NIX_SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
    contents = [
      bin
      baseLayout
      bash
      # propagatedBuildInputs doesn't seem to work for buildRustPackage
      # so add nix, git etc below here:
      nix
      gnutar
      cacert
      gzip # to unpack nixpkg tarballs
      iana-etc # needed not sure why!
      # git with perl and python is huge, breaks without perl (/usr/bin/perl: No such file or directory)
      (git.override { withManual = false; pythonSupport = false; })
      openssh
      coreutils
    ];
  };
}
