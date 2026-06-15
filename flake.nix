{

  description = "Meon dev env";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forEachSystem = fn: nixpkgs.lib.genAttrs supportedSystems (system: fn system);
    in {
      devShells = forEachSystem (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          rust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
            extensions = [ "rust-src" "rust-analyzer" "clippy" ];
          });
        in {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              rust
              gcc
              pkg-config
              cargo-fuzz
              cargo-expand
            ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.linuxPackages.perf
            ] ++ [
              cargo-flamegraph
              samply
              heaptrack
              gnuplot
              cargo-watch
            ];

            shellHook = ''
                cchm() {
                    RUSTFLAGS="-Z macro-backtrace" cargo check "$@"
                }

              cat <<EOF
              Aliases:
                - cchm -> RUSTFLAGS="-Z macro-backtrace" cargo check
              EOF
            '';

            RUST_BACKTRACE = "1";
          };
        }
      );
    };

}
