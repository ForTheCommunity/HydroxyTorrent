use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Dracula,
    Nord,
    Matrix,
    Light,
}

impl ThemeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::Matrix => "Matrix",
            Self::Light => "Light",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Dracula,
            Self::Dracula => Self::Nord,
            Self::Nord => Self::Matrix,
            Self::Matrix => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Dark => ThemePalette {
                bg: Color::Rgb(15, 17, 23),
                panel_bg: Color::Rgb(22, 25, 34),
                header_bg: Color::Rgb(28, 33, 45),
                selected_bg: Color::Rgb(38, 52, 74),
                border: Color::Rgb(54, 62, 79),
                border_focused: Color::Rgb(88, 166, 255),
                text_primary: Color::Rgb(230, 237, 243),
                text_secondary: Color::Rgb(139, 148, 158),
                text_muted: Color::Rgb(95, 104, 115),
                accent_blue: Color::Rgb(88, 166, 255),
                accent_green: Color::Rgb(63, 185, 80),
                accent_amber: Color::Rgb(210, 153, 34),
                accent_red: Color::Rgb(248, 81, 73),
                accent_purple: Color::Rgb(188, 140, 255),
            },
            Self::Dracula => ThemePalette {
                bg: Color::Rgb(40, 42, 54),
                panel_bg: Color::Rgb(33, 34, 44),
                header_bg: Color::Rgb(52, 55, 70),
                selected_bg: Color::Rgb(68, 71, 90),
                border: Color::Rgb(98, 114, 164),
                border_focused: Color::Rgb(189, 147, 249),
                text_primary: Color::Rgb(248, 248, 242),
                text_secondary: Color::Rgb(189, 195, 218),
                text_muted: Color::Rgb(98, 114, 164),
                accent_blue: Color::Rgb(139, 233, 253),
                accent_green: Color::Rgb(80, 250, 123),
                accent_amber: Color::Rgb(241, 250, 140),
                accent_red: Color::Rgb(255, 85, 85),
                accent_purple: Color::Rgb(255, 121, 198),
            },
            Self::Nord => ThemePalette {
                bg: Color::Rgb(46, 52, 64),
                panel_bg: Color::Rgb(59, 66, 82),
                header_bg: Color::Rgb(67, 76, 94),
                selected_bg: Color::Rgb(76, 86, 106),
                border: Color::Rgb(76, 86, 106),
                border_focused: Color::Rgb(136, 192, 208),
                text_primary: Color::Rgb(236, 239, 244),
                text_secondary: Color::Rgb(216, 222, 233),
                text_muted: Color::Rgb(129, 161, 193),
                accent_blue: Color::Rgb(136, 192, 208),
                accent_green: Color::Rgb(163, 190, 140),
                accent_amber: Color::Rgb(235, 203, 139),
                accent_red: Color::Rgb(191, 97, 106),
                accent_purple: Color::Rgb(180, 142, 173),
            },
            Self::Matrix => ThemePalette {
                bg: Color::Rgb(6, 12, 8),
                panel_bg: Color::Rgb(10, 20, 13),
                header_bg: Color::Rgb(14, 30, 18),
                selected_bg: Color::Rgb(20, 52, 28),
                border: Color::Rgb(28, 90, 42),
                border_focused: Color::Rgb(0, 255, 102),
                text_primary: Color::Rgb(170, 255, 190),
                text_secondary: Color::Rgb(90, 200, 115),
                text_muted: Color::Rgb(45, 120, 62),
                accent_blue: Color::Rgb(0, 220, 150),
                accent_green: Color::Rgb(0, 255, 102),
                accent_amber: Color::Rgb(180, 255, 80),
                accent_red: Color::Rgb(255, 90, 90),
                accent_purple: Color::Rgb(80, 255, 180),
            },
            Self::Light => ThemePalette {
                bg: Color::Rgb(246, 248, 250),
                panel_bg: Color::Rgb(255, 255, 255),
                header_bg: Color::Rgb(234, 238, 242),
                selected_bg: Color::Rgb(218, 234, 252),
                border: Color::Rgb(208, 215, 222),
                border_focused: Color::Rgb(9, 105, 218),
                text_primary: Color::Rgb(31, 35, 40),
                text_secondary: Color::Rgb(87, 96, 106),
                text_muted: Color::Rgb(140, 149, 159),
                accent_blue: Color::Rgb(9, 105, 218),
                accent_green: Color::Rgb(26, 127, 55),
                accent_amber: Color::Rgb(154, 103, 0),
                accent_red: Color::Rgb(207, 34, 46),
                accent_purple: Color::Rgb(130, 80, 223),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub bg: Color,
    pub panel_bg: Color,
    pub header_bg: Color,
    pub selected_bg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub accent_blue: Color,
    pub accent_green: Color,
    pub accent_amber: Color,
    pub accent_red: Color,
    pub accent_purple: Color,
}
