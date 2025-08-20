use tui_widgets::scrollview::ScrollViewState;

mod render;

#[derive(Default, Clone, Copy)]
enum SelectedTab {
    #[default]
    Overview,
    Diagnostic,
    Configuration,
    Commands,
    Help,
}

struct ScrollState {
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

    fn scroll_down(&mut self) {
        self.position = self.position.saturating_sub(1);
        // When scrolled all the way to the bottom, auto follow the feed ("sticky behavior")
        self.follow = self.position == 0;
        self.state.scroll_down();
    }

    fn scroll_up(&mut self) {
        self.follow = false;
        // Can't scroll up when already on top
        if self.state.offset().y != 0 {
            self.position = self.position.saturating_add(1);
        }
        self.state.scroll_up();
    }

    fn get_state_for_rendering(&mut self) -> &mut ScrollViewState {
        // For the "sticky" behavior, where the view remains at the bottom
        // Needs to be done during rendering as more content could have been added, making
        // a jump to the bottom necessary
        if self.follow {
            self.state.scroll_to_bottom();
        }

        &mut self.state
    }
}

pub struct UiState {
    device_path: Option<String>,
    overview_scroll: ScrollState,
    diagnostic_scroll: ScrollState,
    configuration_scroll: ScrollState,
    command_scroll: ScrollState,
    help_scroll: ScrollState,
    current_tab: SelectedTab,
    riot_board: String,
    riot_version: String,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            device_path: None,

            current_tab: SelectedTab::Overview,

            overview_scroll: ScrollState::new(),
            diagnostic_scroll: ScrollState::new(),
            configuration_scroll: ScrollState::new(),
            command_scroll: ScrollState::new(),
            help_scroll: ScrollState::new(),

            riot_board: "Unkown".to_owned(),
            riot_version: "Unkown".to_owned(),
        }
    }

    pub fn set_board_name(&mut self, name: String) {
        self.riot_board = name;
    }

    pub fn set_board_version(&mut self, version: String) {
        self.riot_version = version;
    }

    pub fn set_device_path(&mut self, path: String) {
        self.device_path = Some(path);
    }

    pub fn clear_device_path(&mut self) {
        self.device_path = None;
    }

    fn get_config(&self) -> String {
        format!(
            "Version: {}\nBoard: {}\n",
            self.riot_version, self.riot_board,
        )
    }

    fn get_connection(&self) -> String {
        match &self.device_path {
            Some(device_path) => {
                format!(
                    "✅ connected via {device_path} with RIOT {}",
                    self.riot_version
                )
            }
            None => "❌ not connected, retrying..".to_owned(),
        }
    }

    pub fn scroll_down(&mut self) {
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_down();
                self.configuration_scroll.scroll_down();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_down(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_down(),
            SelectedTab::Commands => self.command_scroll.scroll_down(),
            SelectedTab::Help => self.help_scroll.scroll_down(),
        }
    }

    pub fn scroll_up(&mut self) {
        match self.current_tab {
            SelectedTab::Overview => {
                self.overview_scroll.scroll_up();
                self.configuration_scroll.scroll_up();
            }
            SelectedTab::Diagnostic => self.diagnostic_scroll.scroll_up(),
            SelectedTab::Configuration => self.configuration_scroll.scroll_up(),
            SelectedTab::Commands => self.command_scroll.scroll_up(),
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
}
