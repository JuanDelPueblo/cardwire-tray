use crate::{
    models::{AppletConfig, AppletMode, CardwireClient, CardwireConfigState, ConfigKey, GpuInfo},
    utils::{get_gpus, get_latest_version},
};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Shadow, Size, Subscription, Task,
    Theme, Vector, border, font, Font,
    widget::{
        Column, Row, button, checkbox, column, container, horizontal_rule, pick_list, radio, row,
        scrollable, text,
    },
    window,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Status,
    Config,
    Gpu(u32),
}

#[derive(Debug, Clone)]
struct GuiSnapshot {
    daemon_online: bool,
    mode: u32,
    config: CardwireConfigState,
    gpus: Vec<GpuInfo>,
}

#[derive(Debug, Clone)]
enum Message {
    SnapshotLoaded(Result<GuiSnapshot, String>),
    OpenPoll,
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    RefreshTick,
    SelectPage(Page),
    ModeSelected(u32),
    DaemonConfigToggled(ConfigKey, bool),
    GpuBlockToggled(u32, bool),
    TrayToggleFromChanged(AppletMode),
    TrayToggleToChanged(AppletMode),
    ClearError,
    CheckUpdate,
    UpdateChecked(Option<String>),
    OpenUrl(String),
}

pub fn run_gui_daemon(
    client: CardwireClient,
    applet_config: Arc<Mutex<AppletConfig>>,
    open_requested: Arc<AtomicBool>,
) -> iced::Result {
    iced::daemon("Cardwire GUI", CardwireGui::update, CardwireGui::view)
        .subscription(CardwireGui::subscription)
        .theme(|_, _| app_theme())
        .run_with(move || {
            (
                CardwireGui::new(client, applet_config, open_requested),
                Task::perform(get_latest_version(), Message::UpdateChecked),
            )
        })
}

struct CardwireGui {
    client: CardwireClient,
    applet_config: Arc<Mutex<AppletConfig>>,
    open_requested: Arc<AtomicBool>,
    applet_config_state: AppletConfig,
    window_id: Option<window::Id>,
    selected_page: Page,
    snapshot: Option<GuiSnapshot>,
    loading: bool,
    error: Option<String>,
    latest_version: Option<String>,
    checking_update: bool,
}

impl CardwireGui {
    fn new(
        client: CardwireClient,
        applet_config: Arc<Mutex<AppletConfig>>,
        open_requested: Arc<AtomicBool>,
    ) -> Self {
        let applet_config_state = applet_config
            .lock()
            .map(|config| *config)
            .unwrap_or_default();

        Self {
            client,
            applet_config,
            open_requested,
            applet_config_state,
            window_id: None,
            selected_page: Page::Status,
            snapshot: None,
            loading: false,
            error: None,
            latest_version: None,
            checking_update: false,
        }
    }

