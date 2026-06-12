use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use zbus::{Connection, proxy};

pub const CARDWIRE_SERVICE: &str = "com.github.opengamingcollective.cardwire";
pub const CARDWIRE_PATH: &str = "/com/github/opengamingcollective/cardwire";
pub const CARDWIRE_GPU_INTERFACE: &str = "com.github.opengamingcollective.cardwire.Gpu";

pub type GpuDevice = (String, String, u32, u32, bool, bool, String);
pub type GpuLsof = HashMap<String, Vec<String>>;

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

    pub async fn status(&self) -> zbus::Result<()> {
        CardwireManagerProxy::new(&self.conn).await?.status().await
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

    pub async fn mode(&self) -> zbus::Result<u32> {
        self.mode_proxy().await?.mode().await
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKey {
    AutoApplyGpuState,
    BatteryAutoSwitch,
    ExperimentalNvidiaBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppletMode {
    Integrated,
    Hybrid,
    Manual,
    Smart,
}

impl AppletMode {
    pub const ALL: [AppletMode; 4] = [
        AppletMode::Integrated,
        AppletMode::Hybrid,
        AppletMode::Manual,
        AppletMode::Smart,
    ];

    pub fn value(self) -> u32 {
        match self {
            AppletMode::Integrated => 0,
            AppletMode::Hybrid => 1,
            AppletMode::Manual => 2,
            AppletMode::Smart => 3,
        }
    }

    fn first_except(excluded: AppletMode) -> AppletMode {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| *mode != excluded)
            .unwrap_or(AppletMode::Integrated)
    }
}

impl std::fmt::Display for AppletMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppletMode::Integrated => write!(f, "Integrated"),
            AppletMode::Hybrid => write!(f, "Hybrid"),
            AppletMode::Manual => write!(f, "Manual"),
            AppletMode::Smart => write!(f, "Smart"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AppletConfig {
    pub toggle_from: AppletMode,
    pub toggle_to: AppletMode,
}

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            toggle_from: AppletMode::Integrated,
            toggle_to: AppletMode::Hybrid,
        }
    }
}

impl AppletConfig {
    pub fn set_toggle_from(&mut self, mode: AppletMode) {
        self.toggle_from = mode;

        if self.toggle_to == mode {
            self.toggle_to = AppletMode::first_except(mode);
        }
    }

    pub fn set_toggle_to(&mut self, mode: AppletMode) {
        self.toggle_to = mode;

        if self.toggle_from == mode {
            self.toggle_from = AppletMode::first_except(mode);
        }
    }

    pub fn normalized(mut self) -> Self {
        if self.toggle_from == self.toggle_to {
            self.toggle_to = AppletMode::first_except(self.toggle_from);
        }

        self
    }

    pub fn next_mode(self, current_mode: u32) -> u32 {
        let from = self.toggle_from.value();
        let to = self.toggle_to.value();

        if current_mode == from { to } else { from }
    }

    pub fn load() -> Self {
        let Some(path) = applet_config_path() else {
            return Self::default();
        };

        fs::read_to_string(path)
            .ok()
            .and_then(|contents| toml::from_str::<AppletConfigFile>(&contents).ok())
            .map(AppletConfigFile::into_config)
            .unwrap_or_default()
    }

    pub fn save(self) -> std::io::Result<()> {
        let Some(path) = applet_config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(&self)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        fs::write(path, contents)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct AppletConfigFile {
    toggle_from: Option<AppletMode>,
    toggle_to: Option<AppletMode>,
    left_click_toggle: Option<LegacyTrayToggleMode>,
}

impl AppletConfigFile {
    fn into_config(self) -> AppletConfig {
        let default = AppletConfig::default();

        if self.toggle_from.is_some() || self.toggle_to.is_some() {
            return AppletConfig {
                toggle_from: self.toggle_from.unwrap_or(default.toggle_from),
                toggle_to: self.toggle_to.unwrap_or(default.toggle_to),
            }
            .normalized();
        }

        match self.left_click_toggle {
            Some(LegacyTrayToggleMode::SmartHybrid) => AppletConfig {
                toggle_from: AppletMode::Smart,
                toggle_to: AppletMode::Hybrid,
            },
            Some(LegacyTrayToggleMode::IntegratedHybrid) | None => default,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LegacyTrayToggleMode {
    IntegratedHybrid,
    SmartHybrid,
}

fn applet_config_path() -> Option<PathBuf> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        if !config_home.is_empty() {
            return Some(PathBuf::from(config_home).join("cardwire-tray/config.toml"));
        }
    }

    std::env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".config/cardwire-tray/config.toml"))
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub is_default: bool,
    pub is_nvidia: bool,
    pub nvidia_minor: String,
    pub blocked: bool,
    pub power_state: String,
    pub lsof: GpuLsof,
}

pub struct CardwireTray {
    pub mode: u32,
    pub gpus: Vec<GpuInfo>,
    pub applet_config: std::sync::Arc<std::sync::Mutex<AppletConfig>>,
    pub action_tx: mpsc::Sender<TrayAction>,
    pub current_version: String,
    pub latest_version: Option<String>,
}

pub enum TrayAction {
    OpenWindow,
    SetMode(u32),
    ToggleGpuBlock(u32, bool),
    Notify(String, String),
    Quit,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
}
