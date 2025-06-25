{
  description = "systemd-status-leds - Monitor systemd service status on WS281x LED strips";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
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
          cargo-audit
          
          # Testing tools
          cargo-tarpaulin  # For coverage
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
            
            # Install binary
            cargo install --path . --root $out
            
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

        # Development shell
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          PKG_CONFIG_PATH = "${pkgs.systemd.dev}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig";
          
          shellHook = ''
            echo "🦀 Rust development environment for systemd-status-leds"
            echo "Available commands:"
            echo "  cargo build          - Build the project"
            echo "  cargo test           - Run tests"
            echo "  cargo run            - Run the application"
            echo "  cargo clippy         - Run linter"
            echo "  cargo fmt            - Format code"
            echo "  cargo audit          - Security audit"
            echo "  cargo tarpaulin      - Generate test coverage"
            echo ""
            echo "To build and test:"
            echo "  nix build            - Build the package"
            echo "  nix flake check      - Run all checks"
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

          # Security audit
          audit = pkgs.runCommand "security-audit" {
            buildInputs = buildInputs;
          } ''
            cd ${./.}
            cargo audit
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

        # Formatter for `nix fmt`
        formatter = pkgs.nixpkgs-fmt;
      });
}