{
  description = "systemd-status-leds - Monitor systemd service status on WS281x LED strips";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    mcp-nixos.url = "github:utensils/mcp-nixos";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, mcp-nixos }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        # Build inputs for the application
        buildInputs = with pkgs; [
          # System dependencies
          pkg-config
          systemd
          dbus
          
          # Development tools
          rustToolchain
          cargo-watch
          cargo-edit
          cargo-auditable  # For embedding audit info in binaries
          
          # Testing tools
          cargo-tarpaulin  # For coverage
          
          # MCP NixOS for AI-assisted development
          mcp-nixos.packages.${system}.default
        ];

        # Native build inputs (build-time dependencies)
        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        # Rust package
        rustPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "systemd-status-leds";
          version = "0.1.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          inherit buildInputs nativeBuildInputs;
          
          # Environment variables for building
          PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          
          # Run tests during build
          checkPhase = ''
            cargo test --release
          '';
          
          # Install the binary and config
          installPhase = ''
            mkdir -p $out/bin
            mkdir -p $out/etc/systemd-status-leds
            
            # Install binary with embedded audit info
            cargo auditable install --path . --root $out
            
            # Install example configuration
            cp config.yaml $out/etc/systemd-status-leds/config.yaml.example
          '';
          
          meta = with pkgs.lib; {
            description = "Monitor systemd service status and display on WS281x RGBW LED strips";
            homepage = "https://github.com/shift/systemd-status-leds";
            license = licenses.asl20;
            maintainers = [ ];
            platforms = platforms.linux;
          };
        };

      in
      {
        # Default package
        packages.default = rustPackage;
        packages.systemd-status-leds = rustPackage;
        packages.mcp-nixos = mcp-nixos.packages.${system}.default;

        # Development shell
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          
          shellHook = ''
            echo "🦀 Rust development environment for systemd-status-leds"
            echo "Available commands:"
            echo "  cargo build          - Build the project"
            echo "  cargo auditable build - Build with embedded audit info"
            echo "  cargo test           - Run tests"
            echo "  cargo run            - Run the application"
            echo "  cargo clippy         - Run linter"
            echo "  cargo fmt            - Format code"
            echo "  cargo tarpaulin      - Generate test coverage"
            echo "  cargo audit bin <binary> - Check embedded audit info"
            echo "  mcp-nixos            - Start MCP NixOS server for AI assistance"
            echo ""
            echo "To build and test:"
            echo "  nix build            - Build the package"
            echo "  nix flake check      - Run all checks"
            echo "  nix run .#mcp-nixos  - Run MCP NixOS server"
          '';
        };

        # Checks that run during `nix flake check`
        checks = {
          # Format check
          fmt = pkgs.runCommand "check-format" {
            buildInputs = [ rustToolchain ];
          } ''
            cd ${./.}
            cargo fmt --check
            touch $out
          '';

          # Clippy linting
          clippy = pkgs.runCommand "check-clippy" {
            buildInputs = buildInputs;
            PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          } ''
            cd ${./.}
            cargo clippy -- -D warnings
            touch $out
          '';

          # Test execution
          test = pkgs.runCommand "run-tests" {
            buildInputs = buildInputs;
            PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          } ''
            cd ${./.}
            cargo test
            touch $out
          '';

          # Audit check - build with embedded audit info
          audit = pkgs.runCommand "check-audit" {
            buildInputs = buildInputs;
            PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          } ''
            # Build with cargo-auditable to embed audit information
            cd ${./.}
            cargo auditable build --release
            
            # Verify the binary was built successfully with auditable
            if [ ! -f target/release/systemd-status-leds ]; then
              echo "Failed to build binary with cargo auditable"
              exit 1
            fi
            
            echo "Binary built successfully with embedded audit information"
            touch $out
          '';

          # Build check
          build = rustPackage;
        };

        # Apps for running
        apps.default = flake-utils.lib.mkApp {
          drv = rustPackage;
          name = "systemd-status-leds";
        };
        
        apps.mcp-nixos = flake-utils.lib.mkApp {
          drv = mcp-nixos.packages.${system}.default;
          name = "mcp-nixos";
        };

        # Formatter for `nix fmt`
        formatter = pkgs.nixpkgs-fmt;
      });
}