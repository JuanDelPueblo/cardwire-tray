mod dbus;
mod gui;
mod models;
mod tray;
mod utils;

use dbus::get_client;
use futures_util::StreamExt;
use gui::run_gui_daemon;
use ksni::TrayMethods;
use models::{AppletConfig, CardwireClient, CardwireTray, PowerStateChangedArgs, TrayAction};
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use utils::{get_gpus, get_latest_version};

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

fn spawn_power_state_listener(
    client: CardwireClient,
    gpu_id: u32,
    power_tx: mpsc::Sender<(u32, String)>,
) {
    tokio::spawn(async move {
        let Ok(gpu) = client.gpu_proxy(gpu_id).await else {
            return;
        };
        let Ok(mut stream) = gpu.receive_power_state_changed().await else {
            return;
        };

        while let Some(signal) = stream.next().await {
            let args: PowerStateChangedArgs = match signal.args() {
                Ok(args) => args,
                Err(_) => continue,
            };

            if power_tx.send((gpu_id, args.state)).await.is_err() {
                break;
            }
        }
    });
}

fn main() -> AppResult<()> {
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    let open_window_requested = Arc::new(AtomicBool::new(false));

    let (client, applet_config) =
        runtime.block_on(start_tray(Arc::clone(&open_window_requested)))?;

    run_gui_daemon(client, applet_config, open_window_requested)?;

    Ok(())
}

async fn start_tray(
    open_window_requested: Arc<AtomicBool>,
) -> AppResult<(CardwireClient, Arc<Mutex<AppletConfig>>)> {
    // Retry loop for the Cardwire service.
    let client = get_client().await;
    let mode_proxy = client.mode_proxy().await?;

    let initial_mode = mode_proxy.mode().await.unwrap_or(0);

    let gpus = get_gpus(&client).await;
    let initial_gpu_ids = gpus.iter().map(|gpu| gpu.id).collect::<Vec<_>>();

    let applet_config = Arc::new(Mutex::new(AppletConfig::load()));
    let latest_version = get_latest_version().await;

    let (action_tx, mut action_rx) = mpsc::channel(10);
    let tray = CardwireTray {
        mode: initial_mode,
        gpus,
        applet_config: Arc::clone(&applet_config),
        action_tx,
        current_version: format!("v{}", env!("CARGO_PKG_VERSION").to_string()),
        latest_version,
    };

    let tray_handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut mode_stream = mode_proxy.receive_mode_changed().await;

    let client_clone = client.clone();
    let handle_clone = tray_handle.clone();

    tokio::spawn(async move {
        let mut version_refresh = tokio::time::interval(Duration::from_secs(60 * 60 * 24));
        version_refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
        version_refresh.tick().await;

        let mut gpu_discovery_refresh = tokio::time::interval(Duration::from_secs(60));
        gpu_discovery_refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
        gpu_discovery_refresh.tick().await;

        let (power_tx, mut power_rx) = mpsc::channel(20);
        let mut watched_gpu_ids = HashSet::new();
        for gpu_id in initial_gpu_ids {
            if watched_gpu_ids.insert(gpu_id) {
                spawn_power_state_listener(client_clone.clone(), gpu_id, power_tx.clone());
            }
        }

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
                            let _ = client_clone.set_mode(mode).await;
                        }
                        TrayAction::OpenWindow => {
                            open_window_requested.store(true, Ordering::SeqCst);
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
                            if client_clone.set_gpu_block(gpu_id, block).await.is_ok() {
                                let _ = handle_clone.update(|tray: &mut CardwireTray| {
                                    if let Some(gpu) = tray.gpus.iter_mut().find(|gpu| gpu.id == gpu_id) {
                                        gpu.blocked = block;
                                    }
                                }).await;
                            }
                        }
                        TrayAction::Quit => {
                            std::process::exit(0);
                        }
                    }
                }
                Some((gpu_id, power_state)) = power_rx.recv() => {
                    let _ = handle_clone.update(|tray: &mut CardwireTray| {
                        if let Some(gpu) = tray.gpus.iter_mut().find(|gpu| gpu.id == gpu_id) {
                            gpu.power_state = power_state;
                        }
                    }).await;
                }
                _ = gpu_discovery_refresh.tick() => {
                    let new_gpus = get_gpus(&client_clone).await;
                    for gpu in &new_gpus {
                        if watched_gpu_ids.insert(gpu.id) {
                            spawn_power_state_listener(client_clone.clone(), gpu.id, power_tx.clone());
                        }
                    }
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

    Ok((client, applet_config))
}
