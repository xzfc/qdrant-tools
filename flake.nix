{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    poetry2nix.url = "github:nix-community/poetry2nix";

    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    poetry2nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    inputs@{
      fenix,
      flake-parts,
      nixpkgs,
      poetry2nix,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        { pkgs, system, ... }:
        let
	  versions = builtins.fromJSON (builtins.readFile ./versions.json);

          rust-combined =
            let
              stable = fenix.packages.${system}.toolchainOf {
                channel = versions.stable.version;
                sha256 = versions.stable.sha256;
              };
              nightly = fenix.packages.${system}.toolchainOf {
                channel = "nightly";
                date = versions.nightly.date;
                sha256 = versions.nightly.sha256;
              };
            in
            fenix.packages.${system}.combine [
              nightly.rustfmt # should be the first
              stable.rust
              stable.rust-analyzer
              stable.rust-src
            ];

          # Python dependencies used in tests
          poetry2nix' = poetry2nix.lib.mkPoetry2Nix { inherit pkgs; };
          qdrant-python-env = poetry2nix'.mkPoetryEnv {
            projectDir = ./poetry;

            # 1. Wheels speed up building of the environment
            # 2. PyYAML/cython3 issues: https://github.com/nix-community/poetry2nix/pull/1559
            preferWheels = true;

            overrides = poetry2nix'.overrides.withDefaults (
              _self: super: {
                hypothesis-graphql = super.hypothesis-graphql.overridePythonAttrs (old: {
                  buildInputs = old.buildInputs ++ [ super.hatchling ];
                });
                grpc-requests = super.grpc-requests.overridePythonAttrs (old: {
                  buildInputs = old.buildInputs ++ [ super.setuptools ];
                });
              }
            );
          };

          # A workaround to allow running `cargo +nightly fmt`
          cargo-wrapper = pkgs.writeScriptBin "cargo" ''
            #!${pkgs.stdenv.shell}
            [ "$1" != "+nightly" ] || [ "$2" != "fmt" ] || shift
            exec ${rust-combined}/bin/cargo "$@"
          '';

          # Cache to speed up builds
          caching-hook = ''
            # For C/C++ deps, particularly for librocksdb-sys
            PATH="${pkgs.ccache}/bin:$PATH"
            export CC="ccache $CC"
            export CXX="ccache $CXX"

            PATH="${pkgs.sccache}/bin:$PATH"
            export RUSTC_WRAPPER="sccache"

            # For lindera-unidic
            [ "''${LINDERA_CACHE+x}" ] ||
              export LINDERA_CACHE="''${XDG_CACHE_HOME:-$HOME/.cache}/lindera"
          '';

          # Use mold linker to speed up builds
          mkShellMold = pkgs.mkShell.override {
            stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;
          };

          qdrant-rust-inputs = [
            cargo-wrapper # should be before rust-combined
            rust-combined

            # For deps
            pkgs.gnuplot # optional runtime dep for criterion
            pkgs.libunwind # for unwind-sys
            pkgs.pkg-config # for unwind-sys and other deps
            pkgs.protobuf # for prost-wkt-types
            pkgs.rustPlatform.bindgenHook # for bindgen deps
          ];
        in
        {
          # Allow using RustRover unfree package
          _module.args.pkgs = import nixpkgs {
            inherit system;
            config.allowUnfree = true;
          };

          formatter = pkgs.nixfmt-rfc-style;

          # Environment to develop qdrant
          devShells.default = mkShellMold {
            buildInputs = qdrant-rust-inputs ++ [
              pkgs.cargo-nextest # mentioned in .github/workflows/rust.yml
              pkgs.curl # used in tests
              pkgs.grpcurl # mentioned in QUICK_START_GRPC.md
              pkgs.wget # used in tests/storage-compat
              pkgs.yq-go # used in tools/generate_openapi_models.sh
              pkgs.ytt # used in tools/generate_openapi_models.sh
              qdrant-python-env # used in tests
            ];
            shellHook = caching-hook;
          };

          # Environment to run RustRover
          # TODO: make a runnable package, not a shell
          devShells.jb = mkShellMold {
            buildInputs = qdrant-rust-inputs ++ [ pkgs.jetbrains.rust-rover ];
            shellHook = caching-hook;
          };

          # Enviroment to maintain this repo contents
          devShells.dev = pkgs.mkShell {
            buildInputs = [
              pkgs.black # ./Justfile
              pkgs.deadnix # ./Justfile
              pkgs.isort # ./Justfile
              pkgs.just # for running Justfile
              pkgs.mypy # ./Justfile
              pkgs.nixfmt-rfc-style # ./Justfile
              pkgs.poetry # ./poetry/regen.py
            ];
          };
        };
    };
}
