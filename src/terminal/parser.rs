use crate::terminal::buffer::{Buffer, Cell, CellAttributes, Color};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserState {
    Ground,
    Escape,
    Csi,
    Osc,
    Dcs,
}

pub struct Parser {
    state: ParserState,
    params: Vec<u32>,
    intermediate: Vec<u8>,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediate: Vec::new(),
        }
    }

    pub fn parse(&mut self, buffer: &mut Buffer, data: &[u8]) {
        for &byte in data {
            match self.state {
                ParserState::Ground => {
                    if byte == 0x1b {
                        self.state = ParserState::Escape;
                        self.params.clear();
                        self.intermediate.clear();
                    } else if byte == b'\r' {
                        let (_, y) = buffer.cursor_position();
                        buffer.move_cursor(0, y);
                    } else if byte == b'\n' || byte == 0x0b || byte == 0x0c {
                        let (x, y) = buffer.cursor_position();
                        let new_y = (y + 1).min(buffer.viewport_height() - 1);
                        if new_y == buffer.viewport_height() - 1 {
                            buffer.scroll_up(1);
                        } else {
                            buffer.move_cursor(x, new_y);
                        }
                    } else if byte == 0x08 {
                        let (x, y) = buffer.cursor_position();
                        if x > 0 {
                            buffer.move_cursor(x - 1, y);
                        }
                    } else if byte == b'\t' {
                        let (x, y) = buffer.cursor_position();
                        let new_x = ((x / 8) + 1) * 8;
                        buffer.move_cursor(new_x.min(buffer.viewport_width() - 1), y);
                    } else if byte == 0x07 {
                        // Bell - ignore
                    } else if byte >= 0x20 {
                        let (x, y) = buffer.cursor_position();
                        if x < buffer.viewport_width() {
                            let cell = Cell {
                                char: byte as char,
                                attributes: CellAttributes::default(),
                            };
                            buffer.set_cell(x, y, cell);
                            if x + 1 >= buffer.viewport_width() {
                                buffer.move_cursor(0, (y + 1).min(buffer.viewport_height() - 1));
                            } else {
                                buffer.move_cursor(x + 1, y);
                            }
                        }
                    }
                }
                ParserState::Escape => {
                    if byte == b'[' {
                        self.state = ParserState::Csi;
                    } else if byte == b']' {
                        self.state = ParserState::Osc;
                    } else if byte == b'P' {
                        self.state = ParserState::Dcs;
                    } else if byte == b'M' {
                        let (x, y) = buffer.cursor_position();
                        if y > 0 {
                            buffer.move_cursor(x, y - 1);
                        } else {
                            buffer.scroll_down(1);
                        }
                        self.state = ParserState::Ground;
                    } else if byte == b'7' {
                        buffer.save_cursor();
                        self.state = ParserState::Ground;
                    } else if byte == b'8' {
                        buffer.restore_cursor();
                        self.state = ParserState::Ground;
                    } else if byte == b'D' {
                        let (x, y) = buffer.cursor_position();
                        let new_y = (y + 1).min(buffer.viewport_height() - 1);
                        buffer.move_cursor(x, new_y);
                        self.state = ParserState::Ground;
                    } else {
                        self.state = ParserState::Ground;
                    }
                }
                ParserState::Csi => {
                    if byte >= 0x40 && byte <= 0x7e {
                        self.execute_csi(buffer, byte);
                        self.state = ParserState::Ground;
                        self.params.clear();
                        self.intermediate.clear();
                    } else if byte == 0x3b {
                        self.params.push(0);
                    } else if byte >= 0x30 && byte <= 0x3f {
                        if self.params.is_empty() {
                            self.params.push(0);
                        }
                        let last = self.params.last_mut().unwrap();
                        *last = *last * 10 + (byte - 0x30) as u32;
                    } else if byte >= 0x20 && byte <= 0x2f {
                        self.intermediate.push(byte);
                    }
                }
                ParserState::Osc => {
                    if byte == 0x07 || byte == 0x9c {
                        self.state = ParserState::Ground;
                    }
                }
                ParserState::Dcs => {
                    if byte == 0x9c || byte == 0x5c {
                        self.state = ParserState::Ground;
                    }
                }
            }
        }
    }

    fn execute_csi(&self, buffer: &mut Buffer, final_byte: u8) {
        let params = &self.params;
        let param = |i: usize| -> u32 { params.get(i).copied().unwrap_or(0) };

        match final_byte {
            b'A' => {
                let (x, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor(x, y.saturating_sub(n));
            }
            b'B' => {
                let (x, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor(x, (y + n).min(buffer.viewport_height() - 1));
            }
            b'C' => {
                let (x, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor((x + n).min(buffer.viewport_width() - 1), y);
            }
            b'D' => {
                let (x, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor(x.saturating_sub(n), y);
            }
            b'E' => {
                let (_, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor(0, (y + n).min(buffer.viewport_height() - 1));
            }
            b'F' => {
                let (_, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor(0, y.saturating_sub(n));
            }
            b'G' => {
                let (_, y) = buffer.cursor_position();
                let n = param(0).max(1) as usize;
                buffer.move_cursor((n - 1).min(buffer.viewport_width() - 1), y);
            }
            b'H' | b'f' => {
                let row = param(0).max(1) as usize;
                let col = param(1).max(1) as usize;
                buffer.move_cursor(
                    (col - 1).min(buffer.viewport_width() - 1),
                    (row - 1).min(buffer.viewport_height() - 1),
                );
            }
            b'J' => {
                let mode = param(0);
                match mode {
                    0 => buffer.clear_from_cursor_to_end(),
                    1 => buffer.clear_from_start_to_cursor(),
                    2 | 3 => buffer.clear(),
                    _ => {}
                }
            }
            b'K' => {
                let mode = param(0);
                let (x, y) = buffer.cursor_position();
                match mode {
                    0 => {
                        if let Some(line) = buffer.lines().get(y) {
                            for cx in x..line.cells.len() {
                                buffer.set_cell(
                                    cx,
                                    y,
                                    Cell {
                                        char: ' ',
                                        attributes: CellAttributes::default(),
                                    },
                                );
                            }
                        }
                    }
                    1 => {
                        for cx in 0..=x {
                            buffer.set_cell(
                                cx,
                                y,
                                Cell {
                                    char: ' ',
                                    attributes: CellAttributes::default(),
                                },
                            );
                        }
                    }
                    2 => buffer.clear_line(y),
                    _ => {}
                }
            }
            b'L' => {
                let n = param(0).max(1) as usize;
                let (_, y) = buffer.cursor_position();
                buffer.insert_lines(y, n);
            }
            b'M' => {
                let n = param(0).max(1) as usize;
                let (_, y) = buffer.cursor_position();
                buffer.delete_lines(y, n);
            }
            b'P' => {
                let n = param(0).max(1) as usize;
                let (x, y) = buffer.cursor_position();
                buffer.delete_chars(x, y, n);
            }
            b'S' => {
                let n = param(0).max(1) as usize;
                buffer.scroll_up(n);
            }
            b'T' => {
                let n = param(0).max(1) as usize;
                buffer.scroll_down(n);
            }
            b'X' => {
                let n = param(0).max(1) as usize;
                let (x, y) = buffer.cursor_position();
                buffer.erase_chars(x, y, n);
            }
            b'd' => {
                let row = param(0).max(1) as usize;
                let (x, _) = buffer.cursor_position();
                buffer.move_cursor(x, (row - 1).min(buffer.viewport_height() - 1));
            }
            b'm' => self.execute_sgr(buffer, params),
            b'r' => {
                // Set scrolling region - TODO
            }
            b's' => {
                buffer.save_cursor();
            }
            b'u' => {
                buffer.restore_cursor();
            }
            _ => {}
        }
    }

    fn execute_sgr(&self, buffer: &mut Buffer, params: &[u32]) {
        let (x, y) = buffer.cursor_position();

        if params.is_empty() || (params.len() == 1 && params[0] == 0) {
            // Reset all attributes
            if let Some(line) = buffer.lines().get(y) {
                if x < line.cells.len() {
                    buffer.set_cell(
                        x,
                        y,
                        Cell {
                            char: line.cells[x].char,
                            attributes: CellAttributes::default(),
                        },
                    );
                }
            }
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    // Reset
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: CellAttributes::default(),
                                },
                            );
                        }
                    }
                }
                1 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.bold = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                2 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.dim = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                3 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.italic = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                4 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.underline = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                7 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.reverse = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                9 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.strikethrough = true;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                22 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.bold = false;
                            attrs.dim = false;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                23 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.italic = false;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                24 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.underline = false;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                27 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.reverse = false;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                29 => {
                    if let Some(line) = buffer.lines().get(y) {
                        if x < line.cells.len() {
                            let mut attrs = line.cells[x].attributes.clone();
                            attrs.strikethrough = false;
                            buffer.set_cell(
                                x,
                                y,
                                Cell {
                                    char: line.cells[x].char,
                                    attributes: attrs,
                                },
                            );
                        }
                    }
                }
                // Standard foreground colors
                30 => self.set_fg(buffer, x, y, Color::Black),
                31 => self.set_fg(buffer, x, y, Color::Red),
                32 => self.set_fg(buffer, x, y, Color::Green),
                33 => self.set_fg(buffer, x, y, Color::Yellow),
                34 => self.set_fg(buffer, x, y, Color::Blue),
                35 => self.set_fg(buffer, x, y, Color::Magenta),
                36 => self.set_fg(buffer, x, y, Color::Cyan),
                37 => self.set_fg(buffer, x, y, Color::White),
                38 => {
                    // Extended foreground color
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 => {
                                // 256-color: ESC[38;5;Nm
                                if i + 2 < params.len() {
                                    let color_idx = params[i + 2] as u8;
                                    self.set_fg(buffer, x, y, Color::Indexed(color_idx));
                                    i += 2;
                                }
                            }
                            2 => {
                                // RGB: ESC[38;2;R;G;Bm
                                if i + 4 < params.len() {
                                    let r = params[i + 2] as u8;
                                    let g = params[i + 3] as u8;
                                    let b = params[i + 4] as u8;
                                    self.set_fg(buffer, x, y, Color::Rgb(r, g, b));
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.set_fg(buffer, x, y, Color::White),
                // Standard background colors
                40 => self.set_bg(buffer, x, y, Color::Black),
                41 => self.set_bg(buffer, x, y, Color::Red),
                42 => self.set_bg(buffer, x, y, Color::Green),
                43 => self.set_bg(buffer, x, y, Color::Yellow),
                44 => self.set_bg(buffer, x, y, Color::Blue),
                45 => self.set_bg(buffer, x, y, Color::Magenta),
                46 => self.set_bg(buffer, x, y, Color::Cyan),
                47 => self.set_bg(buffer, x, y, Color::White),
                48 => {
                    // Extended background color
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 => {
                                if i + 2 < params.len() {
                                    let color_idx = params[i + 2] as u8;
                                    self.set_bg(buffer, x, y, Color::Indexed(color_idx));
                                    i += 2;
                                }
                            }
                            2 => {
                                if i + 4 < params.len() {
                                    let r = params[i + 2] as u8;
                                    let g = params[i + 3] as u8;
                                    let b = params[i + 4] as u8;
                                    self.set_bg(buffer, x, y, Color::Rgb(r, g, b));
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.set_bg(buffer, x, y, Color::Black),
                // Bright foreground colors
                90 => self.set_fg(buffer, x, y, Color::BrightBlack),
                91 => self.set_fg(buffer, x, y, Color::BrightRed),
                92 => self.set_fg(buffer, x, y, Color::BrightGreen),
                93 => self.set_fg(buffer, x, y, Color::BrightYellow),
                94 => self.set_fg(buffer, x, y, Color::BrightBlue),
                95 => self.set_fg(buffer, x, y, Color::BrightMagenta),
                96 => self.set_fg(buffer, x, y, Color::BrightCyan),
                97 => self.set_fg(buffer, x, y, Color::BrightWhite),
                _ => {}
            }
            i += 1;
        }
    }

    fn set_fg(&self, buffer: &mut Buffer, x: usize, y: usize, color: Color) {
        if let Some(line) = buffer.lines().get(y) {
            if x < line.cells.len() {
                let mut attrs = line.cells[x].attributes.clone();
                attrs.foreground = Some(color);
                buffer.set_cell(
                    x,
                    y,
                    Cell {
                        char: line.cells[x].char,
                        attributes: attrs,
                    },
                );
            }
        }
    }

    fn set_bg(&self, buffer: &mut Buffer, x: usize, y: usize, color: Color) {
        if let Some(line) = buffer.lines().get(y) {
            if x < line.cells.len() {
                let mut attrs = line.cells[x].attributes.clone();
                attrs.background = Some(color);
                buffer.set_cell(
                    x,
                    y,
                    Cell {
                        char: line.cells[x].char,
                        attributes: attrs,
                    },
                );
            }
        }
    }
}
