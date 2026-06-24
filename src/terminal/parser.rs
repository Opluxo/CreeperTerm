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
    current_attrs: CellAttributes,
    scroll_top: usize,
    scroll_bottom: usize,
    title: String,
    osc_string: Vec<u8>,
    osc_param: u32,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediate: Vec::new(),
            current_attrs: CellAttributes::default(),
            scroll_top: 0,
            scroll_bottom: 0,
            title: String::new(),
            osc_string: Vec::new(),
            osc_param: 0,
        }
    }

    pub fn current_attrs(&self) -> &CellAttributes {
        &self.current_attrs
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn parse(&mut self, buffer: &mut Buffer, data: &[u8]) {
        if self.scroll_bottom == 0 {
            self.scroll_bottom = buffer.viewport_height();
        }

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
                        if y >= self.scroll_bottom - 1 {
                            buffer.scroll_up_in_region(self.scroll_top, self.scroll_bottom, 1);
                        } else {
                            buffer.move_cursor(x, (y + 1).min(buffer.viewport_height() - 1));
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
                    } else if byte == 0x0d {
                        // CR - ignore (already handled above)
                    } else if byte >= 0x20 {
                        let (x, y) = buffer.cursor_position();
                        let cell = Cell {
                            char: byte as char,
                            attributes: self.current_attrs.clone(),
                        };
                        if x < buffer.viewport_width() {
                            buffer.set_cell(x, y, cell);
                        }
                        if x + 1 >= buffer.viewport_width() {
                            buffer.move_cursor(0, (y + 1).min(buffer.viewport_height() - 1));
                        } else {
                            buffer.move_cursor(x + 1, y);
                        }
                    }
                }
                ParserState::Escape => {
                    if byte == b'[' {
                        self.state = ParserState::Csi;
                    } else if byte == b']' {
                        self.state = ParserState::Osc;
                        self.osc_string.clear();
                        self.osc_param = 0;
                        self.params.clear();
                    } else if byte == b'P' {
                        self.state = ParserState::Dcs;
                    } else if byte == b'M' {
                        // Reverse index
                        let (x, y) = buffer.cursor_position();
                        if y == self.scroll_top {
                            buffer.scroll_down_in_region(self.scroll_top, self.scroll_bottom, 1);
                        } else if y > 0 {
                            buffer.move_cursor(x, y - 1);
                        }
                        self.state = ParserState::Ground;
                    } else if byte == b'7' {
                        buffer.save_cursor();
                        self.state = ParserState::Ground;
                    } else if byte == b'8' {
                        buffer.restore_cursor();
                        self.state = ParserState::Ground;
                    } else if byte == b'D' {
                        // Index - move down, scroll if at bottom of region
                        let (x, y) = buffer.cursor_position();
                        if y >= self.scroll_bottom - 1 {
                            buffer.scroll_up_in_region(self.scroll_top, self.scroll_bottom, 1);
                        } else {
                            buffer.move_cursor(x, (y + 1).min(buffer.viewport_height() - 1));
                        }
                        self.state = ParserState::Ground;
                    } else if byte == b'E' {
                        // Next line
                        let (x, y) = buffer.cursor_position();
                        if y >= self.scroll_bottom - 1 {
                            buffer.scroll_up_in_region(self.scroll_top, self.scroll_bottom, 1);
                        } else {
                            buffer.move_cursor(x, (y + 1).min(buffer.viewport_height() - 1));
                        }
                        buffer.move_cursor(0, buffer.cursor_position().1);
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
                        // OSC complete - process it
                        self.process_osc();
                        self.state = ParserState::Ground;
                    } else if byte == b';' && self.osc_string.is_empty() {
                        // First semicolon separates param from string
                        // Parse the param from accumulated bytes
                        let param_str = String::from_utf8_lossy(&self.osc_param.to_be_bytes().to_vec()).to_string();
                        // Actually parse from the string we've been accumulating
                    } else if byte >= 0x20 {
                        self.osc_string.push(byte);
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

    fn execute_csi(&mut self, buffer: &mut Buffer, final_byte: u8) {
        let params = &self.params;
        let param = |i: usize| -> u32 { params.get(i).copied().unwrap_or(0) };

        // Handle private mode sequences (CSI ? ... h/l)
        if self.intermediate.contains(&b'?') {
            match final_byte {
                b'h' => {
                    // Set mode
                    match param(0) {
                        25 => {
                            // Show cursor - TODO: track cursor visibility
                        }
                        1049 => {
                            // Switch to alternate screen buffer
                            // TODO: implement alternate buffer
                        }
                        _ => {}
                    }
                    return;
                }
                b'l' => {
                    // Reset mode
                    match param(0) {
                        25 => {
                            // Hide cursor - TODO: track cursor visibility
                        }
                        1049 => {
                            // Switch back from alternate screen buffer
                            // TODO: implement alternate buffer
                        }
                        _ => {}
                    }
                    return;
                }
                _ => {}
            }
        }

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
                                        attributes: self.current_attrs.clone(),
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
                                    attributes: self.current_attrs.clone(),
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
                buffer.scroll_up_in_region(self.scroll_top, self.scroll_bottom, n);
            }
            b'T' => {
                let n = param(0).max(1) as usize;
                buffer.scroll_down_in_region(self.scroll_top, self.scroll_bottom, n);
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
            b'm' => self.execute_sgr(params),
            b'r' => {
                // Set scrolling region
                let top = param(0).max(1) as usize;
                let bottom = param(1).unwrap_or(buffer.viewport_height() as u32).max(1) as usize;
                self.scroll_top = (top - 1).min(buffer.viewport_height() - 1);
                self.scroll_bottom = bottom.min(buffer.viewport_height());
                if self.scroll_top >= self.scroll_bottom {
                    self.scroll_top = 0;
                    self.scroll_bottom = buffer.viewport_height();
                }
                buffer.move_cursor(0, self.scroll_top);
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

    fn execute_sgr(&mut self, params: &[u32]) {
        if params.is_empty() {
            self.current_attrs = CellAttributes::default();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.current_attrs = CellAttributes::default();
                }
                1 => self.current_attrs.bold = true,
                2 => self.current_attrs.dim = true,
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                7 => self.current_attrs.reverse = true,
                9 => self.current_attrs.strikethrough = true,
                22 => {
                    self.current_attrs.bold = false;
                    self.current_attrs.dim = false;
                }
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                27 => self.current_attrs.reverse = false,
                29 => self.current_attrs.strikethrough = false,
                // Standard foreground colors
                30 => self.current_attrs.foreground = Some(Color::Black),
                31 => self.current_attrs.foreground = Some(Color::Red),
                32 => self.current_attrs.foreground = Some(Color::Green),
                33 => self.current_attrs.foreground = Some(Color::Yellow),
                34 => self.current_attrs.foreground = Some(Color::Blue),
                35 => self.current_attrs.foreground = Some(Color::Magenta),
                36 => self.current_attrs.foreground = Some(Color::Cyan),
                37 => self.current_attrs.foreground = Some(Color::White),
                38 => {
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 => {
                                if i + 2 < params.len() {
                                    self.current_attrs.foreground =
                                        Some(Color::Indexed(params[i + 2] as u8));
                                    i += 2;
                                }
                            }
                            2 => {
                                if i + 4 < params.len() {
                                    self.current_attrs.foreground = Some(Color::Rgb(
                                        params[i + 2] as u8,
                                        params[i + 3] as u8,
                                        params[i + 4] as u8,
                                    ));
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.current_attrs.foreground = None,
                // Standard background colors
                40 => self.current_attrs.background = Some(Color::Black),
                41 => self.current_attrs.background = Some(Color::Red),
                42 => self.current_attrs.background = Some(Color::Green),
                43 => self.current_attrs.background = Some(Color::Yellow),
                44 => self.current_attrs.background = Some(Color::Blue),
                45 => self.current_attrs.background = Some(Color::Magenta),
                46 => self.current_attrs.background = Some(Color::Cyan),
                47 => self.current_attrs.background = Some(Color::White),
                48 => {
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 => {
                                if i + 2 < params.len() {
                                    self.current_attrs.background =
                                        Some(Color::Indexed(params[i + 2] as u8));
                                    i += 2;
                                }
                            }
                            2 => {
                                if i + 4 < params.len() {
                                    self.current_attrs.background = Some(Color::Rgb(
                                        params[i + 2] as u8,
                                        params[i + 3] as u8,
                                        params[i + 4] as u8,
                                    ));
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.current_attrs.background = None,
                // Bright foreground colors
                90 => self.current_attrs.foreground = Some(Color::BrightBlack),
                91 => self.current_attrs.foreground = Some(Color::BrightRed),
                92 => self.current_attrs.foreground = Some(Color::BrightGreen),
                93 => self.current_attrs.foreground = Some(Color::BrightYellow),
                94 => self.current_attrs.foreground = Some(Color::BrightBlue),
                95 => self.current_attrs.foreground = Some(Color::BrightMagenta),
                96 => self.current_attrs.foreground = Some(Color::BrightCyan),
                97 => self.current_attrs.foreground = Some(Color::BrightWhite),
                // Bright background colors
                100 => self.current_attrs.background = Some(Color::BrightBlack),
                101 => self.current_attrs.background = Some(Color::BrightRed),
                102 => self.current_attrs.background = Some(Color::BrightGreen),
                103 => self.current_attrs.background = Some(Color::BrightYellow),
                104 => self.current_attrs.background = Some(Color::BrightBlue),
                105 => self.current_attrs.background = Some(Color::BrightMagenta),
                106 => self.current_attrs.background = Some(Color::BrightCyan),
                107 => self.current_attrs.background = Some(Color::BrightWhite),
                _ => {}
            }
            i += 1;
        }
    }

    fn process_osc(&mut self) {
        let data = String::from_utf8_lossy(&self.osc_string).to_string();
        // OSC format: "param;string" or just "string"
        if let Some(semi_pos) = data.find(';') {
            let param_str = &data[..semi_pos];
            let value = &data[semi_pos + 1..];
            if let Ok(param) = param_str.parse::<u32>() {
                match param {
                    0 | 1 | 2 => {
                        // Set window title (and icon name for 0/1)
                        self.title = value.to_string();
                        log::debug!("OSC title set: {}", self.title);
                    }
                    4 => {
                        // Set/create color palette entry - TODO
                        log::debug!("OSC color: {}", value);
                    }
                    10 => {
                        // Set foreground color - TODO
                    }
                    11 => {
                        // Set background color - TODO
                    }
                    _ => {
                        log::debug!("OSC {} = {}", param, value);
                    }
                }
            }
        } else {
            // No semicolon - treat as title for param 0
            self.title = data;
            log::debug!("OSC title set: {}", self.title);
        }
    }
}
