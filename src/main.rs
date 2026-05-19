mod dbus;
mod models;
mod tray;
mod utils;

use dbus::get_proxy;
use futures_util::StreamExt;
use ksni::TrayMethods;
use models::{CardwireTray, TrayAction};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use utils::{get_gpus, get_latest_version};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Retry loop for the specific service proxy
    let proxy = get_proxy().await;

    let initial_mode = proxy.mode().await.unwrap_or(0);

    let gpus = get_gpus(&proxy).await;

    let latest_version = get_latest_version().await;

    let (action_tx, mut action_rx) = mpsc::channel(10);
    let tray = CardwireTray {
        mode: initial_mode,
        gpus,
        action_tx,
        current_version: format!("v{}", env!("CARGO_PKG_VERSION").to_string()),
        latest_version,
    };

    let tray_handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut mode_stream = proxy.receive_mode_changed().await;

    let proxy_clone = proxy.clone();
    let handle_clone = tray_handle.clone();

    tokio::spawn(async move {
        let mut version_refresh = tokio::time::interval(Duration::from_secs(60 * 60 * 24));
        version_refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
        version_refresh.tick().await;

        loop {
            tokio::select! {
                Some(changed) = mode_stream.next() => {
                    if let Ok(new_mode) = changed.get().await {
                        let _ = handle_clone.update(|tray: &mut CardwireTray| {
                            tray.mode = new_mode;
                        }).await;
                    }
                }
                Some(action) = action_rx.recv() => {
                    match action {
                        TrayAction::SetMode(mode) => {
                            let _ = proxy_clone.set_mode(mode).await;
                        }
                        TrayAction::Notify(msg, icon) => {
                            // Using tokio::spawn to ensure any blocking code in show() doesn't block the async task
                            tokio::task::spawn_blocking(move || {
                                let _ = notify_rust::Notification::new()
                                    .summary("Cardwire")
                                    .body(&msg)
                                    .icon(&icon)
                                    .show();
                            });
                        }
                        TrayAction::ToggleGpuBlock(gpu_id, block) => {
                            let _ = proxy_clone.set_gpu_block(gpu_id, block).await;
                        }
                        TrayAction::Quit => {
                            std::process::exit(0);
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    let new_gpus = get_gpus(&proxy_clone).await;
                    let _ = handle_clone.update(|tray: &mut CardwireTray| {
                        tray.gpus = new_gpus;
                    }).await;
                }
                _ = version_refresh.tick() => {
                    if let Some(latest_version) = get_latest_version().await {
                        let _ = handle_clone.update(|tray: &mut CardwireTray| {
                            tray.latest_version = Some(latest_version);
                        }).await;
                    }
                }
            }
        }
    });

    // Keeping the main thread alive
    loop {
        std::thread::park();
    }
}
