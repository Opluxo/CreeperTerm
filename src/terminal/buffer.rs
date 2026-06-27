use std::collections::VecDeque;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Cell {
    pub char: char,
    pub attributes: CellAttributes,
}

#[derive(Debug, Clone, Default)]
pub struct CellAttributes {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub reverse: bool,
    pub dim: bool,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_color(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid color: {}", s)))
    }
}

pub fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "bright-black" | "brightblack" | "gray" | "grey" => Some(Color::BrightBlack),
        "bright-red" | "brightred" => Some(Color::BrightRed),
        "bright-green" | "brightgreen" => Some(Color::BrightGreen),
        "bright-yellow" | "brightyellow" => Some(Color::BrightYellow),
        "bright-blue" | "brightblue" => Some(Color::BrightBlue),
        "bright-magenta" | "brightmagenta" => Some(Color::BrightMagenta),
        "bright-cyan" | "brightcyan" => Some(Color::BrightCyan),
        "bright-white" | "brightwhite" => Some(Color::BrightWhite),
        s if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).ok()?;
            let g = u8::from_str_radix(&s[3..5], 16).ok()?;
            let b = u8::from_str_radix(&s[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub cells: Vec<Cell>,
    pub wrapped: bool,
}

#[derive(Debug)]
pub struct Buffer {
    lines: VecDeque<Line>,
    alternate_lines: Option<VecDeque<Line>>,
    alternate_cursor: Option<(usize, usize)>,
    viewport_height: usize,
    viewport_width: usize,
    #[allow(dead_code)]
    scrollback_size: usize,
    cursor_x: usize,
    cursor_y: usize,
    saved_cursor: Option<(usize, usize)>,
}

impl Buffer {
    pub fn new(width: usize, height: usize, scrollback_size: usize) -> Self {
        let mut lines = VecDeque::with_capacity(height + scrollback_size);
        for _ in 0..height {
            lines.push_back(Line::new(width));
        }

        Self {
            lines,
            alternate_lines: None,
            alternate_cursor: None,
            viewport_height: height,
            viewport_width: width,
            scrollback_size,
            cursor_x: 0,
            cursor_y: 0,
            saved_cursor: None,
        }
    }

    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        self.viewport_width = new_width;
        self.viewport_height = new_height;

        while self.lines.len() < new_height {
            self.lines.push_back(Line::new(new_width));
        }

        for line in &mut self.lines {
            line.resize(new_width);
        }
    }

    pub fn set_cell(&mut self, x: usize, y: usize, cell: Cell) {
        if let Some(line) = self.lines.get_mut(y) {
            if x < line.cells.len() {
                line.cells[x] = cell;
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_cell(&self, x: usize, y: usize) -> Option<&Cell> {
        self.lines.get(y).and_then(|line| line.cells.get(x))
    }

    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    pub fn clear_line(&mut self, y: usize) {
        if let Some(line) = self.lines.get_mut(y) {
            line.clear();
        }
    }

    pub fn clear_from_cursor_to_end(&mut self) {
        let (x, y) = (self.cursor_x, self.cursor_y);
        if let Some(line) = self.lines.get_mut(y) {
            for cell in line.cells.iter_mut().skip(x) {
                cell.char = ' ';
                cell.attributes = CellAttributes::default();
            }
        }
        for i in (y + 1)..self.viewport_height {
            self.clear_line(i);
        }
    }

    pub fn clear_from_start_to_cursor(&mut self) {
        let (x, y) = (self.cursor_x, self.cursor_y);
        for i in 0..y {
            self.clear_line(i);
        }
        if let Some(line) = self.lines.get_mut(y) {
            for cell in line.cells.iter_mut().take(x + 1) {
                cell.char = ' ';
                cell.attributes = CellAttributes::default();
            }
        }
    }

    pub fn scroll_up(&mut self, count: usize) {
        self.scroll_up_in_region(0, self.viewport_height, count);
    }

    pub fn scroll_down(&mut self, count: usize) {
        self.scroll_down_in_region(0, self.viewport_height, count);
    }

    pub fn scroll_up_in_region(&mut self, top: usize, bottom: usize, count: usize) {
        let count = count.min(bottom.saturating_sub(top));
        for _ in 0..count {
            if top < self.lines.len() {
                self.lines.remove(top);
            }
            let new_line = Line::new(self.viewport_width);
            if bottom <= self.lines.len() {
                self.lines.insert(bottom, new_line);
            } else {
                self.lines.push_back(new_line);
            }
        }
    }

    pub fn scroll_down_in_region(&mut self, top: usize, bottom: usize, count: usize) {
        let count = count.min(bottom.saturating_sub(top));
        for _ in 0..count {
            if bottom > 0 && bottom <= self.lines.len() {
                self.lines.remove(bottom - 1);
            }
            let new_line = Line::new(self.viewport_width);
            if top < self.lines.len() {
                self.lines.insert(top, new_line);
            } else {
                self.lines.push_front(new_line);
            }
        }
    }

    pub fn insert_lines(&mut self, at: usize, count: usize) {
        for _ in 0..count {
            self.lines
                .insert(at, Line::new(self.viewport_width));
        }
        while self.lines.len() > self.viewport_height + self.scrollback_size {
            self.lines.pop_back();
        }
    }

    pub fn delete_lines(&mut self, at: usize, count: usize) {
        for _ in 0..count {
            if at < self.lines.len() {
                self.lines.remove(at);
            }
        }
    }

    pub fn delete_chars(&mut self, x: usize, y: usize, count: usize) {
        if let Some(line) = self.lines.get_mut(y) {
            let end = (x + count).min(line.cells.len());
            line.cells.drain(x..end);
            while line.cells.len() < self.viewport_width {
                line.cells.push(Cell {
                    char: ' ',
                    attributes: CellAttributes::default(),
                });
            }
        }
    }

    pub fn erase_chars(&mut self, x: usize, y: usize, count: usize) {
        if let Some(line) = self.lines.get(y) {
            let end = (x + count).min(line.cells.len());
            for cx in x..end {
                self.set_cell(
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

    pub fn move_cursor(&mut self, x: usize, y: usize) {
        self.cursor_x = x.min(self.viewport_width.saturating_sub(1));
        self.cursor_y = y.min(self.viewport_height.saturating_sub(1));
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_x, self.cursor_y));
    }

    pub fn restore_cursor(&mut self) {
        if let Some((x, y)) = self.saved_cursor {
            self.cursor_x = x;
            self.cursor_y = y;
            self.saved_cursor = None;
        }
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn switch_to_alternate(&mut self) {
        if self.alternate_lines.is_none() {
            self.alternate_lines = Some(std::mem::replace(
                &mut self.lines,
                VecDeque::with_capacity(self.viewport_height),
            ));
            self.alternate_cursor = Some((self.cursor_x, self.cursor_y));
            for _ in 0..self.viewport_height {
                self.lines.push_back(Line::new(self.viewport_width));
            }
            self.cursor_x = 0;
            self.cursor_y = 0;
        }
    }

    pub fn switch_to_main(&mut self) {
        if let Some(main_lines) = self.alternate_lines.take() {
            self.lines = main_lines;
            if let Some((x, y)) = self.alternate_cursor.take() {
                self.cursor_x = x;
                self.cursor_y = y;
            }
        }
    }

    pub fn lines(&self) -> &VecDeque<Line> {
        &self.lines
    }

    pub fn viewport_height(&self) -> usize {
        self.viewport_height
    }

    pub fn viewport_width(&self) -> usize {
        self.viewport_width
    }
}

impl Line {
    pub fn new(width: usize) -> Self {
        Self {
            cells: vec![
                Cell {
                    char: ' ',
                    attributes: CellAttributes::default(),
                };
                width
            ],
            wrapped: false,
        }
    }

    pub fn resize(&mut self, new_width: usize) {
        self.cells.resize(
            new_width,
            Cell {
                char: ' ',
                attributes: CellAttributes::default(),
            },
        );
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.char = ' ';
            cell.attributes = CellAttributes::default();
        }
        self.wrapped = false;
    }
}
