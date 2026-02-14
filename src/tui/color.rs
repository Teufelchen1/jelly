use clap::ValueEnum;
use ratatui::style::Color;
use ratatui::style::Style;
use terminal_colorsaurus::QueryOptions;
use terminal_colorsaurus::ThemeMode;
use terminal_colorsaurus::theme_mode;

#[derive(Clone, Debug, Default, ValueEnum)]
pub enum ColorTheme {
    #[default]
    Auto,
    Light,
    Dark,
    Riot,
    None,
}

pub struct ColorPalette {
    border: Style,
    tab_selected: Style,
    downlight: Style,
    title: Style,
}

impl ColorPalette {
    const fn new_reset() -> Self {
        Self {
            border: Style::reset(),
            tab_selected: Style::reset(),
            downlight: Style::reset(),
            title: Style::reset(),
        }
    }

    const fn new_dark() -> Self {
        Self {
            border: Style::new().gray(),
            tab_selected: Style::new().fg(Color::Black).bg(Color::White),
            downlight: Style::new().dark_gray(),
            title: Style::new().white(),
        }
    }

    fn new_light() -> Self {
        Self {
            border: Style::new().dark_gray(),
            tab_selected: Style::new().fg(Color::White).bg(Color::Black),
            downlight: Color::Indexed(240).into(),
            title: Style::new().black(),
        }
    }

    const fn new_riot() -> Self {
        Self {
            border: Style::new().red(),
            tab_selected: Style::new().fg(Color::Black).bg(Color::Green),
            downlight: Style::new().green(),
            title: Style::new().green(),
        }
    }

    pub fn from(theme: &ColorTheme) -> Self {
        match theme {
            ColorTheme::Auto => {
                match theme_mode(QueryOptions::default()).unwrap_or(ThemeMode::Dark) {
                    ThemeMode::Dark => Self::new_dark(),
                    ThemeMode::Light => Self::new_light(),
                }
            }
            ColorTheme::Light => Self::new_light(),
            ColorTheme::Dark => Self::new_dark(),
            ColorTheme::Riot => Self::new_riot(),
            ColorTheme::None => Self::new_reset(),
        }
    }

    pub const fn border(&self) -> Style {
        self.border
    }

    pub const fn tab_selected(&self) -> Style {
        self.tab_selected
    }

    pub const fn downlight(&self) -> Style {
        self.downlight
    }

    pub const fn title(&self) -> Style {
        self.title
    }
}
