//! LED Strip module for controlling WS281x RGBW LED strips via SPI
//!
//! This module provides the interface to control WS281x LED strips through SPI,
//! with support for mocking during testing.

use crate::{led::LedCollection, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{debug, error, info, warn};

#[cfg(test)]
use mockall::{automock, predicate::*};

/// Trait for SPI device operations to enable mocking
#[cfg_attr(test, automock)]
pub trait SpiDevice: Send + Sync {
    /// Write data to the SPI device
    fn write(&mut self, data: &[u8]) -> Result<usize>;
}

/// Real SPI device implementation using spidev
pub struct RealSpiDevice {
    device: std::fs::File,
}

impl RealSpiDevice {
    /// Create a new SPI device
    pub fn new(device_path: &str) -> Result<Self> {
        let path = format!("/dev/spidev{}", device_path);
        let device = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| anyhow::anyhow!("Failed to open SPI device {}: {}", path, e))?;
        
        info!("Opened SPI device: {}", path);
        Ok(Self { device })
    }
}

impl SpiDevice for RealSpiDevice {
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.device.write(data).map_err(|e| e.into())
    }
}

/// Configuration for the LED strip
#[derive(Debug, Clone)]
pub struct StripConfig {
    /// SPI device path (e.g., "0.0" for /dev/spidev0.0)
    pub device_path: String,
    /// Number of LEDs in the strip
    pub length: usize,
    /// Number of color channels per LED (typically 4 for RGBW)
    pub channels: usize,
    /// Update frequency in Hz
    pub frequency: u32,
}

/// LED Strip controller
pub struct Strip {
    config: StripConfig,
    spi_device: Box<dyn SpiDevice>,
    led_collection: LedCollection,
    last_update: Instant,
    update_interval: Duration,
}

impl Strip {
    /// Create a new Strip with real SPI device
    pub fn new(config: StripConfig) -> Result<Self> {
        let spi_device = Box::new(RealSpiDevice::new(&config.device_path)?);
        Self::with_spi_device(config, spi_device)
    }

    /// Create a new Strip with custom SPI device (for testing)
    pub fn with_spi_device(config: StripConfig, spi_device: Box<dyn SpiDevice>) -> Result<Self> {
        let led_collection = LedCollection::new(config.length);
        let update_interval = Duration::from_millis(1000 / config.frequency as u64);
        
        Ok(Self {
            config,
            spi_device,
            led_collection,
            last_update: Instant::now(),
            update_interval,
        })
    }

    /// Add a service to monitor (assigns it to the next available LED)
    pub fn add_service(&mut self, unit_name: String) -> Result<()> {
        if self.led_collection.len() >= self.config.length {
            return Err(anyhow::anyhow!(
                "Cannot add more services: strip only has {} LEDs",
                self.config.length
            ));
        }

        self.led_collection.add_led(unit_name.clone())?;
        info!("Added service '{}' to LED position {}", unit_name, self.led_collection.len() - 1);
        Ok(())
    }

    /// Get the LED collection for external access
    pub fn led_collection(&self) -> &LedCollection {
        &self.led_collection
    }

    /// Update the LED strip with current LED states
    pub fn update(&mut self) -> Result<()> {
        let buffer = self.led_collection.to_buffer(self.config.length);
        
        debug!("Updating LED strip with {} bytes", buffer.len());
        
        match self.spi_device.write(&buffer) {
            Ok(bytes_written) => {
                if bytes_written != buffer.len() {
                    warn!(
                        "Partial write to SPI device: {} of {} bytes written",
                        bytes_written, buffer.len()
                    );
                }
                self.last_update = Instant::now();
                Ok(())
            }
            Err(e) => {
                error!("Failed to write to SPI device: {}", e);
                Err(e)
            }
        }
    }

    /// Start the update loop that continuously refreshes the LED strip
    pub async fn run_update_loop(&mut self) -> Result<()> {
        info!("Starting LED strip update loop ({}Hz)", self.config.frequency);
        
        let mut interval = time::interval(self.update_interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update() {
                error!("Error updating LED strip: {}", e);
                // Continue running even if individual updates fail
            }
        }
    }

    /// Set all LEDs to a loading pattern
    pub fn set_loading_pattern(&self) -> Result<()> {
        let loading_color = crate::Color::new(60, 60, 60, 60);
        for led in self.led_collection.leds() {
            led.set_color(loading_color);
        }
        Ok(())
    }

    /// Turn off all LEDs
    pub fn turn_off_all(&self) {
        self.led_collection.reset_all();
    }

    /// Get strip configuration
    pub fn config(&self) -> &StripConfig {
        &self.config
    }

    /// Get the number of services currently configured
    pub fn service_count(&self) -> usize {
        self.led_collection.len()
    }

