use std::collections::HashMap;

use serde::Deserialize;
use tokio::sync::mpsc;
use zbus::{Connection, proxy};

pub const CARDWIRE_SERVICE: &str = "com.github.opengamingcollective.cardwire";
pub const CARDWIRE_PATH: &str = "/com/github/opengamingcollective/cardwire";
pub const CARDWIRE_GPU_INTERFACE: &str = "com.github.opengamingcollective.cardwire.Gpu";

pub type GpuDevice = (String, String, u32, u32, bool, bool, String);

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Manager",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
pub trait CardwireManager {
    /// RefreshGpu method
    fn refresh_gpu(&self) -> zbus::Result<()>;

    /// Status method
    fn status(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Mode",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
pub trait CardwireMode {
    /// Mode property
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_mode(&self, mode: u32) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Config",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
pub trait CardwireConfig {
    /// AutoApplyGpuState property
    #[zbus(property)]
    fn auto_apply_gpu_state(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_auto_apply_gpu_state(&self, enabled: bool) -> zbus::Result<()>;

    /// BatteryAutoSwitch property
    #[zbus(property)]
    fn battery_auto_switch(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_battery_auto_switch(&self, enabled: bool) -> zbus::Result<()>;

    /// ExperimentalNvidiaBlock property
    #[zbus(property)]
    fn experimental_nvidia_block(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_experimental_nvidia_block(&self, enabled: bool) -> zbus::Result<()>;

    /// SaveToFile method
    fn save_to_file(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Gpu",
    default_service = "com.github.opengamingcollective.cardwire"
)]
pub trait CardwireGpu {
    /// Block property
    #[zbus(property)]
    fn block(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_block(&self, block: bool) -> zbus::Result<()>;

    /// GetDevice method
    fn get_device(&self) -> zbus::Result<GpuDevice>;

    /// PowerState method
    fn power_state(&self) -> zbus::Result<String>;

    /// Lsof method
    fn lsof(&self) -> zbus::Result<HashMap<String, Vec<String>>>;

    /// PowerStateChanged signal
    #[zbus(signal)]
    fn power_state_changed(&self, state: String) -> zbus::Result<()>;
}

#[derive(Debug, Clone)]
pub struct CardwireClient {
    conn: Connection,
}

impl CardwireClient {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub async fn mode_proxy(&self) -> zbus::Result<CardwireModeProxy<'static>> {
        CardwireModeProxy::new(&self.conn).await
    }

    pub async fn config_proxy(&self) -> zbus::Result<CardwireConfigProxy<'static>> {
        CardwireConfigProxy::new(&self.conn).await
    }

    pub async fn gpu_proxy(&self, gpu_id: u32) -> zbus::Result<CardwireGpuProxy<'static>> {
        CardwireGpuProxy::builder(&self.conn)
            .path(format!("{CARDWIRE_PATH}/Gpu/{gpu_id}"))?
            .build()
            .await
    }

    pub async fn set_mode(&self, mode: u32) -> zbus::Result<()> {
        self.mode_proxy().await?.set_mode(mode).await
    }

    pub async fn config(&self) -> zbus::Result<CardwireConfigState> {
        let proxy = self.config_proxy().await?;
        Ok(CardwireConfigState {
            auto_apply_gpu_state: proxy.auto_apply_gpu_state().await?,
            battery_auto_switch: proxy.battery_auto_switch().await?,
            experimental_nvidia_block: proxy.experimental_nvidia_block().await?,
        })
    }

    pub async fn set_config(&self, key: ConfigKey, enabled: bool) -> zbus::Result<()> {
        let proxy = self.config_proxy().await?;
        match key {
            ConfigKey::AutoApplyGpuState => proxy.set_auto_apply_gpu_state(enabled).await?,
            ConfigKey::BatteryAutoSwitch => proxy.set_battery_auto_switch(enabled).await?,
            ConfigKey::ExperimentalNvidiaBlock => {
                proxy.set_experimental_nvidia_block(enabled).await?
            }
        }
        proxy.save_to_file().await
    }

    pub async fn set_gpu_block(&self, gpu_id: u32, block: bool) -> zbus::Result<()> {
        self.gpu_proxy(gpu_id).await?.set_block(block).await
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CardwireConfigState {
    pub auto_apply_gpu_state: bool,
    pub battery_auto_switch: bool,
    pub experimental_nvidia_block: bool,
}

impl CardwireConfigState {
    pub fn get(self, key: ConfigKey) -> bool {
        match key {
            ConfigKey::AutoApplyGpuState => self.auto_apply_gpu_state,
            ConfigKey::BatteryAutoSwitch => self.battery_auto_switch,
            ConfigKey::ExperimentalNvidiaBlock => self.experimental_nvidia_block,
        }
    }

    pub fn set(&mut self, key: ConfigKey, enabled: bool) {
        match key {
            ConfigKey::AutoApplyGpuState => self.auto_apply_gpu_state = enabled,
            ConfigKey::BatteryAutoSwitch => self.battery_auto_switch = enabled,
            ConfigKey::ExperimentalNvidiaBlock => self.experimental_nvidia_block = enabled,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigKey {
    AutoApplyGpuState,
    BatteryAutoSwitch,
    ExperimentalNvidiaBlock,
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
    pub config: CardwireConfigState,
    pub action_tx: mpsc::Sender<TrayAction>,
    pub current_version: String,
    pub latest_version: Option<String>,
}

pub enum TrayAction {
    SetMode(u32),
    ToggleGpuBlock(u32, bool),
    SetConfig(ConfigKey, bool),
    Notify(String, String),
    Quit,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
}
