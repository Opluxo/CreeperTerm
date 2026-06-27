use std::sync::mpsc;
use std::time::Duration;

use iced::widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text, text_input};
use iced::{keyboard, window, Element, Length, Point, Size, Subscription, Task};

use crate::config::Settings;
use crate::terminal::{Buffer, Parser, Pty, PtySize};
use crate::ui::{TabBar, Theme};

pub struct App {
    settings: Settings,
    terminals: Vec<TerminalTab>,
    active_tab: usize,
    tab_bar: TabBar,
    title: String,
    theme: Theme,
    selection: Option<Selection>,
    context_menu: Option<ContextMenu>,
    ssh_dialog: Option<SshDialog>,
    cursor_blink_state: bool,
    last_mouse_pos: Point,
}

struct TerminalTab {
    buffer: Buffer,
    parser: Parser,
    input_tx: mpsc::Sender<Vec<u8>>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    title: String,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

impl Selection {
    pub fn text_from(&self, buffer: &Buffer) -> String {
        let (start_col, start_row) = self.start;
        let (end_col, end_row) = self.end;

        let (row_min, row_max, col_min, col_max) = if start_row < end_row || (start_row == end_row && start_col <= end_col)
        {
            (start_row, end_row, start_col, end_col)
        } else {
            (end_row, start_row, end_col, start_col)
        };

        let mut result = String::new();
        for row in row_min..=row_max {
            if let Some(line) = buffer.lines().get(row) {
                let start = if row == row_min { col_min } else { 0 };
                let end = if row == row_max {
                    (col_max + 1).min(line.cells.len())
                } else {
                    line.cells.len()
                };
                for cell in &line.cells[start..end] {
                    result.push(cell.char);
                }
                if row < row_max {
                    result.push('\n');
                }
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub position: Point,
    pub items: Vec<ContextMenuItem>,
}

#[derive(Debug, Clone)]
pub enum ContextMenuItem {
    Copy,
    Paste,
    SelectAll,
    Clear,
}

#[derive(Debug, Clone)]
pub struct SshDialog {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabBar(crate::ui::tab_bar::Message),
    PollPty(usize),
    KeyPressed(keyboard::Key, Option<smol_str::SmolStr>),
    Copy,
    Paste(String),
    PasteFromClipboard,
    CopySelection,
    SelectAll,
    Clear,
    Resized(Size),
    MouseClicked(Point),
    MouseRightClicked(Point),
    MouseMoved(Point),
    MouseReleased(Point),
    ContextMenuAction(ContextMenuItem),
    CloseContextMenu,
    ShowSshDialog,
    CloseSshDialog,
    SshDialogInput(SshDialogField, String),
    SshConnect,
    SshConnected(Result<(), String>),
    CursorBlink,
}

#[derive(Debug, Clone)]
pub enum SshDialogField {
    Host,
    Port,
    Username,
    Password,
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
        let shell = settings.general.shell.clone();

        let first_tab = Self::create_terminal(&shell, cols, rows, &settings);

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
                title: "CreeperTerm".to_string(),
                theme,
                selection: None,
                context_menu: None,
                ssh_dialog: None,
                cursor_blink_state: true,
                last_mouse_pos: Point::ORIGIN,
            },
            init_task,
        )
    }

    fn create_terminal(shell: &str, cols: usize, rows: usize, settings: &Settings) -> TerminalTab {
        let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>();
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();

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
        }
    }

