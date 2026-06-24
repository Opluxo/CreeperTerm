use std::sync::mpsc;
use std::time::Duration;

use iced::widget::{column, container, horizontal_rule, row, scrollable, text};
use iced::{
    clipboard, keyboard, window, Element, Length, Subscription, Task,
};

use crate::config::Settings;
use crate::terminal::{Buffer, CellAttributes, Parser, Pty, PtySize};
use crate::ui::{TabBar, Theme};

pub struct App {
    settings: Settings,
    terminals: Vec<TerminalTab>,
    active_tab: usize,
    tab_bar: TabBar,
    title: String,
    theme: Theme,
    mouse_position: Option<(f32, f32)>,
    selection: Option<Selection>,
}

struct TerminalTab {
    buffer: Buffer,
    parser: Parser,
    input_tx: mpsc::Sender<Vec<u8>>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    title: String,
    _pty_size: PtySize,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    // Tab management
    TabBar(crate::ui::tab_bar::Message),
    // Terminal I/O
    PollPty(usize),
    // Keyboard
    KeyPressed(keyboard::Key, Option<smol_str::SmolStr>),
    // Clipboard
    Copy,
    Paste(String),
    // Window
    Resized(window::Size),
    // Mouse
    MouseClicked(iced::Point),
    MouseMoved(iced::Point),
    // SSH
    #[allow(dead_code)]
    SshConnect(String, u16, String, Option<String>),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Settings::load().unwrap_or_default();
        let theme = match settings.appearance.theme.as_str() {
            "dracula" => Theme::dracula_theme(),
            _ => Theme::default_theme(),
        };

        let cols = 80;
        let rows = 24;
        let shell = &settings.general.shell;

        let first_tab = Self::create_terminal(shell, cols, rows, &settings);

        let init_task = Task::perform(
            async { tokio::time::sleep(Duration::from_millis(100)).await },
            |_| Message::PollPty(0),
        );

