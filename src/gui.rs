use crate::{
    models::{
        AppletConfig, CardwireClient, CardwireConfigState, ConfigKey, GpuInfo, TrayToggleMode,
    },
    utils::get_gpus,
};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Shadow, Size, Subscription, Task,
    Theme, Vector, border,
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
    TrayToggleChanged(TrayToggleMode),
    ClearError,
}

pub fn run_gui_daemon(
    client: CardwireClient,
    applet_config: Arc<Mutex<AppletConfig>>,
    open_requested: Arc<AtomicBool>,
) -> iced::Result {
    iced::daemon("Cardwire", CardwireGui::update, CardwireGui::view)
        .subscription(CardwireGui::subscription)
        .theme(|_, _| app_theme())
        .run_with(move || {
            (
                CardwireGui::new(client, applet_config, open_requested),
                Task::none(),
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
            selected_page: Page::Config,
            snapshot: None,
            loading: false,
            error: None,
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
            Message::TrayToggleChanged(left_click_toggle) => {
                self.applet_config_state.left_click_toggle = left_click_toggle;

                if let Ok(mut config) = self.applet_config.lock() {
                    config.left_click_toggle = left_click_toggle;
                    if let Err(error) = config.save() {
                        self.error = Some(format!("Could not save tray applet settings: {error}"));
                    }
                }

                Task::none()
            }
            Message::ClearError => {
                self.error = None;
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
            Page::Config => self.view_config(),
            Page::Gpu(gpu_id) => self.view_gpu(*gpu_id),
        };

        container(
            row![
                self.sidebar(),
                container(scrollable(content))
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

        if let Some(id) = self.window_id {
            Task::batch([window::gain_focus(id), Self::load_task(self.client.clone())])
        } else {
            let (id, open) = window::open(window_settings());
            self.window_id = Some(id);

            Task::batch([
                open.map(Message::WindowOpened),
                Self::load_task(self.client.clone()),
            ])
        }
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let mode = self.snapshot.as_ref().map(|snapshot| snapshot.mode);
        let mut sidebar = Column::new()
            .spacing(10)
            .padding(16)
            .push(text("Cardwire").size(26))
            .push(sidebar_button(
                "Config",
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

    fn view_config(&self) -> Element<'_, Message> {
        let mut content = Column::new()
            .spacing(16)
            .push(text("Config").size(30))
            .push(self.status_section());

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
            content = content
                .push(mode_section(snapshot.mode))
                .push(daemon_config_section(snapshot.config));
        } else {
            content = content.push(text("Waiting for daemon data..."));
        }

        content = content.push(self.tray_applet_section());

        container(content).width(Fill).into()
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
            row![
                text("Left-click toggle").width(Length::FillPortion(1)),
                pick_list(
                    TrayToggleMode::ALL.as_slice(),
                    Some(self.applet_config_state.left_click_toggle),
                    Message::TrayToggleChanged,
                )
                .width(Length::FillPortion(2))
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
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
    window::Settings {
        size: Size::new(960.0, 640.0),
        position: window::Position::Centered,
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
    .align_y(Alignment::Center);

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
    button(text(label.into()).size(14))
        .width(Fill)
        .height(38)
        .padding([0, 12])
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

fn root_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        text_color: Some(palette.background.base.text),
        ..container::Style::default()
    }
}

fn sidebar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.base.text),
        border: Border {
            color: palette.background.strong.color.scale_alpha(0.35),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.base.text),
        border: border::rounded(8)
            .width(1)
            .color(palette.background.strong.color.scale_alpha(0.45)),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
    }
}

fn error_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(Background::Color(palette.danger.weak.color)),
        text_color: Some(palette.danger.weak.text),
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
    let palette = theme.extended_palette();
    let mut style = iced::widget::button::Style {
        text_color: palette.background.base.text,
        border: border::rounded(8),
        ..iced::widget::button::Style::default()
    };

    if selected {
        style.background = Some(Background::Color(palette.primary.weak.color));
        style.text_color = palette.primary.weak.text;
        style.border = border::rounded(8)
            .width(1)
            .color(palette.primary.base.color.scale_alpha(0.45));
    }

    match status {
        iced::widget::button::Status::Hovered => {
            style.background = Some(Background::Color(if selected {
                palette.primary.base.color
            } else {
                palette.background.strong.color.scale_alpha(0.45)
            }));
        }
        iced::widget::button::Status::Pressed => {
            style.background = Some(Background::Color(palette.primary.base.color));
        }
        iced::widget::button::Status::Disabled => {
            style.text_color = style.text_color.scale_alpha(0.45);
        }
        iced::widget::button::Status::Active => {}
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
