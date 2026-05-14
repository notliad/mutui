use ratatui::style::Color;

/// All colors that define a visual theme for the TUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    /// Main background color.
    pub bg: Color,
    /// Primary foreground (normal text).
    pub fg: Color,
    /// Dimmed foreground (hints, counters, inactive elements).
    pub fg_dim: Color,
    /// Accent color (active/focused elements, current track).
    pub accent: Color,
    /// Border color for inactive panels.
    pub border: Color,
    /// Border color for active/focused panels.
    pub border_active: Color,
    /// Background for selected list items.
    pub selection_bg: Color,
    /// Foreground for selected list items and prominent text (titles).
    pub selection_fg: Color,
}

impl Theme {
    pub fn all() -> &'static [Theme] {
        &THEMES
    }

    pub fn default_theme() -> Theme {
        THEMES[0]
    }

    /// Find a theme by name, falling back to the default.
    pub fn by_name(name: &str) -> Theme {
        THEMES
            .iter()
            .copied()
            .find(|t| t.name == name)
            .unwrap_or(THEMES[0])
    }
}

static THEMES: [Theme; 12] = [
    Theme {
        name: "Default",
        bg: Color::Black,
        fg: Color::Gray,
        fg_dim: Color::DarkGray,
        accent: Color::Cyan,
        border: Color::DarkGray,
        border_active: Color::Cyan,
        selection_bg: Color::DarkGray,
        selection_fg: Color::White,
    },
    Theme {
        name: "Dracula",
        bg: Color::Rgb(40, 42, 54),
        fg: Color::Rgb(248, 248, 242),
        fg_dim: Color::Rgb(98, 114, 164),
        accent: Color::Rgb(189, 147, 249),
        border: Color::Rgb(68, 71, 90),
        border_active: Color::Rgb(189, 147, 249),
        selection_bg: Color::Rgb(68, 71, 90),
        selection_fg: Color::Rgb(248, 248, 242),
    },
    Theme {
        name: "Gruvbox Dark",
        bg: Color::Rgb(29, 32, 33),
        fg: Color::Rgb(235, 219, 178),
        fg_dim: Color::Rgb(146, 131, 116),
        accent: Color::Rgb(250, 189, 47),
        border: Color::Rgb(80, 73, 69),
        border_active: Color::Rgb(250, 189, 47),
        selection_bg: Color::Rgb(80, 73, 69),
        selection_fg: Color::Rgb(235, 219, 178),
    },
    Theme {
        name: "Nord",
        bg: Color::Rgb(46, 52, 64),
        fg: Color::Rgb(216, 222, 233),
        fg_dim: Color::Rgb(76, 86, 106),
        accent: Color::Rgb(136, 192, 208),
        border: Color::Rgb(67, 76, 94),
        border_active: Color::Rgb(136, 192, 208),
        selection_bg: Color::Rgb(67, 76, 94),
        selection_fg: Color::Rgb(236, 239, 244),
    },
    Theme {
        name: "Tokyo Night",
        bg: Color::Rgb(26, 27, 38),
        fg: Color::Rgb(169, 177, 214),
        fg_dim: Color::Rgb(86, 95, 137),
        accent: Color::Rgb(122, 162, 247),
        border: Color::Rgb(65, 72, 104),
        border_active: Color::Rgb(122, 162, 247),
        selection_bg: Color::Rgb(45, 50, 80),
        selection_fg: Color::Rgb(192, 202, 245),
    },
    Theme {
        name: "Catppuccin Mocha",
        bg: Color::Rgb(30, 30, 46),
        fg: Color::Rgb(205, 214, 244),
        fg_dim: Color::Rgb(108, 112, 134),
        accent: Color::Rgb(203, 166, 247),
        border: Color::Rgb(69, 71, 90),
        border_active: Color::Rgb(203, 166, 247),
        selection_bg: Color::Rgb(69, 71, 90),
        selection_fg: Color::Rgb(205, 214, 244),
    },
    Theme {
        name: "One Dark",
        bg: Color::Rgb(40, 44, 52),
        fg: Color::Rgb(171, 178, 191),
        fg_dim: Color::Rgb(92, 99, 112),
        accent: Color::Rgb(97, 175, 239),
        border: Color::Rgb(62, 68, 81),
        border_active: Color::Rgb(97, 175, 239),
        selection_bg: Color::Rgb(62, 68, 81),
        selection_fg: Color::Rgb(171, 178, 191),
    },
    Theme {
        name: "Solarized Dark",
        bg: Color::Rgb(0, 43, 54),
        fg: Color::Rgb(131, 148, 150),
        fg_dim: Color::Rgb(88, 110, 117),
        accent: Color::Rgb(42, 161, 152),
        border: Color::Rgb(7, 54, 66),
        border_active: Color::Rgb(42, 161, 152),
        selection_bg: Color::Rgb(7, 54, 66),
        selection_fg: Color::Rgb(147, 161, 161),
    },
    Theme {
        name: "Solarized Light",
        bg: Color::Rgb(253, 246, 227),
        fg: Color::Rgb(101, 123, 131),
        fg_dim: Color::Rgb(147, 161, 161),
        accent: Color::Rgb(42, 161, 152),
        border: Color::Rgb(238, 232, 213),
        border_active: Color::Rgb(42, 161, 152),
        selection_bg: Color::Rgb(238, 232, 213),
        selection_fg: Color::Rgb(88, 110, 117),
    },
    Theme {
        name: "Nord Light",
        bg: Color::Rgb(236, 239, 244),
        fg: Color::Rgb(46, 52, 64),
        fg_dim: Color::Rgb(76, 86, 106),
        accent: Color::Rgb(94, 129, 172),
        border: Color::Rgb(216, 222, 233),
        border_active: Color::Rgb(94, 129, 172),
        selection_bg: Color::Rgb(216, 222, 233),
        selection_fg: Color::Rgb(46, 52, 64),
    },
    Theme {
        name: "Papercolor Light",
        bg: Color::Rgb(255, 255, 255),
        fg: Color::Rgb(68, 68, 68),
        fg_dim: Color::Rgb(135, 135, 135),
        accent: Color::Rgb(0, 95, 135),
        border: Color::Rgb(188, 188, 188),
        border_active: Color::Rgb(0, 95, 135),
        selection_bg: Color::Rgb(215, 215, 215),
        selection_fg: Color::Rgb(68, 68, 68),
    },
    Theme {
        name: "Gruvbox Light",
        bg: Color::Rgb(251, 241, 199),
        fg: Color::Rgb(60, 56, 54),
        fg_dim: Color::Rgb(146, 131, 116),
        accent: Color::Rgb(215, 153, 33),
        border: Color::Rgb(213, 196, 161),
        border_active: Color::Rgb(215, 153, 33),
        selection_bg: Color::Rgb(213, 196, 161),
        selection_fg: Color::Rgb(60, 56, 54),
    },
];