        (
            Self {
                settings,
                terminals: vec![first_tab],
                active_tab: 0,
                tab_bar: TabBar::new(),
                title: settings.general.window_title.clone(),
                theme,
                mouse_position: None,
                selection: None,
            },
            init_task,
        )
    }

    fn create_terminal(shell: &str, cols: usize, rows: usize, settings: &Settings) -> TerminalTab {
        let (input_tx, input_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();

        let buffer = Buffer::new(cols, rows, settings.terminal.scroll_buffer_size);
        let parser = Parser::new();

        let size = PtySize {
            rows: rows as u16,
            cols: cols as u16,
        };

        let shell = shell.to_string();
        std::thread::spawn(move || {
            let mut pty = match Pty::new(&shell, size) {
                Ok(pty) => pty,
                Err(e) => {
                    log::error!("Failed to create PTY: {}", e);
                    return;
                }
            };

            loop {
                while let Ok(data) = input_rx.try_recv() {
                    if let Err(e) = pty.write(&data) {
                        log::error!("Failed to write to PTY: {}", e);
                    }
                }

                let mut buf = [0u8; 4096];
                match pty.try_read(&mut buf) {
                    Ok(Some(n)) => {
                        if output_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Failed to read PTY: {}", e);
                    }
                }

                std::thread::sleep(Duration::from_millis(10));
            }
        });

        TerminalTab {
            buffer,
            parser,
            input_tx,
            output_rx,
            title: "Terminal".to_string(),
            _pty_size: size,
        }
    }

    fn active_terminal(&self) -> Option<&TerminalTab> {
        self.terminals.get(self.active_tab)
    }

    fn active_terminal_mut(&mut self) -> Option<&mut TerminalTab> {
        self.terminals.get_mut(self.active_tab)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PollPty(tab_idx) => {
                if let Some(tab) = self.terminals.get_mut(tab_idx) {
                    while let Ok(data) = tab.output_rx.try_recv() {
                        tab.parser.parse(&mut tab.buffer, &data);
                    }
                }
                let idx = self.active_tab;
                Task::perform(
                    async { tokio::time::sleep(Duration::from_millis(16)).await },
                    move |_| Message::PollPty(idx),
                )
            }
            Message::KeyPressed(key, key_text) => {
                let input = translate_key(key, key_text);
                if let Some(tab) = self.active_terminal_mut() {
                    if !input.is_empty() {
                        tab.input_tx.send(input).ok();
                    }
                }
                Task::none()
            }
            Message::TabBar(msg) => {
                match msg {
                    crate::ui::tab_bar::Message::NewTab => {
                        let cols = 80;
                        let rows = 24;
                        let new_tab = Self::create_terminal(
                            &self.settings.general.shell,
                            cols,
                            rows,
                            &self.settings,
                        );
                        let tab_idx = self.terminals.len();
                        self.terminals.push(new_tab);
                        self.tab_bar.update(msg);
                        self.active_tab = tab_idx;
                        let idx = self.active_tab;
                        return Task::perform(
                            async { tokio::time::sleep(Duration::from_millis(100)).await },
                            move |_| Message::PollPty(idx),
                        );
                    }
                    _ => {
                        if let Some(tab_id) = self.tab_bar.update(msg) {
                            self.active_tab = (tab_id - 1).min(self.terminals.len().saturating_sub(1));
                        }
                    }
                }
                Task::none()
            }
            Message::Copy => {
                if let Some(tab) = self.active_terminal() {
                    let selected_text = self.get_selected_text(&tab.buffer);
                    if !selected_text.is_empty() {
                        return clipboard::write(selected_text);
                    }
                }
                Task::none()
            }
            Message::Paste(text) => {
                if let Some(tab) = self.active_terminal_mut() {
                    tab.input_tx.send(text.into_bytes()).ok();
                }
                Task::none()
            }
            Message::Resized(size) => {
                // Calculate new cols/rows based on window size
                let cell_width = 8.0;
                let cell_height = 16.0;
                let cols = ((size.width as f32) / cell_width).floor() as usize;
                let rows = ((size.height as f32 - 30.0) / cell_height).floor() as usize; // 30px for tab bar
                let cols = cols.max(1);
                let rows = rows.max(1);

                if let Some(tab) = self.active_terminal_mut() {
                    let size = PtySize {
                        rows: rows as u16,
                        cols: cols as u16,
                    };
                    tab.buffer.resize(cols, rows);
                    // Note: PTY resize would need the Pty handle, which is in the thread
                    // For now, just resize the buffer
                    log::debug!("Terminal resized to {}x{}", cols, rows);
                }
                Task::none()
            }
            Message::MouseClicked(point) => {
                // Start text selection
                let cell_width = 8.0;
                let cell_height = 16.0;
                let col = (point.x / cell_width) as usize;
                let row = (point.y / cell_height) as usize;
                self.selection = Some(Selection {
                    start: (col, row),
                    end: (col, row),
                });
                Task::none()
            }
            Message::MouseMoved(point) => {
                if self.selection.is_some() {
                    let cell_width = 8.0;
                    let cell_height = 16.0;
                    let col = (point.x / cell_width) as usize;
                    let row = (point.y / cell_height) as usize;
                    if let Some(sel) = &mut self.selection {
                        sel.end = (col, row);
                    }
                }
                self.mouse_position = Some((point.x, point.y));
                Task::none()
            }
            Message::SshConnect(_host, _port, _user, _password) => {
                // TODO: Implement SSH connection dialog
                log::info!("SSH connect requested");
                Task::none()
            }
        }
    }

    fn get_selected_text(&self, buffer: &Buffer) -> String {
        if let Some(sel) = &self.selection {
            let (start_col, start_row) = sel.start;
            let (end_col, end_row) = sel.end;

            let (min_row, max_row) = if start_row <= end_row {
                (start_row, end_row)
            } else {
                (end_row, start_row)
            };
            let (min_col, max_col) = if start_col <= end_col {
                (start_col, end_col)
            } else {
                (end_col, start_col)
            };

            let mut result = String::new();
            let lines = buffer.lines();

            for row in min_row..=max_row {
                if let Some(line) = lines.get(row) {
                    let start = if row == min_row { min_col } else { 0 };
                    let end = if row == max_row {
                        max_col.min(line.cells.len())
                    } else {
                        line.cells.len()
                    };
                    for cell in line.cells.iter().take(end).skip(start) {
                        result.push(cell.char);
                    }
                    if row < max_row {
                        result.push('\n');
                    }
                }
            }
            result
        } else {
            String::new()
        }
    }

    pub fn view(&self) -> Element<Message> {
        let tab_bar = self.tab_bar.view().map(Message::TabBar);

        let terminal_content: Element<Message> = if let Some(tab) = self.active_terminal() {
            self.render_terminal(tab)
        } else {
            text("No terminal").into()
        };

        let status_bar = self.render_status_bar();

        column![
            tab_bar,
            horizontal_rule(1),
            terminal_content,
            horizontal_rule(1),
            status_bar,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn render_terminal(&self, tab: &TerminalTab) -> Element<Message> {
        let theme = &self.theme;
        let cell_width = 8.0f32;
        let cell_height = 16.0f32;

        let mut lines: Vec<Element<Message>> = Vec::new();

        for (row_idx, line) in tab.buffer.lines().iter().enumerate() {
            let mut line_elements: Vec<Element<Message>> = Vec::new();

            for (col_idx, cell) in line.cells.iter().enumerate() {
                let fg_color = cell
                    .attributes
                    .foreground
                    .map(|c| Theme::color_to_iced(&c))
                    .unwrap_or_else(|| Theme::color_to_iced(&theme.colors.foreground));

                let bg_color = cell
                    .attributes
                    .background
                    .map(|c| Theme::color_to_iced(&c));

                let mut char_str = String::new();
                char_str.push(cell.char);

                let mut t = text(char_str)
                    .size(self.settings.appearance.font_size)
                    .color(fg_color);

                // Note: iced doesn't have built-in bold/italic/underline for text widget
                // These would need custom rendering or styled containers

                line_elements.push(t.into());
            }

            // Join all characters in the line
            let line_element: Element<Message> = line_elements
                .into_iter()
                .fold(row![].into(), |acc: Element<Message>, elem| {
                    row![acc, elem].into()
                });

            lines.push(line_element);
        }

        let content = lines
            .into_iter()
            .fold(column![].width(Length::Fill), |acc, elem| acc.push(elem));

        // Handle selection highlighting
        let content = if let Some(sel) = &self.selection {
            // TODO: Apply selection highlighting
            content
        } else {
            content
        };

        let scroll = scrollable(content)
            .height(Length::Fill)
            .id(scrollable::Id::new(format!("terminal-{}", self.active_tab)));

        // Add click handler for selection
        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn render_status_bar(&self) -> Element<Message> {
        let tab_info = if let Some(tab) = self.active_terminal() {
            format!(
                " {} | {}x{} ",
                tab.title,
                tab.buffer.viewport_width(),
                tab.buffer.viewport_height()
            )
        } else {
            " No terminal ".to_string()
        };

        let title_text = text(&self.title).size(12);
        let info_text = text(&tab_info).size(12);

        row![title_text, iced::widget::horizontal_space(), info_text]
            .width(Length::Fill)
            .padding(2)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let poll = {
            let idx = self.active_tab;
            iced::event::listen_with(move |event, _status| {
                match event {
                    iced::Event::Keyboard(keyboard::Event::KeyPressed {
                        key, text, ..
                    }) => Some(Message::KeyPressed(key, text)),
                    iced::Event::Window(window::Event::Resized(size)) => {
                        Some(Message::Resized(size))
                    }
                    iced::Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    )) => {
                        // Would need cursor position - handled via view
                        None
                    }
                    _ => None,
                }
            })
        };

        poll
    }
}

