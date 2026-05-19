use crate::models::{CardwireProxy, GitHubRelease, GpuInfo};

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

pub async fn get_latest_version() -> Option<String> {
    let url = "https://api.github.com/repos/JuanDelPueblo/cardwire-tray/releases/latest";

    let client = reqwest::Client::new();

    let release = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "my-rust-app")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<GitHubRelease>()
        .await
        .ok()?;

    Some(release.tag_name)
}
