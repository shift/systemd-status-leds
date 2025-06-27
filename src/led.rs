//! LED module for managing individual LED states
//!
//! This module provides the `Led` structure that represents a single LED
//! in the strip and its current state.

use crate::{Color, ServiceState};
use std::sync::{Arc, RwLock};

/// Represents a single LED in the strip
#[derive(Debug, Clone)]
pub struct Led {
    /// Current color of the LED
    color: Arc<RwLock<Color>>,
    /// Index position of this LED in the strip (0-based)
    position: usize,
    /// Name of the systemd unit this LED represents
    unit_name: String,
    /// Current service state
    service_state: Arc<RwLock<ServiceState>>,
}

impl Led {
    /// Create a new LED instance
    pub fn new(position: usize, unit_name: String) -> Self {
        Self {
            color: Arc::new(RwLock::new(Color::default())),
            position,
            unit_name,
            service_state: Arc::new(RwLock::new(ServiceState::Unknown)),
        }
    }

    /// Get the current color of the LED
    pub fn color(&self) -> Color {
        *self.color.read().unwrap()
    }

    /// Set the color of the LED
    pub fn set_color(&self, color: Color) {
        *self.color.write().unwrap() = color;
    }

    /// Get the position of this LED in the strip
    pub fn position(&self) -> usize {
        self.position
    }

    /// Get the unit name this LED represents
    pub fn unit_name(&self) -> &str {
        &self.unit_name
    }

    /// Get the current service state
    pub fn service_state(&self) -> ServiceState {
        self.service_state.read().unwrap().clone()
    }

    /// Set the service state and update color if provided
    pub fn set_service_state(&self, state: ServiceState, color: Option<Color>) {
        *self.service_state.write().unwrap() = state;
        if let Some(color) = color {
            self.set_color(color);
        }
    }

    /// Convert the LED color to bytes for SPI transmission
    pub fn to_bytes(&self) -> [u8; 4] {
        self.color().to_bytes()
    }

    /// Reset the LED to default state
    pub fn reset(&self) {
        self.set_color(Color::default());
        *self.service_state.write().unwrap() = ServiceState::Unknown;
    }
}

/// A collection of LEDs representing multiple services
#[derive(Debug)]
pub struct LedCollection {
    leds: Vec<Led>,
}

impl LedCollection {
    /// Create a new LED collection with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            leds: Vec::with_capacity(capacity),
        }
    }

    /// Add a new LED to the collection
    pub fn add_led(&mut self, unit_name: String) -> crate::Result<&Led> {
        let position = self.leds.len();
        let led = Led::new(position, unit_name);
        self.leds.push(led);
        Ok(self.leds.last().unwrap())
    }

    /// Get a LED by its position
    pub fn get_led(&self, position: usize) -> Option<&Led> {
        self.leds.get(position)
    }

    /// Get a LED by unit name
    pub fn get_led_by_unit(&self, unit_name: &str) -> Option<&Led> {
        self.leds.iter().find(|led| led.unit_name() == unit_name)
    }

    /// Get all LEDs
    pub fn leds(&self) -> &[Led] {
        &self.leds
    }

    /// Get the number of LEDs in the collection
    pub fn len(&self) -> usize {
        self.leds.len()
    }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.leds.is_empty()
    }

    /// Convert all LED colors to a byte buffer for SPI transmission
    pub fn to_buffer(&self, strip_length: usize) -> Vec<u8> {
        let mut buffer = vec![0u8; strip_length * 4]; // 4 bytes per LED (RGBW)

        for led in &self.leds {
            let pos = led.position();
            if pos < strip_length {
                let bytes = led.to_bytes();
                let offset = pos * 4;
                buffer[offset..offset + 4].copy_from_slice(&bytes);
            }
        }

        buffer
    }

    /// Reset all LEDs to default state
    pub fn reset_all(&self) {
        for led in &self.leds {
            led.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_led_creation() {
        let led = Led::new(0, "test.service".to_string());
        assert_eq!(led.position(), 0);
        assert_eq!(led.unit_name(), "test.service");
        assert_eq!(led.color(), Color::default());
        assert_eq!(led.service_state(), ServiceState::Unknown);
    }

    #[test]
    fn test_led_color_operations() {
        let led = Led::new(0, "test.service".to_string());
        let color = Color::new(255, 128, 64, 32);

        led.set_color(color);
        assert_eq!(led.color(), color);
        assert_eq!(led.to_bytes(), [255, 128, 64, 32]);
    }

    #[test]
    fn test_led_service_state() {
        let led = Led::new(0, "test.service".to_string());
        let color = Color::new(0, 255, 0, 0);

        led.set_service_state(ServiceState::Active, Some(color));
        assert_eq!(led.service_state(), ServiceState::Active);
        assert_eq!(led.color(), color);
    }

    #[test]
    fn test_led_reset() {
        let led = Led::new(0, "test.service".to_string());

        led.set_color(Color::new(255, 255, 255, 255));
        led.set_service_state(ServiceState::Active, None);

        led.reset();
        assert_eq!(led.color(), Color::default());
        assert_eq!(led.service_state(), ServiceState::Unknown);
    }

    #[test]
    fn test_led_collection() {
        let mut collection = LedCollection::new(3);

        assert_eq!(collection.len(), 0);
        assert!(collection.is_empty());

        collection.add_led("service1.service".to_string()).unwrap();
        collection.add_led("service2.service".to_string()).unwrap();

        assert_eq!(collection.len(), 2);
        assert!(!collection.is_empty());

        let led = collection.get_led(0).unwrap();
        assert_eq!(led.unit_name(), "service1.service");
        assert_eq!(led.position(), 0);

        let led = collection.get_led_by_unit("service2.service").unwrap();
        assert_eq!(led.position(), 1);
    }

    #[test]
    fn test_led_collection_buffer() {
        let mut collection = LedCollection::new(3);

        collection.add_led("service1.service".to_string()).unwrap();
        collection.add_led("service2.service".to_string()).unwrap();

        // Set colors for the LEDs
        collection
            .get_led(0)
            .unwrap()
            .set_color(Color::new(255, 0, 0, 0));
        collection
            .get_led(1)
            .unwrap()
            .set_color(Color::new(0, 255, 0, 0));

        let buffer = collection.to_buffer(4);
        assert_eq!(buffer.len(), 16); // 4 LEDs * 4 bytes each

        // Check first LED color
        assert_eq!(&buffer[0..4], &[255, 0, 0, 0]);
        // Check second LED color
        assert_eq!(&buffer[4..8], &[0, 255, 0, 0]);
        // Check remaining LEDs are off
        assert_eq!(&buffer[8..12], &[0, 0, 0, 0]);
        assert_eq!(&buffer[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn test_led_collection_reset() {
        let mut collection = LedCollection::new(2);

        collection.add_led("service1.service".to_string()).unwrap();
        collection.add_led("service2.service".to_string()).unwrap();

        // Set some colors and states
        collection
            .get_led(0)
            .unwrap()
            .set_color(Color::new(255, 255, 255, 255));
        collection
            .get_led(1)
            .unwrap()
            .set_service_state(ServiceState::Active, Some(Color::new(0, 255, 0, 0)));

        collection.reset_all();

        for led in collection.leds() {
            assert_eq!(led.color(), Color::default());
            assert_eq!(led.service_state(), ServiceState::Unknown);
        }
    }
}
