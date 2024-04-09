{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          # src = craneLib.cleanCargoSource ./.; # filter out md files which are used in docs
          src = nixpkgs.lib.cleanSource ./.;

          nativeBuildInputs = with pkgs; [ rustToolchain clang ];
          buildInputs = with pkgs; [ ];
          commonArgs = {
            inherit src buildInputs nativeBuildInputs;
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib"; # for rocksdb

            # link rocksdb dynamically
            ROCKSDB_INCLUDE_DIR = "${pkgs.rocksdb}/include";
            ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
            
            cargoExtraArgs = "--all-features";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          bin = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
          });
        in
        with pkgs;
        {
          packages = {
            inherit bin;
            default = bin;
            blocks_iterator = bin;
          };
          devShells.default = mkShell {
            inputsFrom = [ bin ];

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib"; # for rocksdb

            # link rocksdb dynamically
            ROCKSDB_INCLUDE_DIR = "${pkgs.rocksdb}/include";
            ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";

            buildInputs = with pkgs; [ ];
          };
        }
      );
}
