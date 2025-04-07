{
  description = "Rust development shell template, can be used with 'nix develop' or 'nix shell'";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    rec {
      dependencies = with pkgs; [
        rustc
        cargo
        gcc
        rustfmt
        clippy
        rust-analyzer
      ];

      devShells.${system}.default = pkgs.mkShell {
        buildInputs = dependencies;
        shellHook = "echo 'Rust shell init complete.'";
      };

      default = pkgs.rustPlatform.buildRustPackage rec {
        pname = "file-server";
        version = "0.1.0";
        src = pkgs.lib.cleanSource ./.;
        nativeBuildInputs = dependencies;

        cargoLock.lockFile = "${src}/Cargo.lock";
        # phases = [ "buildPhase" "installPhase" ];
        # buildPhase = "cargo build --release";
        # installPhase = "cp $src/target/release/file_server $out/bin/fileserver";
      };

      packages.${system}.default = default;

      RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    };
}
