use crate::models::{
    CARDWIRE_GPU_INTERFACE, CARDWIRE_PATH, CARDWIRE_SERVICE, CardwireClient, GitHubRelease, GpuInfo,
};
use zbus::fdo::ObjectManagerProxy;

pub async fn get_gpus(client: &CardwireClient) -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let object_manager = match ObjectManagerProxy::builder(client.connection())
        .destination(CARDWIRE_SERVICE)
        .and_then(|builder| builder.path(CARDWIRE_PATH))
    {
        Ok(builder) => match builder.build().await {
            Ok(proxy) => proxy,
            Err(_) => return gpus,
        },
        Err(_) => return gpus,
    };

    let devices = match object_manager.get_managed_objects().await {
        Ok(devices) => devices,
        Err(_) => return gpus,
    };

    let gpu_path_prefix = format!("{CARDWIRE_PATH}/Gpu/");

    for (path, interfaces) in devices {
        if !interfaces
            .keys()
            .any(|interface| interface.as_str() == CARDWIRE_GPU_INTERFACE)
        {
            continue;
        }

        let Some(id) = path
            .as_str()
            .strip_prefix(&gpu_path_prefix)
            .and_then(|id| id.parse::<u32>().ok())
        else {
            continue;
        };

        let Ok(gpu) = client.gpu_proxy(id).await else {
            continue;
        };

        if let Ok((name, _, _, _, is_default, _, _)) = gpu.get_device().await {
            let blocked = gpu.block().await.unwrap_or(false);
            let power_state = gpu
                .power_state()
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
