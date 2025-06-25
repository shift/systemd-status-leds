//! SystemD integration module for monitoring service states via DBus
//!
//! This module provides functionality to connect to systemd via DBus and
//! monitor service state changes, with support for mocking during testing.

use crate::{Result, ServiceState};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;
use tracing::{debug, error, info, warn};
use zbus::Connection;

#[cfg(test)]
use mockall::{automock, predicate::*};

/// Event representing a service state change
#[derive(Debug, Clone)]
pub struct ServiceEvent {
    pub unit_name: String,
    pub state: ServiceState,
    pub timestamp: std::time::SystemTime,
}

/// Trait for SystemD operations to enable mocking
#[cfg_attr(test, automock)]
#[async_trait::async_trait]
pub trait SystemdInterface: Send + Sync {
    /// Check if systemd is running
    async fn is_running(&self) -> Result<bool>;

    /// Get the current state of a unit
    async fn get_unit_state(&self, unit_name: &str) -> Result<ServiceState>;

    /// Subscribe to state changes for a unit
    async fn subscribe_to_unit(&self, unit_name: &str) -> Result<()>;

    /// Start monitoring for events (should be called in a separate task)
    async fn monitor_events(&self, event_sender: broadcast::Sender<ServiceEvent>) -> Result<()>;
}

/// Real SystemD interface implementation using zbus
pub struct RealSystemdInterface {
    connection: Connection,
    subscribed_units: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl RealSystemdInterface {
    /// Create a new SystemD interface
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await?;
        info!("Connected to SystemD via DBus");

        Ok(Self {
            connection,
            subscribed_units: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        })
    }
}

#[async_trait::async_trait]
impl SystemdInterface for RealSystemdInterface {
    async fn is_running(&self) -> Result<bool> {
        // Try to get systemd version to check if it's running
        let proxy = zbus::Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await?;

        match proxy.get_property::<String>("Version").await {
            Ok(_) => {
                debug!("SystemD is running");
                Ok(true)
            }
            Err(e) => {
                warn!("SystemD not detected: {}", e);
                Ok(false)
            }
        }
    }

    async fn get_unit_state(&self, unit_name: &str) -> Result<ServiceState> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await?;

        // Get unit object path
        let unit_path_reply = proxy.call_method("GetUnit", &(unit_name,)).await?;
        let unit_path: zbus::zvariant::OwnedObjectPath = unit_path_reply.body().deserialize()?;