    fn load_task(client: CardwireClient) -> Task<Message> {
        Task::perform(load_snapshot(client), Message::SnapshotLoaded)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SnapshotLoaded(result) => {
                self.loading = false;

                match result {
                    Ok(snapshot) => {
                        self.snapshot = Some(snapshot);
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }

                Task::none()
            }
            Message::OpenPoll => {
                if self.open_requested.swap(false, Ordering::SeqCst) {
                    self.open_or_focus_window()
                } else {
                    Task::none()
                }
            }
            Message::WindowOpened(id) => {
                self.window_id = Some(id);
                Task::none()
            }
            Message::WindowClosed(id) => {
                if self.window_id == Some(id) {
                    self.window_id = None;
                }

                Task::none()
            }
            Message::RefreshTick => {
                if self.window_id.is_none() || self.loading {
                    Task::none()
                } else {
                    self.loading = true;
                    Self::load_task(self.client.clone())
                }
            }
            Message::SelectPage(page) => {
                self.selected_page = page;
                Task::none()
            }
            Message::ModeSelected(mode) => {
                self.loading = true;
                Task::perform(
                    set_mode_and_load(self.client.clone(), mode),
                    Message::SnapshotLoaded,
                )
            }
            Message::DaemonConfigToggled(key, enabled) => {
                self.loading = true;
                Task::perform(
                    set_config_and_load(self.client.clone(), key, enabled),
                    Message::SnapshotLoaded,
                )
            }
            Message::GpuBlockToggled(gpu_id, block) => {
                self.loading = true;
                Task::perform(
                    set_gpu_block_and_load(self.client.clone(), gpu_id, block),
                    Message::SnapshotLoaded,
                )
            }
            Message::TrayToggleFromChanged(mode) => {
                self.update_applet_config(|config| config.set_toggle_from(mode));
                Task::none()
            }
            Message::TrayToggleToChanged(mode) => {
                self.update_applet_config(|config| config.set_toggle_to(mode));
                Task::none()
            }
            Message::ClearError => {
                self.error = None;
                Task::none()
            }
            Message::CheckUpdate => {
                self.checking_update = true;
                Task::perform(get_latest_version(), Message::UpdateChecked)
            }
            Message::UpdateChecked(result) => {
                self.checking_update = false;
                self.latest_version = result;
                Task::none()
            }
            Message::OpenUrl(url) => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(url)
                    .spawn();
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(250)).map(|_| Message::OpenPoll),
            iced::time::every(Duration::from_secs(5)).map(|_| Message::RefreshTick),
            window::close_events().map(Message::WindowClosed),
        ])
    }

    fn view(&self, _window_id: window::Id) -> Element<'_, Message> {
        let content = match &self.selected_page {
            Page::Status => self.view_status(),
            Page::Config => self.view_config(),
            Page::Gpu(gpu_id) => self.view_gpu(*gpu_id),
        };

        container(
            row![
                self.sidebar(),
                container(scrollable(
                    container(content)
                        .width(Fill)
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 16.0,
                            bottom: 0.0,
                            left: 0.0,
                        })
                ))
                .width(Fill)
                .height(Fill)
                .padding([24, 28])
            ]
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .style(root_style)
        .into()
    }

    fn open_or_focus_window(&mut self) -> Task<Message> {
        self.loading = true;
        self.checking_update = true;

        if let Some(id) = self.window_id {
            Task::batch([
                window::gain_focus(id),
                Self::load_task(self.client.clone()),
                Task::perform(get_latest_version(), Message::UpdateChecked),
            ])
        } else {
            let (id, open) = window::open(window_settings());
            self.window_id = Some(id);

            Task::batch([
                open.map(Message::WindowOpened),
                Self::load_task(self.client.clone()),
                Task::perform(get_latest_version(), Message::UpdateChecked),
            ])
        }
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let mode = self.snapshot.as_ref().map(|snapshot| snapshot.mode);
        let mut sidebar = Column::new()
            .spacing(10)
            .padding(16)
            .width(Fill)
            .push(text("Cardwire GUI").size(26))
            .push(sidebar_button(
                "Status",
                self.selected_page == Page::Status,
                Message::SelectPage(Page::Status),
            ))
            .push(sidebar_button(
                "Settings",
                self.selected_page == Page::Config,
                Message::SelectPage(Page::Config),
            ))
            .push(horizontal_rule(1))
            .push(
                text("Available GPUs")
                    .size(15)
                    .style(iced::widget::text::secondary),
            );

        if let Some(snapshot) = &self.snapshot {
            if snapshot.gpus.is_empty() {
                sidebar = sidebar.push(text("No GPUs reported").size(14));
            } else {
                for gpu in &snapshot.gpus {
                    sidebar = sidebar.push(gpu_sidebar_entry(
                        gpu,
                        mode == Some(2),
                        self.selected_page == Page::Gpu(gpu.id),
                    ));
                }
            }
        } else if self.loading {
            sidebar = sidebar.push(text("Loading GPUs...").size(14));
        } else {
            sidebar = sidebar.push(text("No daemon snapshot").size(14));
        }

        container(scrollable(sidebar))
            .width(280)
            .height(Fill)
            .style(sidebar_style)
            .into()
    }

    fn view_status(&self) -> Element<'_, Message> {
        let mut content = Column::new()
            .spacing(16)
            .push(text("Status").size(30))
            .push(self.status_section());

        if let Some(snapshot) = &self.snapshot {
            content = content
                .push(mode_section(snapshot.mode))
                .push(self.gpu_summary_section(&snapshot.gpus));
        } else {
            content = content.push(text("Waiting for daemon data..."));
        }

        container(content).width(Fill).into()
    }

    fn view_config(&self) -> Element<'_, Message> {
        let mut content = Column::new()
            .spacing(16)
            .push(text("Settings").size(30));

        if let Some(error) = &self.error {
            content = content.push(
                container(
                    row![
                        text(format!("Error: {error}")),
                        button("Dismiss").on_press(Message::ClearError)
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding(14)
                .style(error_panel_style),
            );
        }

        if let Some(snapshot) = &self.snapshot {
            content = content.push(daemon_config_section(snapshot.config));
        } else {
            content = content.push(text("Waiting for daemon data..."));
        }

        content = content
            .push(self.tray_applet_section())
            .push(self.update_checker_section())
            .push(self.credits_section());

        container(content).width(Fill).into()
    }

    fn gpu_summary_section(&self, gpus: &[GpuInfo]) -> Element<'_, Message> {
        let mut table = Column::new().spacing(10);
        
        if gpus.is_empty() {
            table = table.push(text("No GPUs detected."));
            return section("GPU Status Summary", table);
        }

        // Header Row
        table = table.push(
            row![
                text("GPU Name").width(Length::FillPortion(3)),
                text("Power State").width(Length::FillPortion(2)),
                text("Primary").width(Length::FillPortion(1)),
                text("Blocked").width(Length::FillPortion(1)),
            ]
            .spacing(12)
            .padding([4, 0])
        );

        table = table.push(horizontal_rule(1));

        for gpu in gpus {
            let power_color = match gpu.power_state.to_lowercase().as_str() {
                "active" | "on" => Color::from_rgb(0.2, 0.7, 0.3),
                "suspended" | "off" => Color::from_rgb(0.5, 0.5, 0.5),
                _ => Color::from_rgb(0.8, 0.5, 0.2),
            };

            let blocked_color = if gpu.blocked {
                Color::from_rgb(0.8, 0.2, 0.2)
            } else {
                Color::from_rgb(0.2, 0.7, 0.3)
            };

            let primary_text = if gpu.is_default { "Yes" } else { "No" };
            let blocked_text = if gpu.blocked { "Yes" } else { "No" };

            table = table.push(
                row![
                    text(gpu.name.clone()).width(Length::FillPortion(3)),
                    text(gpu.power_state.clone())
                        .color(power_color)
                        .width(Length::FillPortion(2)),
                    text(primary_text).width(Length::FillPortion(1)),
                    text(blocked_text)
                        .color(blocked_color)
                        .width(Length::FillPortion(1)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            );
        }

        section("GPU Status Summary", table)
    }

    fn credits_section(&self) -> Element<'_, Message> {
        section(
            "Credits",
            column![
                row![
                    link_button("JuanDelPueblo", "https://github.com/JuanDelPueblo"),
                    text(" (for developing "),
                    link_button("Cardwire Tray", "https://github.com/JuanDelPueblo/cardwire-tray"),
                    text(")")
                ]
                .align_y(Alignment::Center),
                row![
                    link_button("Luytan", "https://github.com/luytan"),
                    text(" (for developing "),
                    link_button("Cardwire", "https://github.com/OpenGamingCollective/cardwire"),
                    text(")")
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
    }



    fn status_section(&self) -> Element<'_, Message> {
        let daemon_status = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                if snapshot.daemon_online {
                    "Running"
                } else {
                    "Unavailable"
                }
            })
            .unwrap_or("Checking");

        section(
            "Cardwire daemon",
            column![info_row("Status", daemon_status)].spacing(8),
        )
    }

    fn tray_applet_section(&self) -> Element<'_, Message> {
        section(
            "Tray applet",
            column![
                row![
                    text("Left-click toggles from").width(Length::FillPortion(1)),
                    pick_list(
                        AppletMode::ALL.as_slice(),
                        Some(self.applet_config_state.toggle_from),
                        Message::TrayToggleFromChanged,
                    )
                    .width(Length::FillPortion(2))
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                row![
                    text("Left-click toggles to").width(Length::FillPortion(1)),
                    pick_list(
                        AppletMode::ALL.as_slice(),
                        Some(self.applet_config_state.toggle_to),
                        Message::TrayToggleToChanged,
                    )
                    .width(Length::FillPortion(2))
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
    }

    fn update_checker_section(&self) -> Element<'_, Message> {
        let current_version = format!("v{}", env!("CARGO_PKG_VERSION"));

        let status_text = if self.checking_update {
            text("Checking for updates...").style(iced::widget::text::secondary)
        } else if let Some(latest) = &self.latest_version {
            if latest == &current_version {
                text(format!("You are running the latest version! ({})", current_version))
                    .color(Color::from_rgb(0.2, 0.7, 0.3))
            } else {
                text(format!("Update available! Latest: {} (Current: {})", latest, current_version))
                    .color(Color::from_rgb(0.8, 0.2, 0.2))
            }
        } else {
            text(format!("Current version: {}", current_version))
                .style(iced::widget::text::secondary)
        };

        let check_button = if self.checking_update {
            button(
                container(text("Checking...").size(13))
                    .width(160)
                    .height(34)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
            )
            .padding([0, 12])
        } else {
            button(
                container(text("Check for Updates").size(13))
                    .width(160)
                    .height(34)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
            )
            .padding([0, 12])
            .on_press(Message::CheckUpdate)
        };

        section(
            "Software updates",
            row![
                status_text.width(Length::FillPortion(1)),
                check_button
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(Fill),
        )
    }

    fn update_applet_config(&mut self, update: impl FnOnce(&mut AppletConfig)) {
        let save_result = if let Ok(mut config) = self.applet_config.lock() {
            update(&mut config);
            self.applet_config_state = *config;
            config.save()
        } else {
            return;
        };

        if let Err(error) = save_result {
            self.error = Some(format!("Could not save tray applet settings: {error}"));
        }
    }

    fn view_gpu(&self, gpu_id: u32) -> Element<'_, Message> {
        let Some(snapshot) = &self.snapshot else {
            return container(text("Waiting for daemon data..."))
                .width(Fill)
                .into();
        };

        let Some(gpu) = snapshot.gpus.iter().find(|gpu| gpu.id == gpu_id) else {
            return container(text("This GPU is no longer reported by Cardwire."))
                .width(Fill)
                .into();
        };

        let content = Column::new()
            .spacing(16)
            .push(text(format!("GPU {}: {}", gpu.id, gpu.name)).size(28))
            .push(gpu_info_section(gpu));

        let content = content.push(lsof_section(gpu));

        container(content).width(Fill).into()
    }
}

async fn load_snapshot(client: CardwireClient) -> Result<GuiSnapshot, String> {
    let daemon_online = client.status().await.is_ok();
    let mode = client.mode().await.unwrap_or(0);
    let config = client.config().await.unwrap_or_default();
    let gpus = get_gpus(&client).await;

    Ok(GuiSnapshot {
        daemon_online,
        mode,
        config,
        gpus,
    })
}

async fn set_mode_and_load(client: CardwireClient, mode: u32) -> Result<GuiSnapshot, String> {
    client
        .set_mode(mode)
        .await
        .map_err(|error| format!("Could not set mode: {error}"))?;
    load_snapshot(client).await
}

async fn set_config_and_load(
    client: CardwireClient,
    key: ConfigKey,
    enabled: bool,
) -> Result<GuiSnapshot, String> {
    client
        .set_config(key, enabled)
        .await
        .map_err(|error| format!("Could not update daemon config: {error}"))?;
    load_snapshot(client).await
}

async fn set_gpu_block_and_load(
    client: CardwireClient,
    gpu_id: u32,
    block: bool,
) -> Result<GuiSnapshot, String> {
    client
        .set_gpu_block(gpu_id, block)
        .await
        .map_err(|error| format!("Could not update GPU block state: {error}"))?;
    load_snapshot(client).await
}

fn window_settings() -> window::Settings {
    let icon_bytes = include_bytes!("../icons/gpu.rgba");
    let icon = window::icon::from_rgba(icon_bytes.to_vec(), 64, 64).expect("Failed to load window icon");

    window::Settings {
        size: Size::new(960.0, 640.0),
        position: window::Position::Centered,
        icon: Some(icon),
        platform_specific: window::settings::PlatformSpecific {
            application_id: String::from("me.edyan.cardwiretray"),
            ..Default::default()
        },
        ..window::Settings::default()
    }
}

fn app_theme() -> Theme {
    if Theme::default() == Theme::Light {
        Theme::KanagawaLotus
    } else {
        Theme::KanagawaDragon
    }
}

fn section<'a>(
    title: &'static str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![text(title).size(20), content.into()]
            .spacing(12)
            .width(Fill),
    )
    .width(Fill)
    .padding(18)
    .style(panel_style)
    .into()
}