fn translate_key(key: keyboard::Key, text: Option<smol_str::SmolStr>) -> Vec<u8> {
    use iced::keyboard::{Key, NamedKey};

    match &key {
        Key::Named(NamedKey::Enter) => return vec![b'\r'],
        Key::Named(NamedKey::Backspace) => return vec![0x7f],
        Key::Named(NamedKey::Tab) => return vec![b'\t'],
        Key::Named(NamedKey::Escape) => return vec![0x1b],
        Key::Named(NamedKey::ArrowUp) => return vec![0x1b, b'[', b'A'],
        Key::Named(NamedKey::ArrowDown) => return vec![0x1b, b'[', b'B'],
        Key::Named(NamedKey::ArrowRight) => return vec![0x1b, b'[', b'C'],
        Key::Named(NamedKey::ArrowLeft) => return vec![0x1b, b'[', b'D'],
        Key::Named(NamedKey::Home) => return vec![0x1b, b'[', b'H'],
        Key::Named(NamedKey::End) => return vec![0x1b, b'[', b'F'],
        Key::Named(NamedKey::PageUp) => return vec![0x1b, b'[', b'5', b'~'],
        Key::Named(NamedKey::PageDown) => return vec![0x1b, b'[', b'6', b'~'],
        Key::Named(NamedKey::Delete) => return vec![0x1b, b'[', b'3', b'~'],
        Key::Named(NamedKey::Insert) => return vec![0x1b, b'[', b'2', b'~'],
        Key::Named(NamedKey::F1) => return vec![0x1b, b'[', b'1', b'1', b'~'],
        Key::Named(NamedKey::F2) => return vec![0x1b, b'[', b'1', b'2', b'~'],
        Key::Named(NamedKey::F3) => return vec![0x1b, b'[', b'1', b'3', b'~'],
        Key::Named(NamedKey::F4) => return vec![0x1b, b'[', b'1', b'4', b'~'],
        Key::Named(NamedKey::F5) => return vec![0x1b, b'[', b'1', b'5', b'~'],
        Key::Named(NamedKey::F6) => return vec![0x1b, b'[', b'1', b'7', b'~'],
        Key::Named(NamedKey::F7) => return vec![0x1b, b'[', b'1', b'8', b'~'],
        Key::Named(NamedKey::F8) => return vec![0x1b, b'[', b'1', b'9', b'~'],
        Key::Named(NamedKey::F9) => return vec![0x1b, b'[', b'2', b'0', b'~'],
        Key::Named(NamedKey::F10) => return vec![0x1b, b'[', b'2', b'1', b'~'],
        Key::Named(NamedKey::F11) => return vec![0x1b, b'[', b'2', b'3', b'~'],
        Key::Named(NamedKey::F12) => return vec![0x1b, b'[', b'2', b'4', b'~'],
        Key::Named(NamedKey::PrintScreen) => return vec![0x1b, b'[', b'?', b'2', b'5', b'h'],
        Key::Named(NamedKey::ScrollLock) => return vec![],
        Key::Named(NamedKey::Pause) => return vec![0x1b, b'[', b'P'],
        _ => {}
    }

    // Ctrl+key combinations
    if let Key::Character(ch) = &key {
        let ch_lower = ch.as_str().to_lowercase();
        if let Some(c) = ch_lower.chars().next() {
            // Check if this is a Ctrl combination by looking at the raw key
            // In iced, Ctrl+key sends the control character directly
            // We need to handle the case where text is the control character
        }
    }

    if let Some(text) = text {
        let bytes = text.as_bytes().to_vec();
        // If it's a single printable ASCII character, send it directly
        if bytes.len() == 1 && bytes[0] >= 0x20 && bytes[0] < 0x7f {
            return bytes;
        }
        // For control characters (Ctrl+A = 0x01, etc.)
        if bytes.len() == 1 && bytes[0] < 0x20 {
            return bytes;
        }
    }

    vec![]
}

use iced::mouse;
