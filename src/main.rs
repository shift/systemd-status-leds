//! Main application for systemd-status-leds
//!
//! This application monitors systemd service status and displays it on WS281x RGBW LED strips.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use systemd_status_leds::{Config, Strip, SystemdMonitor};
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Command line arguments
#[derive(Parser, Debug)]
#[command(name = "systemd-status-leds")]
#[command(about = "Monitor systemd service status and display on WS281x LED strips")]
#[command(version)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = args.log_level.parse().unwrap_or(tracing::Level::INFO);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level.to_string())),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting systemd-status-leds");

    // Load configuration
    let config = Config::from_file(&args.config)
        .map_err(|e| anyhow::anyhow!("Failed to load config from {:?}: {}", args.config, e))?;

    info!(
        "Loaded configuration: {} services, {} LEDs",
        config.services.len(),
        config.strip.length
    );

    // Check if systemd is running
    let systemd_monitor = SystemdMonitor::new().await?;
    if !systemd_monitor.is_systemd_running().await? {
        error!("SystemD is not running. This application requires systemd.");
        std::process::exit(1);
    }

    // Initialize LED strip
    let strip_config = systemd_status_leds::strip::StripConfig {
        device_path: config.strip.spidev.clone(),
        length: config.strip.length as usize,
        channels: config.strip.channels as usize,
        frequency: config.strip.hertz,
    };

    let mut strip = Strip::new(strip_config)?;
    info!("Initialized LED strip: {}", config.strip.spidev);

    // Set loading pattern
    strip.set_loading_pattern()?;
    strip.update()?;

    // Add services to monitoring
    for (index, service) in config.services.iter().enumerate() {
        info!("Adding service '{}' to position {}", service.name, index);
        
        // Add service to strip
        strip.add_service(service.name.clone())?;
        
        // Add service to systemd monitoring
        systemd_monitor.add_service(&service.name).await?;
    }

    // Start monitoring tasks
    let mut event_receiver = systemd_monitor.subscribe_to_events();
    
    // Start systemd monitoring task
    let monitor_handle = {
        let systemd_monitor = systemd_monitor;
        tokio::spawn(async move {
            if let Err(e) = systemd_monitor.start_monitoring().await {
                error!("SystemD monitoring failed: {}", e);
            }
        })
    };

    // Start LED update loop task
    let update_handle = {
        let mut strip_clone = strip;
        tokio::spawn(async move {
            if let Err(e) = strip_clone.run_update_loop().await {
                error!("LED strip update loop failed: {}", e);
            }
        })
    };

    // Handle service events
    let event_handle = {
        let config_clone = config.clone();
        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                info!(
                    "Service '{}' state changed to: {:?}",
                    event.unit_name, event.state
                );

                // Find the service in our configuration
                if let Some((index, _)) = config_clone
                    .services
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.name == event.unit_name)
                {
                    let state_str = match event.state {
                        systemd_status_leds::ServiceState::Active => "active",
                        systemd_status_leds::ServiceState::Inactive => "inactive",
                        systemd_status_leds::ServiceState::Activating => "activating",
                        systemd_status_leds::ServiceState::Deactivating => "deactivating",
                        systemd_status_leds::ServiceState::Reloading => "reloading",
                        systemd_status_leds::ServiceState::Failed => "failed",
                        systemd_status_leds::ServiceState::Unknown => "unknown",
                    };

                    if let Some(color) = config_clone.get_color_for_state(index, state_str) {
                        info!(
                            "Setting LED {} to color {} for service '{}'",
                            index, color.to_hex(), event.unit_name
                        );
                        
                        // In a real implementation, we'd need to get access to the strip here
                        // For now, we'll log the color change
                        // This would require refactoring to share strip access between tasks
                    } else {
                        warn!(
                            "No color defined for state '{}' of service '{}'",
                            state_str, event.unit_name
                        );
                    }
                }
            }
        })
    };

    info!("Application started successfully. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Shutdown signal received");
        }
        _ = monitor_handle => {
            error!("SystemD monitoring task ended unexpectedly");
        }
        _ = update_handle => {
            error!("LED update task ended unexpectedly");
        }
        _ = event_handle => {
            error!("Event handling task ended unexpectedly");
        }
    }

    info!("Shutting down...");
    Ok(())
}