fn gpu_sidebar_entry(
    gpu: &GpuInfo,
    manual_mode: bool,
    selected: bool,
) -> Element<'static, Message> {
    let mut entry = row![sidebar_button(
        format!("GPU {}: {}", gpu.id, gpu.name),
        selected,
        Message::SelectPage(Page::Gpu(gpu.id)),
    )]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Fill);

    if manual_mode {
        if gpu.is_default {
            entry = entry.push(container(text("")).width(26));
        } else {
            let gpu_id = gpu.id;
            entry = entry.push(
                checkbox("", gpu.blocked)
                    .size(18)
                    .spacing(0)
                    .on_toggle(move |block| Message::GpuBlockToggled(gpu_id, block)),
            );
        }
    }

    entry.into()
}

fn sidebar_button(
    label: impl Into<String>,
    selected: bool,
    message: Message,
) -> Element<'static, Message> {
    button(
        container(
            text(label.into())
                .size(13)
                .font(Font {
                    weight: font::Weight::Medium,
                    ..Font::default()
                })
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(38)
    .padding([0, 16])
    .style(move |theme, status| sidebar_button_style(theme, status, selected))
    .on_press(message)
    .into()
}

fn mode_section(current_mode: u32) -> Element<'static, Message> {
    let mut modes = Column::new().spacing(10);

    for (value, label) in MODE_CHOICES {
        modes = modes.push(radio(
            label,
            value,
            Some(current_mode),
            Message::ModeSelected,
        ));
    }

    section("Mode", modes)
}

