{ system ? builtins.currentSystem
, pkgs ? import <nixpkgs> { inherit system; }
}:
  with pkgs;
  with pkgs.lib;
let
  container2 = (import <nixpkgs/nixos/lib/eval-config.nix> {
    inherit system;
    modules = [{
      nixpkgs.system = system;
      boot.isContainer = true;
      networking.firewall.enable = false;
      security.audit.enable = false;
      services.postgresql.enable = true;
    }];
  }).config.system.build.toplevel;
in
{
  # The last /nix/store path is expanded into / instead of copied to
  # /nix/store. If we make it the system it'll be missing in /nix/store
  # but other scripts will be referencing it.
  container = symlinkJoin { name = "container"; paths = [ container2 ];};
}
