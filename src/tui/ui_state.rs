use std::fmt::Write;

use ratatui::style::Color;
use ratatui::style::Style;
use terminal_colorsaurus::QueryOptions;
use terminal_colorsaurus::ThemeMode;
use terminal_colorsaurus::theme_mode;
use tui_widgets::scrollview::ScrollViewState;

#[derive(Default, Clone, Copy)]
pub enum SelectedTab {
    #[default]
    Overview,
    Diagnostic,
    Configuration,
    Commands,
    Net,
    Help,
}

pub struct ScrollState {
    state: ScrollViewState,
    position: usize,
    follow: bool,
}

impl ScrollState {
    fn new() -> Self {
        Self {
            state: ScrollViewState::default(),
            position: 0,
            follow: true,
        }
    }

    const fn scroll_down(&mut self) {
        self.position = self.position.saturating_sub(1);
        // When scrolled all the way to the bottom, auto follow the feed ("sticky behavior")
        self.follow = self.position == 0;
        self.state.scroll_down();
    }

    const fn scroll_up(&mut self) {
        self.follow = false;
        // Can't scroll up when already on top
        if self.state.offset().y != 0 {
            self.position = self.position.saturating_add(1);
        }
        self.state.scroll_up();
    }

    pub fn get_state_for_rendering(&mut self) -> &mut ScrollViewState {
        // For the "sticky" behavior, where the view remains at the bottom
        // Needs to be done during rendering as more content could have been added, making
        // a jump to the bottom necessary
        if self.follow {
            self.state.scroll_to_bottom();
        }

        &mut self.state
    }
}

struct ColorPalette {
    border: Style,
    tab_selected: Style,
    downlight: Style,
}

impl ColorPalette {
    const fn new_dark() -> Self {
        Self {
            border: Style::new().gray(),
            tab_selected: Style::new().fg(Color::Black).bg(Color::White),
            downlight: Style::new().dark_gray(),
        }
    }

    fn new_light() -> Self {
        Self {
            border: Style::new().dark_gray(),
            tab_selected: Style::new().fg(Color::White).bg(Color::Black),
            downlight: Color::Indexed(240).into(),
        }
    }

    fn from(theme: ThemeMode) -> Self {
        match theme {
            ThemeMode::Dark => Self::new_dark(),
            ThemeMode::Light => Self::new_light(),
        }
    }
}

pub struct UiState {
    device_path: Option<String>,
    iface_name: Option<String>,
    pub overview_scroll: ScrollState,
    pub diagnostic_scroll: ScrollState,
    pub configuration_scroll: ScrollState,
    pub command_scroll: ScrollState,
    pub net_scroll: ScrollState,
    pub help_scroll: ScrollState,
    pub current_tab: SelectedTab,
    pub command_help_list: String,
    riot_board: String,
    riot_version: String,
    theme: ColorPalette,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            device_path: None,
            iface_name: None,

            current_tab: SelectedTab::Overview,

            overview_scroll: ScrollState::new(),
            diagnostic_scroll: ScrollState::new(),
            configuration_scroll: ScrollState::new(),
            command_scroll: ScrollState::new(),
            net_scroll: ScrollState::new(),
            help_scroll: ScrollState::new(),

            command_help_list: String::new(),

            riot_board: "Unknown".to_owned(),
            riot_version: "Unknown".to_owned(),

            theme: ColorPalette::from(
                theme_mode(QueryOptions::default()).unwrap_or(ThemeMode::Dark),
            ),
        }
    }

    pub const fn border_style(&self) -> Style {
        self.theme.border
    }

    pub const fn selected_style(&self) -> Style {
        self.theme.tab_selected
    }

    pub const fn downlight(&self) -> Style {
        self.theme.downlight
    }

    pub fn set_command_help_list(&mut self, cmds: Vec<(String, String, String)>) {
        self.command_help_list.clear();
        for (cmd, description, help) in cmds {
            if help.is_empty() {
                let _ = writeln!(self.command_help_list, "{cmd:<20}: {description}");
            } else {
                let _ = writeln!(
                    self.command_help_list,
                    "{cmd:<20}: {description} | see --help for more information"
                );
            }
        }
    }

    pub fn set_board_name(&mut self, name: String) {
        self.riot_board = name;
    }

    pub fn set_board_version(&mut self, version: String) {
        self.riot_version = version;
    }

    pub fn set_iface_name(&mut self, name: String) {
        self.iface_name = Some(name);
    }

    pub fn set_device_path(&mut self, path: String) {
        self.device_path = Some(path);
    }

    pub fn clear_device_path(&mut self) {
        self.device_path = None;
    }

    pub fn get_config(&self) -> String {
        format!(
            "Version: {}\nBoard: {}\n",
            self.riot_version, self.riot_board,
        )
    }

    pub fn get_connection(&self) -> String {
        let net = match &self.iface_name {
            Some(iface_name) => {
                format!(" | Network via {iface_name}")
            }
            None => String::new(),
        };
        match &self.device_path {
            Some(device_path) => {
                format!("✅ connected via {device_path}{net}")
            }
            None => format!("❌ not connected, retrying..{net}"),
        }
    }

    pub const fn scroll_down(&mut self) {
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_down();
                self.configuration_scroll.scroll_down();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_down(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_down(),
            SelectedTab::Commands => self.command_scroll.scroll_down(),
            SelectedTab::Net => self.net_scroll.scroll_down(),
            SelectedTab::Help => self.help_scroll.scroll_down(),
        }
    }

    pub const fn scroll_up(&mut self) {
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_up();
                self.configuration_scroll.scroll_up();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_up(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_up(),
            SelectedTab::Commands => self.command_scroll.scroll_up(),
            SelectedTab::Net => self.net_scroll.scroll_up(),
            SelectedTab::Help => self.help_scroll.scroll_up(),
        }
    }

    pub const fn select_overview_view(&mut self) {
        self.current_tab = SelectedTab::Overview;
    }

    pub const fn select_diagnostic_view(&mut self) {
        self.current_tab = SelectedTab::Diagnostic;
    }

    pub const fn select_configuration_view(&mut self) {
        self.current_tab = SelectedTab::Configuration;
    }

    pub const fn select_commands_view(&mut self) {
        self.current_tab = SelectedTab::Commands;
    }

    pub const fn select_help_view(&mut self) {
        self.current_tab = SelectedTab::Help;
    }

    pub const fn select_net_view(&mut self) {
        self.current_tab = SelectedTab::Net;
    }
}
