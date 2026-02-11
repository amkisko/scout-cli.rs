{
  description = "ScoutAPM CLI — query apps, endpoints, traces, and metrics";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "scout-cli";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = self + "/Cargo.lock";
          buildAndTestSubdir = "scout";
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/scout";
        };
      });
}