    /// Check if the strip is at capacity
    pub fn is_full(&self) -> bool {
        self.led_collection.len() >= self.config.length
    }
}

impl Drop for Strip {
    /// Clean up: turn off all LEDs when dropping the strip
    fn drop(&mut self) {
        debug!("Shutting down LED strip");
        self.turn_off_all();
        if let Err(e) = self.update() {
            error!("Failed to turn off LEDs during shutdown: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    #[tokio::test]
    async fn test_strip_creation() {
        let config = StripConfig {
            device_path: "test".to_string(),
            length: 5,
            channels: 4,
            frequency: 10,
        };

        let mut mock_spi = MockSpiDevice::new();
        mock_spi.expect_write().returning(|data| Ok(data.len()));

        let strip = Strip::with_spi_device(config.clone(), Box::new(mock_spi)).unwrap();
        
        assert_eq!(strip.config().length, 5);
        assert_eq!(strip.config().channels, 4);
        assert_eq!(strip.service_count(), 0);
        assert!(!strip.is_full());
    }

    #[tokio::test]
    async fn test_add_service() {
        let config = StripConfig {
            device_path: "test".to_string(),
            length: 2,
            channels: 4,
            frequency: 10,
        };

        let mut mock_spi = MockSpiDevice::new();
        mock_spi.expect_write().returning(|data| Ok(data.len()));

        let mut strip = Strip::with_spi_device(config, Box::new(mock_spi)).unwrap();
        
        strip.add_service("service1.service".to_string()).unwrap();
        assert_eq!(strip.service_count(), 1);
        assert!(!strip.is_full());
        
        strip.add_service("service2.service".to_string()).unwrap();
        assert_eq!(strip.service_count(), 2);
        assert!(strip.is_full());
        
        // Should fail to add more services
        assert!(strip.add_service("service3.service".to_string()).is_err());
    }

    #[tokio::test]
    async fn test_update() {
        let config = StripConfig {
            device_path: "test".to_string(),
            length: 2,
            channels: 4,
            frequency: 10,
        };

        let mut mock_spi = MockSpiDevice::new();
        // The buffer will be for the entire strip length (2 LEDs * 4 bytes = 8 bytes)
        // Will be called once for explicit update() and once during drop
        mock_spi
            .expect_write()
            .times(2) 
            .withf(|data| data.len() == 8)
            .returning(|data| Ok(data.len()));

        {
            let mut strip = Strip::with_spi_device(config, Box::new(mock_spi)).unwrap();
            
            strip.add_service("service1.service".to_string()).unwrap();
            strip.add_service("service2.service".to_string()).unwrap();
            
            // Set colors
            let led1 = strip.led_collection().get_led(0).unwrap();
            let led2 = strip.led_collection().get_led(1).unwrap();
            led1.set_color(Color::new(255, 0, 0, 0));
            led2.set_color(Color::new(0, 255, 0, 0));
            
            strip.update().unwrap();
        } // strip is dropped here, triggering another write call
    }

    #[tokio::test]
    async fn test_loading_pattern() {
        let config = StripConfig {
            device_path: "test".to_string(),
            length: 3,
            channels: 4,
            frequency: 10,
        };

        let mut mock_spi = MockSpiDevice::new();
        mock_spi.expect_write().returning(|data| Ok(data.len()));

        let mut strip = Strip::with_spi_device(config, Box::new(mock_spi)).unwrap();
        
        strip.add_service("service1.service".to_string()).unwrap();
        strip.add_service("service2.service".to_string()).unwrap();
        
        strip.set_loading_pattern().unwrap();
        
        let loading_color = Color::new(60, 60, 60, 60);
        for led in strip.led_collection().leds() {
            assert_eq!(led.color(), loading_color);
        }
    }

    #[tokio::test]
    async fn test_turn_off_all() {
        let config = StripConfig {
            device_path: "test".to_string(),
            length: 2,
            channels: 4,
            frequency: 10,
        };

        let mut mock_spi = MockSpiDevice::new();
        mock_spi.expect_write().returning(|data| Ok(data.len()));

        let mut strip = Strip::with_spi_device(config, Box::new(mock_spi)).unwrap();
        
        strip.add_service("service1.service".to_string()).unwrap();
        
        // Set a color
        let led = strip.led_collection().get_led(0).unwrap();
        led.set_color(Color::new(255, 255, 255, 255));
        
        strip.turn_off_all();
        
        assert_eq!(led.color(), Color::default());
    }

    #[test]
    fn test_real_spi_device_creation_fails_gracefully() {
        // This should fail since we don't have real SPI devices in test environment
        let result = RealSpiDevice::new("99.99");
        assert!(result.is_err());
    }
}