        // Get unit properties
        let unit_proxy = zbus::Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            unit_path.as_str(),
            "org.freedesktop.systemd1.Unit",
        )
        .await?;

        let active_state: String = unit_proxy.get_property("ActiveState").await?;
        let load_state: String = unit_proxy.get_property("LoadState").await?;

        if load_state == "not-found" {
            debug!("Unit '{}' not found", unit_name);
            return Ok(ServiceState::Unknown);
        }

        debug!("Unit '{}' state: {}", unit_name, active_state);
        Ok(ServiceState::from(active_state.as_str()))
    }

    async fn subscribe_to_unit(&self, unit_name: &str) -> Result<()> {
        {
            let mut subscribed = self.subscribed_units.lock().unwrap();
            if subscribed.contains(unit_name) {
                debug!("Already subscribed to unit '{}'", unit_name);
                return Ok(());
            }
            subscribed.insert(unit_name.to_string());
        }

        // Subscribe to SystemD signals
        let proxy = zbus::Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .await?;

        proxy.call_method("Subscribe", &()).await?;

        info!("Subscribed to state changes for unit '{}'", unit_name);
        Ok(())
    }

    async fn monitor_events(&self, event_sender: broadcast::Sender<ServiceEvent>) -> Result<()> {
        info!("Starting SystemD event monitoring");

        // This is a simplified implementation. In a real scenario, you'd want to:
        // 1. Listen for PropertiesChanged signals
        // 2. Parse the signals to extract unit state changes
        // 3. Send events through the broadcast channel

        // For now, we'll implement a polling mechanism as a fallback
        let mut interval = time::interval(Duration::from_secs(5));
        let subscribed_units = self.subscribed_units.clone();

        loop {
            interval.tick().await;

            let units: Vec<String> = {
                let guard = subscribed_units.lock().unwrap();
                guard.iter().cloned().collect()
            };

            for unit_name in units {
                match self.get_unit_state(&unit_name).await {
                    Ok(state) => {
                        let event = ServiceEvent {
                            unit_name: unit_name.clone(),
                            state,
                            timestamp: std::time::SystemTime::now(),
                        };

                        if let Err(e) = event_sender.send(event) {
                            warn!("Failed to send event for unit '{}': {}", unit_name, e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get state for unit '{}': {}", unit_name, e);
                    }
                }
            }
        }
    }
}

/// SystemD monitor that manages service monitoring and event distribution
pub struct SystemdMonitor {
    interface: Box<dyn SystemdInterface>,
    event_sender: broadcast::Sender<ServiceEvent>,
    #[allow(dead_code)]
    event_receiver: broadcast::Receiver<ServiceEvent>,
}

impl SystemdMonitor {
    /// Create a new SystemD monitor with real interface
    pub async fn new() -> Result<Self> {
        let interface = Box::new(RealSystemdInterface::new().await?);
        Self::with_interface(interface).await
    }

    /// Create a new SystemD monitor with custom interface (for testing)
    pub async fn with_interface(interface: Box<dyn SystemdInterface>) -> Result<Self> {
        let (event_sender, event_receiver) = broadcast::channel(100);

        Ok(Self {
            interface,
            event_sender,
            event_receiver,
        })
    }

    /// Check if systemd is running
    pub async fn is_systemd_running(&self) -> Result<bool> {
        self.interface.is_running().await
    }

    /// Add a service to monitor
    pub async fn add_service(&self, unit_name: &str) -> Result<()> {
        info!("Adding service '{}' to monitoring", unit_name);

        // Check if unit exists and get initial state
        match self.interface.get_unit_state(unit_name).await {
            Ok(state) => {
                info!("Unit '{}' initial state: {:?}", unit_name, state);

                // Subscribe to changes
                self.interface.subscribe_to_unit(unit_name).await?;

                // Send initial state event
                let event = ServiceEvent {
                    unit_name: unit_name.to_string(),
                    state,
                    timestamp: std::time::SystemTime::now(),
                };

                if let Err(e) = self.event_sender.send(event) {
                    warn!(
                        "Failed to send initial event for unit '{}': {}",
                        unit_name, e
                    );
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to add service '{}': {}", unit_name, e);
                Err(e)
            }
        }
    }

    /// Get a receiver for service events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ServiceEvent> {
        self.event_sender.subscribe()
    }

    /// Start the monitoring loop (should be called in a separate task)
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting SystemD monitoring loop");
        self.interface
            .monitor_events(self.event_sender.clone())
            .await
    }

    /// Get the current state of a unit
    pub async fn get_unit_state(&self, unit_name: &str) -> Result<ServiceState> {
        self.interface.get_unit_state(unit_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_systemd_monitor_creation() {
        let mut mock_interface = MockSystemdInterface::new();
        mock_interface
            .expect_is_running()
            .times(1)
            .returning(|| Ok(true));

        let monitor = SystemdMonitor::with_interface(Box::new(mock_interface))
            .await
            .unwrap();

        assert!(monitor.is_systemd_running().await.unwrap());
    }

    #[tokio::test]
    async fn test_add_service() {
        let mut mock_interface = MockSystemdInterface::new();

        mock_interface
            .expect_get_unit_state()
            .with(eq("test.service"))
            .times(1)
            .returning(|_| Ok(ServiceState::Active));

        mock_interface
            .expect_subscribe_to_unit()
            .with(eq("test.service"))
            .times(1)
            .returning(|_| Ok(()));

        let monitor = SystemdMonitor::with_interface(Box::new(mock_interface))
            .await
            .unwrap();

        assert!(monitor.add_service("test.service").await.is_ok());
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let mut mock_interface = MockSystemdInterface::new();

        mock_interface
            .expect_get_unit_state()
            .with(eq("test.service"))
            .times(1)
            .returning(|_| Ok(ServiceState::Active));

        mock_interface
            .expect_subscribe_to_unit()
            .with(eq("test.service"))
            .times(1)
            .returning(|_| Ok(()));

        let monitor = SystemdMonitor::with_interface(Box::new(mock_interface))
            .await
            .unwrap();

        let mut event_receiver = monitor.subscribe_to_events();

        // Add service should trigger an initial event
        monitor.add_service("test.service").await.unwrap();

        // Wait for the event with timeout
        let event = timeout(Duration::from_millis(100), event_receiver.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");

        assert_eq!(event.unit_name, "test.service");
        assert_eq!(event.state, ServiceState::Active);
    }

    #[tokio::test]
    async fn test_service_state_conversion() {
        assert_eq!(ServiceState::from("active"), ServiceState::Active);
        assert_eq!(ServiceState::from("inactive"), ServiceState::Inactive);
        assert_eq!(ServiceState::from("activating"), ServiceState::Activating);
        assert_eq!(
            ServiceState::from("deactivating"),
            ServiceState::Deactivating
        );
        assert_eq!(ServiceState::from("reloading"), ServiceState::Reloading);
        assert_eq!(ServiceState::from("failed"), ServiceState::Failed);
        assert_eq!(ServiceState::from("unknown"), ServiceState::Unknown);
    }
}
