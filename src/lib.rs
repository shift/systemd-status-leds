//! # systemd-status-leds
//!
//! A Rust application that monitors systemd service status and displays it on WS281x RGBW LED strips via SPI.
//!
//! This library provides functionality to:
//! - Monitor systemd service states via DBus
//! - Control WS281x RGBW LED strips through SPI interface
//! - Map service states to LED colors
//! - Handle configuration from YAML files

pub mod config;
pub mod led;
pub mod strip;
pub mod systemd;

pub use config::Config;
pub use led::Led;
pub use strip::Strip;
pub use systemd::SystemdMonitor;

/// Result type used throughout the library
pub type Result<T> = anyhow::Result<T>;

/// LED color represented as RGBW values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub white: u8,
}

impl Color {
    /// Create a new color from RGBW values
    pub fn new(red: u8, green: u8, blue: u8, white: u8) -> Self {
        Self {
            red,
            green,
            blue,
            white,
        }
    }

    /// Create a color from a hex string (format: "RRGGBBWW")
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let hex_str = hex_str.trim_start_matches("0x").trim_start_matches("#");

        if hex_str.len() != 8 {
            return Err(anyhow::anyhow!(
                "Invalid hex color format: expected 8 characters, got {}",
                hex_str.len()
            ));
        }

        let bytes = hex::decode(hex_str)?;
        Ok(Self {
            red: bytes[0],
            green: bytes[1],
            blue: bytes[2],
            white: bytes[3],
        })
    }

    /// Convert color to a 4-byte array
    pub fn to_bytes(&self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.white]
    }

    /// Convert color to hex string
    pub fn to_hex(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue, self.white
        )
    }
}

/// Default colors for different service states
impl Default for Color {
    fn default() -> Self {
        Self::new(0, 0, 0, 0) // Off
    }
}

/// Service state enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    Active,
    Inactive,
    Activating,
    Deactivating,
    Reloading,
    Failed,
    Unknown,
}

impl From<&str> for ServiceState {
    fn from(state: &str) -> Self {
        match state {
            "active" => ServiceState::Active,
            "inactive" => ServiceState::Inactive,
            "activating" => ServiceState::Activating,
            "deactivating" => ServiceState::Deactivating,
            "reloading" => ServiceState::Reloading,
            "failed" => ServiceState::Failed,
            _ => ServiceState::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let color = Color::from_hex("ff0000ff").unwrap();
        assert_eq!(color, Color::new(255, 0, 0, 255));

        let color = Color::from_hex("0x00ff00aa").unwrap();
        assert_eq!(color, Color::new(0, 255, 0, 170));

        let color = Color::from_hex("#0000ff55").unwrap();
        assert_eq!(color, Color::new(0, 0, 255, 85));
    }

    #[test]
    fn test_color_to_hex() {
        let color = Color::new(255, 128, 64, 32);
        assert_eq!(color.to_hex(), "ff804020");
    }

    #[test]
    fn test_color_invalid_hex() {
        assert!(Color::from_hex("ff00").is_err());
        assert!(Color::from_hex("gghhiijj").is_err());
    }

    #[test]
    fn test_service_state_from_str() {
        assert_eq!(ServiceState::from("active"), ServiceState::Active);
        assert_eq!(ServiceState::from("inactive"), ServiceState::Inactive);
        assert_eq!(ServiceState::from("failed"), ServiceState::Failed);
        assert_eq!(ServiceState::from("unknown_state"), ServiceState::Unknown);
    }
}
