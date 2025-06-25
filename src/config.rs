//! Configuration module for systemd-status-leds
//!
//! Handles loading and parsing of YAML configuration files.

use crate::{Color, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// Configuration for a systemd service to monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Name of the systemd unit (e.g., "ssh.service")
    pub name: String,
    /// Optional custom color mappings for service states
    #[serde(default)]
    pub states_map: HashMap<String, String>,
}

/// Configuration for the LED strip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripConfig {
    /// SPI device path (e.g., "0.0")
    pub spidev: String,
    /// Number of color channels per LED (typically 4 for RGBW)
    pub channels: u8,
    /// Number of LEDs in the strip
    pub length: u8,
    /// SPI frequency in Hz
    pub hertz: u32,
    /// Color mappings for service states
    pub colours: HashMap<String, String>,
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// List of services to monitor
    pub services: Vec<ServiceConfig>,
    /// LED strip configuration
    pub strip: StripConfig,
}

impl Config {
    /// Load configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = content.parse()?;
        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.services.is_empty() {
            return Err(anyhow::anyhow!("No services configured"));
        }

        if self.services.len() > self.strip.length as usize {
            return Err(anyhow::anyhow!(
                "More services ({}) than LEDs ({})",
                self.services.len(),
                self.strip.length
            ));
        }

        // Validate color formats in strip configuration
        for (state, color_str) in &self.strip.colours {
            Color::from_hex(color_str).map_err(|e| {
                anyhow::anyhow!("Invalid color '{}' for state '{}': {}", color_str, state, e)
            })?;
        }

        // Validate color formats in service configurations
        for service in &self.services {
            for (state, color_str) in &service.states_map {
                Color::from_hex(color_str).map_err(|e| {
                    anyhow::anyhow!(
                        "Invalid color '{}' for state '{}' in service '{}': {}",
                        color_str,
                        state,
                        service.name,
                        e
                    )
                })?;
            }
        }

        Ok(())
    }

    /// Get color for a service state, with fallback to strip default colors
    pub fn get_color_for_state(&self, service_index: usize, state: &str) -> Option<Color> {
        // First check service-specific color mapping
        if let Some(service) = self.services.get(service_index) {
            if let Some(color_str) = service.states_map.get(state) {
                if let Ok(color) = Color::from_hex(color_str) {
                    return Some(color);
                }
            }
        }

        // Fallback to strip default colors
        if let Some(color_str) = self.strip.colours.get(state) {
            Color::from_hex(color_str).ok()
        } else {
            None
        }
    }
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(content: &str) -> Result<Self> {
        let config: Config = serde_yaml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut colours = HashMap::new();
        colours.insert("active".to_string(), "00ff0000".to_string());
        colours.insert("inactive".to_string(), "01010101".to_string());
        colours.insert("reloading".to_string(), "11551100".to_string());
        colours.insert("failed".to_string(), "55002200".to_string());
        colours.insert("activating".to_string(), "00442200".to_string());
        colours.insert("deactivating".to_string(), "22440000".to_string());

        Self {
            services: vec![ServiceConfig {
                name: "example.service".to_string(),
                states_map: HashMap::new(),
            }],
            strip: StripConfig {
                spidev: "0.0".to_string(),
                channels: 4,
                length: 5,
                hertz: 1200,
                colours,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const VALID_CONFIG: &str = r#"
services:
  - name: network.target
    states_map:
      active: 00ff5500
  - name: minecraft.service
    states_map:
      active: 00ff9900
  - name: multi-user.target
  - name: local-exporter.service
  - name: node-exporter.service
strip:
  spidev: "0.0"
  channels: 4
  length: 5
  hertz: 1200
  colours:
    active: 00ff0000
    inactive: 01010101
    reloading: 11551100
    failed: 55002200
    activating: 00442200
    deactivating: 22440000
"#;

    #[test]
    fn test_config_from_str() {
        let config: Config = VALID_CONFIG.parse().unwrap();
        assert_eq!(config.services.len(), 5);
        assert_eq!(config.services[0].name, "network.target");
        assert_eq!(config.strip.spidev, "0.0");
        assert_eq!(config.strip.length, 5);
    }

    #[test]
    fn test_config_from_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(VALID_CONFIG.as_bytes()).unwrap();

        let config = Config::from_file(temp_file.path()).unwrap();
        assert_eq!(config.services.len(), 5);
    }

    #[test]
    fn test_config_validation_no_services() {
        let config_str = r#"
services: []
strip:
  spidev: "0.0"
  channels: 4
  length: 5
  hertz: 1200
  colours:
    active: 00ff0000
"#;
        assert!(config_str.parse::<Config>().is_err());
    }

    #[test]
    fn test_config_validation_too_many_services() {
        let config_str = r#"
services:
  - name: service1
  - name: service2
  - name: service3
  - name: service4
  - name: service5
  - name: service6
strip:
  spidev: "0.0"
  channels: 4
  length: 5
  hertz: 1200
  colours:
    active: 00ff0000
"#;
        assert!(config_str.parse::<Config>().is_err());
    }

    #[test]
    fn test_config_validation_invalid_color() {
        let config_str = r#"
services:
  - name: service1
strip:
  spidev: "0.0"
  channels: 4
  length: 5
  hertz: 1200
  colours:
    active: invalid_color
"#;
        assert!(config_str.parse::<Config>().is_err());
    }

    #[test]
    fn test_get_color_for_state() {
        let config: Config = VALID_CONFIG.parse().unwrap();

        // Test service-specific color
        let color = config.get_color_for_state(0, "active").unwrap();
        assert_eq!(color, Color::from_hex("00ff5500").unwrap());

        // Test fallback to strip default
        let color = config.get_color_for_state(2, "active").unwrap();
        assert_eq!(color, Color::from_hex("00ff0000").unwrap());

        // Test unknown state
        assert!(config.get_color_for_state(0, "unknown").is_none());
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.strip.length, 5);
        assert!(config.strip.colours.contains_key("active"));
    }
}
