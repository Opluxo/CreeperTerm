use serde::Deserialize;

use crate::terminal::buffer::Color;

#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: ThemeColors,
    pub font: ThemeFont,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeColors {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeFont {
    pub family: String,
    pub size: u32,
    pub line_height: f32,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            name: "Default".to_string(),
            colors: ThemeColors {
                background: Color::Rgb(30, 30, 30),
                foreground: Color::Rgb(220, 220, 220),
                cursor: Color::Rgb(255, 255, 255),
                selection: Color::Rgb(68, 71, 90),
                black: Color::Black,
                red: Color::Red,
                green: Color::Green,
                yellow: Color::Yellow,
                blue: Color::Blue,
                magenta: Color::Magenta,
                cyan: Color::Cyan,
                white: Color::White,
                bright_black: Color::BrightBlack,
                bright_red: Color::BrightRed,
                bright_green: Color::BrightGreen,
                bright_yellow: Color::BrightYellow,
                bright_blue: Color::BrightBlue,
                bright_magenta: Color::BrightMagenta,
                bright_cyan: Color::BrightCyan,
                bright_white: Color::BrightWhite,
            },
            font: ThemeFont {
                family: "Fira Code".to_string(),
                size: 14,
                line_height: 1.2,
            },
        }
    }

    pub fn dracula_theme() -> Self {
        Self {
            name: "Dracula".to_string(),
            colors: ThemeColors {
                background: Color::Rgb(40, 42, 54),
                foreground: Color::Rgb(248, 248, 242),
                cursor: Color::Rgb(248, 248, 242),
                selection: Color::Rgb(68, 71, 90),
                black: Color::Rgb(0, 0, 0),
                red: Color::Rgb(255, 85, 85),
                green: Color::Rgb(80, 250, 123),
                yellow: Color::Rgb(241, 250, 140),
                blue: Color::Rgb(98, 114, 164),
                magenta: Color::Rgb(255, 121, 198),
                cyan: Color::Rgb(139, 233, 253),
                white: Color::Rgb(255, 255, 255),
                bright_black: Color::Rgb(85, 85, 85),
                bright_red: Color::Rgb(255, 121, 121),
                bright_green: Color::Rgb(115, 255, 155),
                bright_yellow: Color::Rgb(255, 255, 170),
                bright_blue: Color::Rgb(130, 145, 200),
                bright_magenta: Color::Rgb(255, 160, 220),
                bright_cyan: Color::Rgb(170, 240, 255),
                bright_white: Color::Rgb(255, 255, 255),
            },
            font: ThemeFont {
                family: "Fira Code".to_string(),
                size: 14,
                line_height: 1.2,
            },
        }
    }

    pub fn color_to_iced(color: &Color) -> iced::Color {
        match color {
            Color::Black => iced::Color::from_rgb(0.0, 0.0, 0.0),
            Color::Red => iced::Color::from_rgb(0.7, 0.0, 0.0),
            Color::Green => iced::Color::from_rgb(0.0, 0.7, 0.0),
            Color::Yellow => iced::Color::from_rgb(0.7, 0.7, 0.0),
            Color::Blue => iced::Color::from_rgb(0.0, 0.0, 0.7),
            Color::Magenta => iced::Color::from_rgb(0.7, 0.0, 0.7),
            Color::Cyan => iced::Color::from_rgb(0.0, 0.7, 0.7),
            Color::White => iced::Color::from_rgb(0.7, 0.7, 0.7),
            Color::BrightBlack => iced::Color::from_rgb(0.3, 0.3, 0.3),
            Color::BrightRed => iced::Color::from_rgb(1.0, 0.3, 0.3),
            Color::BrightGreen => iced::Color::from_rgb(0.3, 1.0, 0.3),
            Color::BrightYellow => iced::Color::from_rgb(1.0, 1.0, 0.3),
            Color::BrightBlue => iced::Color::from_rgb(0.3, 0.3, 1.0),
            Color::BrightMagenta => iced::Color::from_rgb(1.0, 0.3, 1.0),
            Color::BrightCyan => iced::Color::from_rgb(0.3, 1.0, 1.0),
            Color::BrightWhite => iced::Color::from_rgb(1.0, 1.0, 1.0),
            Color::Rgb(r, g, b) => {
                iced::Color::from_rgb(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0)
            }
            Color::Indexed(idx) => Self::indexed_color(*idx),
        }
    }

    fn indexed_color(idx: u8) -> iced::Color {
        match idx {
            0 => iced::Color::from_rgb(0.0, 0.0, 0.0),
            1 => iced::Color::from_rgb(0.7, 0.0, 0.0),
            2 => iced::Color::from_rgb(0.0, 0.7, 0.0),
            3 => iced::Color::from_rgb(0.7, 0.7, 0.0),
            4 => iced::Color::from_rgb(0.0, 0.0, 0.7),
            5 => iced::Color::from_rgb(0.7, 0.0, 0.7),
            6 => iced::Color::from_rgb(0.0, 0.7, 0.7),
            7 => iced::Color::from_rgb(0.7, 0.7, 0.7),
            8 => iced::Color::from_rgb(0.3, 0.3, 0.3),
            9 => iced::Color::from_rgb(1.0, 0.3, 0.3),
            10 => iced::Color::from_rgb(0.3, 1.0, 0.3),
            11 => iced::Color::from_rgb(1.0, 1.0, 0.3),
            12 => iced::Color::from_rgb(0.3, 0.3, 1.0),
            13 => iced::Color::from_rgb(1.0, 0.3, 1.0),
            14 => iced::Color::from_rgb(0.3, 1.0, 1.0),
            15 => iced::Color::from_rgb(1.0, 1.0, 1.0),
            _ => {
                // 256-color approximation
                let r = ((idx / 36) % 6) as f32 / 5.0;
                let g = ((idx / 6) % 6) as f32 / 5.0;
                let b = (idx % 6) as f32 / 5.0;
                iced::Color::from_rgb(r, g, b)
            }
        }
    }
}
