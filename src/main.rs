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
use utils::get_gpus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Retry loop for the specific service proxy
    let proxy = get_proxy().await;

    let initial_mode = proxy.mode().await.unwrap_or(0);

    let gpus = get_gpus(&proxy).await;

    let (action_tx, mut action_rx) = mpsc::channel(10);
    let tray = CardwireTray {
        mode: initial_mode,
        gpus,
        action_tx,
    };

    let tray_handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut mode_stream = proxy.receive_mode_changed().await;

    let proxy_clone = proxy.clone();
    let handle_clone = tray_handle.clone();

    tokio::spawn(async move {
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
            }
        }
    });

    // Keeping the main thread alive
    loop {
        std::thread::park();
    }
}
