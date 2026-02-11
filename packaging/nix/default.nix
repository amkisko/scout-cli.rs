# Standalone default.nix for Nix (non-flake) installs.
# Usage: nix-build -A scout-cli (when using flake) or adapt for nix-env.

{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage {
  pname = "scout-cli";
  version = "0.1.0";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  buildAndTestSubdir = "scout";
}