fn daemon_config_section(config: CardwireConfigState) -> Element<'static, Message> {
    section(
        "Cardwire daemon settings",
        column![
            checkbox(
                "Auto-apply saved GPU block states",
                config.auto_apply_gpu_state
            )
            .on_toggle(|enabled| {
                Message::DaemonConfigToggled(ConfigKey::AutoApplyGpuState, enabled)
            }),
            checkbox(
                "Automatically switch modes on battery power",
                config.battery_auto_switch
            )
            .on_toggle(|enabled| Message::DaemonConfigToggled(
                ConfigKey::BatteryAutoSwitch,
                enabled
            )),
            checkbox(
                "Use experimental NVIDIA blocking",
                config.experimental_nvidia_block
            )
            .on_toggle(|enabled| {
                Message::DaemonConfigToggled(ConfigKey::ExperimentalNvidiaBlock, enabled)
            }),
        ]
        .spacing(10),
    )
}

fn gpu_info_section(gpu: &GpuInfo) -> Element<'static, Message> {
    section(
        "Device",
        column![
            info_row("Name", gpu.name.clone()),
            info_row("PCI address", empty_or_value(&gpu.pci)),
            info_row(
                "Render node",
                format!("minor {} (/dev/dri/renderD{})", gpu.render, gpu.render)
            ),
            info_row(
                "Card node",
                format!("minor {} (/dev/dri/card{})", gpu.card, gpu.card)
            ),
            info_row("Default display GPU", yes_no(gpu.is_default)),
            info_row("NVIDIA device", yes_no(gpu.is_nvidia)),
            info_row("NVIDIA minor", empty_or_value(&gpu.nvidia_minor)),
            info_row("Power state", empty_or_value(&gpu.power_state)),
            info_row("Blocked", yes_no(gpu.blocked)),
        ]
        .spacing(8),
    )
}

