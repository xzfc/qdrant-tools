let
  pkgs = import <nixpkgs> { };
in
pkgs.mkShell {
  buildInputs = [
    pkgs.bear
    pkgs.cargo
    # pkgs.clang
    pkgs.clang-tools
    pkgs.clang.cc
    pkgs.clippy
    pkgs.elfutils
    pkgs.pkg-config
    pkgs.rust-analyzer
    pkgs.rustPlatform.rust.rustc # TODO: evaluation warning: rustPlatform.rust.rustc is deprecated. Use rustc instead.
    pkgs.rustfmt
    pkgs.zlib
  ];

  env = {
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}"; # for rust-analyzer
  };
}
