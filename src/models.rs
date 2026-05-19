use std::collections::HashMap;

use serde::Deserialize;
use tokio::sync::mpsc;
use zbus::proxy;

// Define the DBus proxy interface
#[proxy(
    interface = "com.github.opengamingcollective.cardwire",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
pub trait Cardwire {
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
pub struct GpuInfo {
    pub id: u32,
    pub name: String,
    pub is_default: bool,
    pub blocked: bool,
    pub power_state: String,
}

pub struct CardwireTray {
    pub mode: u32,
    pub gpus: Vec<GpuInfo>,
    pub action_tx: mpsc::Sender<TrayAction>,
    pub current_version: String,
    pub latest_version: Option<String>,
}

pub enum TrayAction {
    SetMode(u32),
    ToggleGpuBlock(u32, bool),
    Notify(String, String),
    Quit,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
}