fn lsof_section(gpu: &GpuInfo) -> Element<'static, Message> {
    let mut table = Column::new().spacing(6);

    if gpu.lsof.is_empty() {
        table = table.push(text("No applications currently reported by lsof."));
        return section("Open GPU file descriptors", table);
    }

    table = table.push(lsof_row("Device node", "Applications"));

    let mut entries = gpu.lsof.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    for (path, processes) in entries {
        let mut processes = processes.clone();
        processes.sort();

        let process_text = if processes.is_empty() {
            "No process names reported".to_string()
        } else {
            processes.join(", ")
        };

        table = table.push(lsof_row(path, process_text));
    }

    section("Open GPU file descriptors", table)
}

fn is_dark_mode(theme: &Theme) -> bool {
    !matches!(theme, Theme::Light | Theme::KanagawaLotus)
}

// Custom premium palette for dark mode
const DARK_BG_ROOT: Color = Color::from_rgb(0.082, 0.086, 0.110);      // #15161c
const DARK_BG_SIDEBAR: Color = Color::from_rgb(0.055, 0.059, 0.075);   // #0e0f13
const DARK_BG_PANEL: Color = Color::from_rgb(0.114, 0.122, 0.153);     // #1d1f27
const DARK_BORDER_PANEL: Color = Color::from_rgb(0.165, 0.173, 0.216); // #2a2c37
const DARK_TEXT_PRIMARY: Color = Color::from_rgb(0.957, 0.957, 0.973); // #f4f4f8
const DARK_TEXT_SECONDARY: Color = Color::from_rgb(0.627, 0.647, 0.706); // #a0a5b4
const DARK_PRIMARY: Color = Color::from_rgb(0.259, 0.447, 0.843);      // #4272d7
const DARK_PRIMARY_TEXT: Color = Color::from_rgb(1.0, 1.0, 1.0);

