use futures_util::StreamExt;
use ksni::{Tray, TrayMethods};
use rfd::MessageDialog;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc;
use zbus::{Connection, proxy};

// Define the DBus proxy interface
#[proxy(
    interface = "com.github.opengamingcollective.cardwire",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait Cardwire {
    /// Mode property
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_mode(&self, mode: u32) -> zbus::Result<()>;

    /// SetGpuBlock method
    fn set_gpu_block(&self, gpu_id: u32, block: bool) -> zbus::Result<()>;

    /// GetStatus method
    fn get_status(&self, gpu_id: u32) -> zbus::Result<String>;

    /// ListDevices method
    fn list_devices(
        &self,
    ) -> zbus::Result<HashMap<u64, (u32, String, String, u32, u32, bool, bool, bool, String)>>;
}

#[derive(Debug, Clone)]
struct GpuInfo {
    id: u32,
    name: String,
    is_default: bool,
    blocked: bool,
    power_state: String,
}

struct CardwireTray {
    mode: u32,
    gpus: Vec<GpuInfo>,
    action_tx: mpsc::Sender<TrayAction>,
}

enum TrayAction {
    SetMode(u32),
    ToggleGpuBlock(u32, bool),
    Notify(String, String),
    Quit,
}

impl Tray for CardwireTray {
    fn id(&self) -> String {
        "me.edyan.cardwiretray".to_string()
    }