    #[allow(dead_code)]
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
                if self.ssh_dialog.is_some() {
                    return Task::none();
                }
                self.context_menu = None;
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
                            self.active_tab =
                                (tab_id - 1).min(self.terminals.len().saturating_sub(1));
                            self.selection = None;
                        }
                    }
                }
                Task::none()
            }
            Message::CopySelection => {
                if let (Some(sel), Some(tab)) =
                    (&self.selection, self.active_terminal())
                {
                    let text = sel.text_from(&tab.buffer);
                    if !text.is_empty() {
                        return iced::clipboard::write(text);
                    }
                }
                Task::none()
            }
            Message::PasteFromClipboard => {
                iced::clipboard::read().map(|opt| Message::Paste(opt.unwrap_or_default()))
            }
            Message::Copy => {
                if let (Some(sel), Some(tab)) =
                    (&self.selection, self.active_terminal())
                {
                    let text = sel.text_from(&tab.buffer);
                    if !text.is_empty() {
                        return iced::clipboard::write(text);
                    }
                }
                Task::none()
            }
            Message::Paste(text) => {
                if let Some(tab) = self.active_terminal_mut() {
                    let data = text.into_bytes();
                    tab.input_tx.send(data).ok();
                }
                Task::none()
            }
            Message::SelectAll => {
                if let Some(tab) = self.active_terminal() {
                    let h = tab.buffer.viewport_height();
                    let w = tab.buffer.viewport_width();
                    self.selection = Some(Selection {
                        start: (0, 0),
                        end: (w.saturating_sub(1), h.saturating_sub(1)),
                    });
                }
                Task::none()
            }
            Message::Clear => {
                if let Some(tab) = self.active_terminal_mut() {
                    tab.buffer.clear();
                }
                self.selection = None;
                Task::none()
            }
            Message::Resized(size) => {
                let cell_width = 8.0;
                let cell_height = 16.0;
                let cols = ((size.width) / cell_width).floor() as usize;
                let rows = ((size.height - 30.0) / cell_height).floor() as usize;
                let cols = cols.max(1);
                let rows = rows.max(1);

                if let Some(tab) = self.active_terminal_mut() {
                    tab.buffer.resize(cols, rows);
                    log::debug!("Terminal resized to {}x{}", cols, rows);
                }
                Task::none()
            }
            Message::MouseClicked(_point) => {
                self.context_menu = None;
                let point = self.last_mouse_pos;
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
            Message::MouseRightClicked(_point) => {
                let point = self.last_mouse_pos;
                self.context_menu = Some(ContextMenu {
                    position: point,
                    items: vec![
                        ContextMenuItem::Copy,
                        ContextMenuItem::Paste,
                        ContextMenuItem::SelectAll,
                        ContextMenuItem::Clear,
                    ],
                });
                Task::none()
            }
            Message::MouseMoved(point) => {
                self.last_mouse_pos = point;
                if self.selection.is_some() {
                    let cell_width = 8.0;
                    let cell_height = 16.0;
                    let col = (point.x / cell_width) as usize;
                    let row = (point.y / cell_height) as usize;
                    if let Some(sel) = &mut self.selection {
                        sel.end = (col, row);
                    }
                }
                Task::none()
            }
            Message::MouseReleased(_point) => {
                if let Some(sel) = &self.selection {
                    if sel.start == sel.end {
                        self.selection = None;
                    }
                }
                Task::none()
            }
            Message::ContextMenuAction(action) => {
                self.context_menu = None;
                match action {
                    ContextMenuItem::Copy => return self.update(Message::CopySelection),
                    ContextMenuItem::Paste => return self.update(Message::PasteFromClipboard),
                    ContextMenuItem::SelectAll => return self.update(Message::SelectAll),
                    ContextMenuItem::Clear => return self.update(Message::Clear),
                }
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::ShowSshDialog => {
                self.ssh_dialog = Some(SshDialog {
                    host: String::new(),
                    port: "22".to_string(),
                    username: String::new(),
                    password: String::new(),
                    error: None,
                });
                Task::none()
            }
            Message::CloseSshDialog => {
                self.ssh_dialog = None;
                Task::none()
            }
            Message::SshDialogInput(field, value) => {
                if let Some(dialog) = &mut self.ssh_dialog {
                    match field {
                        SshDialogField::Host => dialog.host = value,
                        SshDialogField::Port => dialog.port = value,
                        SshDialogField::Username => dialog.username = value,
                        SshDialogField::Password => dialog.password = value,
                    }
                }
                Task::none()
            }
            Message::SshConnect => {
                if let Some(dialog) = &self.ssh_dialog {
                    let config = crate::ssh::SshConfig {
                        host: dialog.host.clone(),
                        port: dialog.port.parse().unwrap_or(22),
                        username: dialog.username.clone(),
                        password: Some(dialog.password.clone()),
                        key_path: None,
                    };
                    return Task::perform(
                        async move {
                            let mut client = crate::ssh::SshClient::new(config.clone());
                            match client.connect(&config) {
                                Ok(()) => {
                                    let _ = client.execute("bash");
                                    Ok(())
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        |result| match result {
                            Ok(()) => Message::CloseSshDialog,
                            Err(e) => Message::SshConnected(Err(e)),
                        },
                    );
                }
                Task::none()
            }
            Message::SshConnected(result) => {
                if let Some(dialog) = &mut self.ssh_dialog {
                    match result {
                        Ok(()) => {
                            self.ssh_dialog = None;
                        }
                        Err(e) => {
                            dialog.error = Some(e);
                        }
                    }
                }
                Task::none()
            }
            Message::CursorBlink => {
                self.cursor_blink_state = !self.cursor_blink_state;
                Task::none()
            }
        }
    }

    fn render_terminal(&self, tab: &TerminalTab) -> Element<'_, Message> {
        let theme = &self.theme;
        let font_size = self.settings.appearance.font_size as f32;

        let mut lines: Vec<Element<Message>> = Vec::new();

        for line in tab.buffer.lines().iter() {
            let mut line_elements: Vec<Element<Message>> = Vec::new();

            for cell in line.cells.iter() {
                let fg_color = cell
                    .attributes
                    .foreground
                    .map(|c| Theme::color_to_iced(&c))
                    .unwrap_or_else(|| Theme::color_to_iced(&theme.colors.foreground));

                let mut char_str = String::new();
                char_str.push(cell.char);

                let t = text(char_str).size(font_size).color(fg_color);

                line_elements.push(t.into());
            }

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

        let scroll = scrollable(content)
            .height(Length::Fill)
            .id(scrollable::Id::new(format!("terminal-{}", self.active_tab)));

        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn render_status_bar(&self) -> Element<'_, Message> {
        let (cols, rows, tab_title) = if let Some(tab) = self.active_terminal() {
            (
                tab.buffer.viewport_width(),
                tab.buffer.viewport_height(),
                tab.title.clone(),
            )
        } else {
            (0, 0, "No terminal".to_string())
        };

        let title_text = text(self.title.clone()).size(12);
        let info_text = text(format!(" {} | {}x{} ", tab_title, cols, rows)).size(12);

        let ssh_button = button(text("SSH").size(12))
            .on_press(Message::ShowSshDialog)
            .padding(2);

        row![title_text, horizontal_space(), ssh_button, info_text]
            .width(Length::Fill)
            .padding(2)
            .into()
    }

    fn render_context_menu(&self) -> Option<Element<'_, Message>> {
        self.context_menu.as_ref().map(|menu| {
            let mut items_col = column![].padding(4).spacing(2);
            for item in &menu.items {
                let (label, msg) = match item {
                    ContextMenuItem::Copy => ("Copy", Message::ContextMenuAction(ContextMenuItem::Copy)),
                    ContextMenuItem::Paste => ("Paste", Message::ContextMenuAction(ContextMenuItem::Paste)),
                    ContextMenuItem::SelectAll => ("Select All", Message::ContextMenuAction(ContextMenuItem::SelectAll)),
                    ContextMenuItem::Clear => ("Clear", Message::ContextMenuAction(ContextMenuItem::Clear)),
                };
                let btn: Element<Message> = button(text(label).size(14))
                    .on_press(msg)
                    .width(Length::Fill)
                    .padding([4, 12])
                    .into();
                items_col = items_col.push(btn);
            }

            let menu_width = 150.0;
            let menu_height = 200.0;
            let _ = (menu_width, menu_height);

            container(items_col)
                .width(Length::Fixed(menu_width))
                .padding(4)
                .into()
        })
    }

    fn render_ssh_dialog(&self) -> Option<Element<'_, Message>> {
        self.ssh_dialog.as_ref().map(|dialog| {
            let title = text("SSH Connection").size(18);

            let host_input = row![
                text("Host:").width(Length::Fixed(80.0)),
                text_input("hostname or IP", &dialog.host)
                    .on_input(|s| Message::SshDialogInput(SshDialogField::Host, s))
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            let port_input = row![
                text("Port:").width(Length::Fixed(80.0)),
                text_input("22", &dialog.port)
                    .on_input(|s| Message::SshDialogInput(SshDialogField::Port, s))
                    .width(Length::Fixed(80.0)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            let user_input = row![
                text("User:").width(Length::Fixed(80.0)),
                text_input("username", &dialog.username)
                    .on_input(|s| Message::SshDialogInput(SshDialogField::Username, s))
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            let pass_input = row![
                text("Pass:").width(Length::Fixed(80.0)),
                text_input("password", &dialog.password)
                    .on_input(|s| Message::SshDialogInput(SshDialogField::Password, s))
                    .secure(true)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            let error_text = dialog
                .error
                .as_ref()
                .map(|e| text(format!("Error: {}", e)).color(iced::Color::from_rgb(1.0, 0.3, 0.3)))
                .unwrap_or_else(|| text(""));

            let buttons = row![
                button(text("Connect").size(14))
                    .on_press(Message::SshConnect)
                    .padding([6, 16]),
                button(text("Cancel").size(14))
                    .on_press(Message::CloseSshDialog)
                    .padding([6, 16]),
            ]
            .spacing(8);

            let form = column![
                title,
                host_input,
                port_input,
                user_input,
                pass_input,
                error_text,
                buttons,
            ]
            .spacing(12)
            .padding(20)
            .max_width(400);

            container(form)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tab_bar = self.tab_bar.view().map(Message::TabBar);

        let terminal_content: Element<Message> =
            if let Some(tab) = self.terminals.get(self.active_tab) {
                self.render_terminal(tab)
            } else {
                text("No terminal").into()
            };

        let status_bar = self.render_status_bar();

        let main = column![
            tab_bar,
            horizontal_rule(1),
            terminal_content,
            horizontal_rule(1),
            status_bar,
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        let mut content: Element<Message> = main.into();

        if let Some(menu) = self.render_context_menu() {
            content = container(column![content, menu]).into();
        }

        if let Some(dialog) = self.render_ssh_dialog() {
            let overlay = container(dialog)
                .center_x(Length::Fill)
                .center_y(Length::Fill);
            content = container(column![content, overlay])
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        content
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let event_sub = iced::event::listen_with(|event, _status, _window_id| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key, text, ..
            }) => Some(Message::KeyPressed(key, text)),
            iced::Event::Window(window::Event::Resized(size)) => {
                Some(Message::Resized(size))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)) => {
                match button {
                    iced::mouse::Button::Left => Some(Message::MouseClicked(Point::ORIGIN)),
                    iced::mouse::Button::Right => Some(Message::MouseRightClicked(Point::ORIGIN)),
                    _ => None,
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::MouseMoved(position))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)) => {
                match button {
                    iced::mouse::Button::Left => Some(Message::MouseReleased(Point::ORIGIN)),
                    _ => None,
                }
            }
            _ => None,
        });

        let blink_sub = iced::time::every(Duration::from_millis(500)).map(|_| Message::CursorBlink);

        Subscription::batch(vec![event_sub, blink_sub])
    }
}

fn translate_key(key: keyboard::Key, text: Option<smol_str::SmolStr>) -> Vec<u8> {
    use iced::keyboard::key::Named;

    match &key {
        keyboard::Key::Named(Named::Enter) => return vec![b'\r'],
        keyboard::Key::Named(Named::Backspace) => return vec![0x7f],
        keyboard::Key::Named(Named::Tab) => return vec![b'\t'],
        keyboard::Key::Named(Named::Escape) => return vec![0x1b],
        keyboard::Key::Named(Named::ArrowUp) => return vec![0x1b, b'[', b'A'],
        keyboard::Key::Named(Named::ArrowDown) => return vec![0x1b, b'[', b'B'],
        keyboard::Key::Named(Named::ArrowRight) => return vec![0x1b, b'[', b'C'],
        keyboard::Key::Named(Named::ArrowLeft) => return vec![0x1b, b'[', b'D'],
        keyboard::Key::Named(Named::Home) => return vec![0x1b, b'[', b'H'],
        keyboard::Key::Named(Named::End) => return vec![0x1b, b'[', b'F'],
        keyboard::Key::Named(Named::PageUp) => return vec![0x1b, b'[', b'5', b'~'],
        keyboard::Key::Named(Named::PageDown) => return vec![0x1b, b'[', b'6', b'~'],
        keyboard::Key::Named(Named::Delete) => return vec![0x1b, b'[', b'3', b'~'],
        keyboard::Key::Named(Named::Insert) => return vec![0x1b, b'[', b'2', b'~'],
        keyboard::Key::Named(Named::F1) => return vec![0x1b, b'[', b'1', b'1', b'~'],
        keyboard::Key::Named(Named::F2) => return vec![0x1b, b'[', b'1', b'2', b'~'],
        keyboard::Key::Named(Named::F3) => return vec![0x1b, b'[', b'1', b'3', b'~'],
        keyboard::Key::Named(Named::F4) => return vec![0x1b, b'[', b'1', b'4', b'~'],
        keyboard::Key::Named(Named::F5) => return vec![0x1b, b'[', b'1', b'5', b'~'],
        keyboard::Key::Named(Named::F6) => return vec![0x1b, b'[', b'1', b'7', b'~'],
        keyboard::Key::Named(Named::F7) => return vec![0x1b, b'[', b'1', b'8', b'~'],
        keyboard::Key::Named(Named::F8) => return vec![0x1b, b'[', b'1', b'9', b'~'],
        keyboard::Key::Named(Named::F9) => return vec![0x1b, b'[', b'2', b'0', b'~'],
        keyboard::Key::Named(Named::F10) => return vec![0x1b, b'[', b'2', b'1', b'~'],
        keyboard::Key::Named(Named::F11) => return vec![0x1b, b'[', b'2', b'3', b'~'],
        keyboard::Key::Named(Named::F12) => return vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => {}
    }

    if let Some(text) = text {
        let bytes = text.as_bytes().to_vec();
        if bytes.len() == 1 && bytes[0] >= 0x20 && bytes[0] < 0x7f {
            return bytes;
        }
        if bytes.len() == 1 && bytes[0] < 0x20 {
            return bytes;
        }
    }

    vec![]
}