// Custom premium palette for light mode
const LIGHT_BG_ROOT: Color = Color::from_rgb(0.957, 0.965, 0.976);     // #f4f6f9
const LIGHT_BG_SIDEBAR: Color = Color::from_rgb(0.902, 0.918, 0.937);  // #e6eaef
const LIGHT_BG_PANEL: Color = Color::from_rgb(1.0, 1.0, 1.0);          // #ffffff
const LIGHT_BORDER_PANEL: Color = Color::from_rgb(0.835, 0.859, 0.890); // #d5dbde
const LIGHT_TEXT_PRIMARY: Color = Color::from_rgb(0.086, 0.090, 0.114); // #16171d
const LIGHT_TEXT_SECONDARY: Color = Color::from_rgb(0.404, 0.431, 0.490); // #676e7d
const LIGHT_PRIMARY: Color = Color::from_rgb(0.212, 0.412, 0.784);     // #3669c8
const LIGHT_PRIMARY_TEXT: Color = Color::from_rgb(1.0, 1.0, 1.0);

fn root_style(theme: &Theme) -> container::Style {
    let is_dark = is_dark_mode(theme);
    container::Style {
        background: Some(Background::Color(if is_dark { DARK_BG_ROOT } else { LIGHT_BG_ROOT })),
        text_color: Some(if is_dark { DARK_TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY }),
        ..container::Style::default()
    }
}