    fn icon_name(&self) -> String {
        let name = match self.mode {
            0 => "integrated",
            1 => "hybrid",
            2 => "manual",
            _ => return "preferences-system-windows".to_string(),
        };

        let dev_path = std::env::current_dir()
            .unwrap_or_default()
            .join("icons")
            .join(format!("{}.svg", name));
        if dev_path.exists() {
            dev_path.to_string_lossy().into_owned()
        } else {
            format!("me.edyan.cardwiretray-{}", name)
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let new_mode = if self.mode == 0 { 1 } else { 0 };
        let mode_desc = if new_mode == 0 {
            "Integrated"
        } else {
            "Hybrid"
        };
        let icon_name_base = if new_mode == 0 {
            "integrated"
        } else {
            "hybrid"
        };

        let dev_path = std::env::current_dir()
            .unwrap_or_default()
            .join("icons")
            .join(format!("{}.svg", icon_name_base));
        let icon = if dev_path.exists() {
            dev_path.to_string_lossy().into_owned()
        } else {
            format!("me.edyan.cardwiretray-{}", icon_name_base)
        };

        let _ = self.action_tx.try_send(TrayAction::Notify(
            format!("Switched to {} mode", mode_desc),
            icon,
        ));
        let _ = self.action_tx.try_send(TrayAction::SetMode(new_mode));
    }

    fn title(&self) -> String {
        "Cardwire".to_string()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let mut tooltip_text = String::from("Name | Power state | Default | Blocked");

        for gpu in &self.gpus {
            let default_str = if gpu.is_default { "✅" } else { "❌" };
            let gpu_blocked_str = if gpu.blocked { "✅" } else { "❌" };
            tooltip_text.push_str(&format!(
                "\n{} | {} | {} | {}",
                gpu.name, gpu.power_state, default_str, gpu_blocked_str
            ));
        }

        ksni::ToolTip {
            title: "Cardwire GPUs".to_string(),
            description: tooltip_text,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let mut items = Vec::new();

        // Modes
        let get_icon = |name: &str| -> String {
            let dev_path = std::env::current_dir()
                .unwrap_or_default()
                .join("icons")
                .join(format!("{}.svg", name));
            if dev_path.exists() {
                dev_path.to_string_lossy().into_owned()
            } else {
                format!("me.edyan.cardwiretray-{}", name)
            }
        };

        let options = vec![
            ksni::menu::RadioItem {
                label: "Integrated Mode".to_string(),
                icon_name: get_icon("integrated"),
                ..Default::default()
            },
            ksni::menu::RadioItem {
                label: "Hybrid Mode".to_string(),
                icon_name: get_icon("hybrid"),
                ..Default::default()
            },
            ksni::menu::RadioItem {
                label: "Manual Mode".to_string(),
                icon_name: get_icon("manual"),
                ..Default::default()
            },
        ];

        let selected_mode_index = if self.mode <= 2 {
            self.mode as usize
        } else {
            0
        };

        items.push(
            ksni::menu::RadioGroup {
                selected: selected_mode_index,
                select: Box::new(|this: &mut Self, index: usize| {
                    let _ = this.action_tx.try_send(TrayAction::SetMode(index as u32));
                }),
                options,
            }
            .into(),
        );

        if self.mode == 2 {
            let mut gpu_items = Vec::new();
            for gpu in &self.gpus {
                if gpu.is_default {
                    continue;
                }

                let gpu_id = gpu.id;
                let is_blocked = gpu.blocked;
                // Checked means NOT blocked
                let is_checked = !is_blocked;

                gpu_items.push(ksni::MenuItem::Checkmark(ksni::menu::CheckmarkItem {
                    label: gpu.name.clone(),
                    checked: is_checked,
                    activate: Box::new(move |this: &mut Self| {
                        // Toggling checked: if it was checked, we uncheck -> means we block (block = true)
                        // If it was unchecked, we check -> means we unblock (block = false)
                        let new_block_state = is_checked;
                        let _ = this
                            .action_tx
                            .try_send(TrayAction::ToggleGpuBlock(gpu_id, new_block_state));
                    }),
                    ..Default::default()
                }));
            }

            if !gpu_items.is_empty() {
                items.push(ksni::MenuItem::Separator);
                items.push(ksni::MenuItem::SubMenu(ksni::menu::SubMenu {
                    label: "Disabled GPUs".to_string(),
                    icon_name: get_icon("gpu"),
                    submenu: gpu_items,
                    ..Default::default()
                }));
            }
        }

        items.push(ksni::MenuItem::Separator);

        items.push(ksni::MenuItem::Standard(ksni::menu::StandardItem {
            label: "Quit".to_string(),
            icon_name: "application-exit".into(),
            activate: Box::new(|this: &mut Self| {
                let _ = this.action_tx.try_send(TrayAction::Quit);
            }),
            ..Default::default()
        }));

        items
    }
}

async fn get_connection() -> Connection {
    loop {
        match Connection::system().await {
            Ok(conn) => return conn,
            Err(_) => {
                let dialog = MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("Cardwire DBus Connection")
                    .set_description("Failed to connect to the DBus system bus.")
                    .set_buttons(rfd::MessageButtons::OkCancelCustom(
                        "Retry".into(),
                        "Quit".into(),
                    ));

                if dialog.show() == rfd::MessageDialogResult::Custom("Quit".to_string()) {
                    std::process::exit(1);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = get_connection().await;

    // Retry loop for the specific service proxy
    let proxy = loop {
        match CardwireProxy::new(&conn).await {
            Ok(p) => {
                if p.mode().await.is_ok() {
                    break p;
                }
            }
            Err(_) => {}
        }

        let dialog = MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Cardwire DBus Service")
            .set_description("Could not access the cardwire DBus service. Is it running?")
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                "Retry".into(),
                "Quit".into(),
            ));

        if dialog.show() == rfd::MessageDialogResult::Custom("Quit".to_string()) {
            std::process::exit(1);
        }
    };

    let initial_mode = proxy.mode().await.unwrap_or(0);

    let mut gpus = Vec::new();
    if let Ok(devs) = proxy.list_devices().await {
        for (_, (id, name, _, _, card, is_default, blocked, _, _)) in devs {
            let power_state = proxy
                .get_status(card)
                .await
                .unwrap_or_else(|_| "Unknown".to_string());
            gpus.push(GpuInfo {
                id,
                name,
                is_default,
                blocked,
                power_state,
            });
        }
        gpus.sort_by_key(|g| g.id);
    }

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
                    if let Ok(devs) = proxy_clone.list_devices().await {
                        let mut new_gpus = Vec::new();
                        for (_, (id, name, _, _, card, is_default, blocked, _, _)) in devs {
                                let power_state = proxy_clone
                                    .get_status(card)
                                    .await
                                    .unwrap_or_else(|_| "Unknown".to_string());
                            new_gpus.push(GpuInfo {
                                id,
                                name,
                                is_default,
                                blocked,
                                power_state,
                            });
                        }
                        new_gpus.sort_by_key(|g| g.id);

                        let _ = handle_clone.update(|tray: &mut CardwireTray| {
                            tray.gpus = new_gpus;
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
