use crate::models::{CardwireProxy, GpuInfo};

pub async fn get_gpus(proxy: &CardwireProxy<'_>) -> Vec<GpuInfo> {
    let mut gpus = Vec::new();
    if let Ok(devices) = proxy.list_devices().await {
        for (_, (id, name, _, _, _, is_default, blocked, _, _)) in devices {
            let power_state = proxy
                .get_status(id)
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
    }
    gpus.sort_by_key(|g| g.id);
    gpus
}