fn sidebar_style(theme: &Theme) -> container::Style {
    let is_dark = is_dark_mode(theme);
    container::Style {
        background: Some(Background::Color(if is_dark { DARK_BG_SIDEBAR } else { LIGHT_BG_SIDEBAR })),
        text_color: Some(if is_dark { DARK_TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY }),
        border: Border {
            color: if is_dark { DARK_BORDER_PANEL } else { LIGHT_BORDER_PANEL },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn panel_style(theme: &Theme) -> container::Style {
    let is_dark = is_dark_mode(theme);
    container::Style {
        background: Some(Background::Color(if is_dark { DARK_BG_PANEL } else { LIGHT_BG_PANEL })),
        text_color: Some(if is_dark { DARK_TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY }),
        border: border::rounded(12)
            .width(1)
            .color(if is_dark { DARK_BORDER_PANEL } else { LIGHT_BORDER_PANEL }),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, if is_dark { 0.22 } else { 0.05 }),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
    }
}

fn error_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let is_dark = is_dark_mode(theme);

    container::Style {
        background: Some(Background::Color(if is_dark {
            Color::from_rgb(0.25, 0.08, 0.08)
        } else {
            palette.danger.weak.color
        })),
        text_color: Some(if is_dark {
            Color::from_rgb(1.0, 0.8, 0.8)
        } else {
            palette.danger.weak.text
        }),
        border: border::rounded(8)
            .width(1)
            .color(palette.danger.base.color.scale_alpha(0.55)),
        ..container::Style::default()
    }
}

fn sidebar_button_style(
    theme: &Theme,
    status: iced::widget::button::Status,
    selected: bool,
) -> iced::widget::button::Style {
    let is_dark = is_dark_mode(theme);
    let mut style = iced::widget::button::Style {
        text_color: if is_dark { DARK_TEXT_SECONDARY } else { LIGHT_TEXT_SECONDARY },
        border: border::rounded(8),
        background: None,
        ..iced::widget::button::Style::default()
    };

    if selected {
        style.background = Some(Background::Color(if is_dark { DARK_PRIMARY } else { LIGHT_PRIMARY }));
        style.text_color = if is_dark { DARK_PRIMARY_TEXT } else { LIGHT_PRIMARY_TEXT };
    } else {
        match status {
            iced::widget::button::Status::Hovered => {
                style.background = Some(Background::Color(if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.05)
                }));
                style.text_color = if is_dark { DARK_TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY };
            }
            iced::widget::button::Status::Pressed => {
                style.background = Some(Background::Color(if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.1)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.1)
                }));
                style.text_color = if is_dark { DARK_TEXT_PRIMARY } else { LIGHT_TEXT_PRIMARY };
            }
            _ => {}
        }
    }

    style
}

fn info_row(label: &'static str, value: impl Into<String>) -> Row<'static, Message> {
    row![
        text(label).width(Length::FillPortion(1)),
        text(value.into()).width(Length::FillPortion(2))
    ]
    .spacing(12)
}

fn lsof_row(left: impl Into<String>, right: impl Into<String>) -> Row<'static, Message> {
    row![
        container(text(left.into())).width(Length::FillPortion(2)),
        container(text(right.into())).width(Length::FillPortion(3))
    ]
    .spacing(12)
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn empty_or_value(value: &str) -> String {
    if value.is_empty() {
        "Not reported".to_string()
    } else {
        value.to_string()
    }
}

const MODE_CHOICES: [(u32, &str); 4] = [
    (0, "Integrated"),
    (1, "Hybrid"),
    (2, "Manual"),
    (3, "Smart"),
];

fn link_button_style(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let is_dark = is_dark_mode(theme);
    let mut style = iced::widget::button::Style {
        text_color: if is_dark {
            Color::from_rgb(0.4, 0.6, 1.0)
        } else {
            Color::from_rgb(0.1, 0.4, 0.8)
        },
        background: None,
        border: border::rounded(0),
        ..iced::widget::button::Style::default()
    };

    match status {
        iced::widget::button::Status::Hovered => {
            style.text_color = if is_dark {
                Color::from_rgb(0.6, 0.8, 1.0)
            } else {
                Color::from_rgb(0.2, 0.5, 0.9)
            };
        }
        _ => {}
    }

    style
}

fn link_button<'a>(label: &'a str, url: &'a str) -> Element<'a, Message> {
    button(text(label).size(14))
        .padding(0)
        .style(move |theme, status| link_button_style(theme, status))
        .on_press(Message::OpenUrl(url.to_string()))
        .into()
}
