{
  pkgs,
  inputs,
  ...
}:
let
  rust-bin = inputs.rust-overlay.lib.mkRustBin { } pkgs;
  nightly = rust-bin.selectLatestNightlyWith (
    toolchain:
    toolchain.minimal.override {
      extensions = [
        "miri"
        "rust-src"
      ];
    }
  );
  msrv = rust-bin.stable."1.92.0".minimal;
in
{
  languages.rust = {
    enable = true;
    toolchainFile = ./rust-toolchain.toml;
  };

  packages = [
    pkgs.cargo-hack
    pkgs.cargo-nextest
    pkgs.cargo-show-asm
    pkgs.perf
    pkgs.samply
    pkgs.valgrind
  ];

  scripts = {
    miri.exec = ''PATH=${nightly}/bin:$PATH cargo miri "$@"'';
    msrv.exec = ''PATH=${msrv}/bin:$PATH cargo "$@"'';
  };
}
