{
  description = "SpacePanda - A Rust project with Nix development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "rustfmt" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo
            rustc
            rust-analyzer
            cargo-watch
            cargo-edit
            cargo-outdated
            
            # Build dependencies
            pkg-config
            openssl
            
            # gRPC/Protobuf
            protobuf
            
            # Development tools
            git
            nixpkgs-fmt
          ];

          shellHook = ''
            echo "🐼 SpacePanda Development Environment"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build       - Build the project"
            echo "  cargo test        - Run tests"
            echo "  cargo fmt         - Format code with rustfmt"
            echo "  cargo fmt -- --check - Check formatting without changes"
            echo "  cargo run --bin spacepanda - Run the CLI"
            echo "  cargo watch -x test - Watch and run tests on changes"
            echo ""
          '';

          RUST_BACKTRACE = "1";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
