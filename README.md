# systemd status leds

A Rust application that monitors systemd service status and displays it on WS281x RGBW LED strips via SPI.

## Features

- **Real-time monitoring**: Subscribe to systemd service state changes via DBus
- **LED visualization**: Display service status on WS281x RGBW LED strips
- **Configurable colors**: Custom color mappings for different service states
- **Hardware abstraction**: Mock-friendly design for testing without hardware
- **Comprehensive testing**: Full test coverage with unit and integration tests
- **Nix support**: Build and test using Nix Flakes

## Hardware Requirements

- Raspberry Pi or similar Linux system with systemd
- WS281x RGBW LED strip (tested with 5 LEDs)
- SPI interface enabled (you may need to create a Device Tree Overlay)

## Installation

### Using Nix Flakes (Recommended)

```bash
# Build the application
nix build

# Run directly
nix run

# Enter development environment
nix develop
```

### Using Cargo

```bash
# Build
cargo build --release

# Run
cargo run -- --config config.yaml
```

## Configuration

Create a `config.yaml` file:

```yaml
services:
  - name: network.target
    states_map:
      active: 00ff5500  # Custom color for this service
  - name: minecraft.service
    states_map:
      active: 00ff9900
  - name: multi-user.target
  - name: local-exporter.service
  - name: node-exporter.service

strip:
  spidev: "0.0"          # SPI device (creates /dev/spidev0.0)
  channels: 4            # RGBW = 4 channels
  length: 5              # Number of LEDs
  hertz: 1200           # Update frequency
  colours:              # Default colors for all services
    active: 00ff0000      # Green
    inactive: 01010101    # Very dim white
    reloading: 11551100   # Yellow
    failed: 55002200      # Red
    activating: 00442200  # Orange
    deactivating: 22440000 # Dark orange
```

### Color Format

Colors are specified as 8-character hex strings representing RGBW values:
- Format: `RRGGBBWW`
- Example: `00ff0000` = Red=0, Green=255, Blue=0, White=0 (pure green)

## Usage

```bash
# Run with default config
systemd-status-leds

# Run with custom config
systemd-status-leds --config /path/to/config.yaml

# Set log level
systemd-status-leds --log-level debug
```

The application requires root privileges to access SPI devices and systemd DBus interface.

## Development

### Setting up the development environment

```bash
# Enter Nix development shell
nix develop

# Or install dependencies manually:
# - Rust toolchain
# - systemd development headers
# - dbus development headers
```

### Building and Testing

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test

# Run tests with coverage
cargo tarpaulin --out html

# Security audit
cargo audit
```

### Nix Development Commands

```bash
# Run all checks
nix flake check

# Build package
nix build

# Format Nix files
nix fmt
```

## Architecture

The application is structured into several modules:

- **`config`**: YAML configuration parsing and validation
- **`led`**: LED state management and color handling
- **`strip`**: SPI interface for controlling WS281x LED strips
- **`systemd`**: DBus integration for monitoring systemd services
- **`main`**: Application orchestration and event handling

### Testing Strategy

- **Unit tests**: Each module has comprehensive unit tests
- **Mock testing**: Hardware interfaces (SPI, DBus) are mocked for testing
- **Integration tests**: Test the interaction between modules
- **Property-based testing**: Color parsing and validation
- **Coverage**: Aim for >90% test coverage

## Hardware Setup

### Enabling SPI

On Raspberry Pi, enable SPI in `/boot/config.txt`:

```
dtparam=spi=on
```

You may need to create a custom Device Tree Overlay for your specific LED strip configuration.

### Wiring

Connect your WS281x LED strip to the SPI pins:
- MOSI (GPIO 10) → Data In
- SCLK (GPIO 11) → Clock (if required)
- GND → Ground
- 5V → Power (ensure adequate power supply)

## Troubleshooting

### Permission Issues
- Run as root or add user to `spi` group
- Ensure systemd DBus access permissions

### SPI Issues
- Verify SPI is enabled: `ls /dev/spidev*`
- Check Device Tree configuration
- Verify wiring and power supply

### Service Not Found
- Check service names: `systemctl list-units`
- Ensure services exist and are loaded

## Background

This project originated as a sub-project for a [Minecraft Server](https://github.com/shift/fcos-mc-pi4) setup, providing visual status indication for various system services.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Format code: `cargo fmt`
6. Run linter: `cargo clippy`
7. Submit a pull request
