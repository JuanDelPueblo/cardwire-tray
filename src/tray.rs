use crate::models::{CardwireTray, TrayAction};
use ksni::Tray;

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
