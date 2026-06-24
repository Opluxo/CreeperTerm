use std::sync::mpsc;
use std::time::Duration;

use iced::widget::{column, scrollable, text};
use iced::{Element, Length, Subscription, Task};

use crate::config::Settings;
use crate::terminal::{Buffer, Parser, Pty, PtySize};
use crate::ui::TabBar;

pub struct App {
    #[allow(dead_code)]
    settings: Settings,
    buffer: Buffer,
    parser: Parser,
    input_tx: mpsc::Sender<Vec<u8>>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    tab_bar: TabBar,
    title: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    PollPty,
    KeyPressed(iced::keyboard::Key, Option<smol_str::SmolStr>),
    TabBar(crate::ui::tab_bar::Message),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Settings::load().unwrap_or_default();

        let (input_tx, input_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();

        let cols = 80;
        let rows = 24;
        let buffer = Buffer::new(cols, rows, settings.terminal.scroll_buffer_size);
        let parser = Parser::new();

        let shell = settings.general.shell.clone();
        let size = PtySize {
            rows: rows as u16,
            cols: cols as u16,
        };

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

        let init_task = Task::perform(
            async { tokio::time::sleep(Duration::from_millis(100)).await },
            |_| Message::PollPty,
        );

        (
            Self {
                settings,
                buffer,
                parser,
                input_tx,
                output_rx,
                tab_bar: TabBar::new(),
                title: "CreeperTerm".to_string(),
            },
            init_task,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PollPty => {
                while let Ok(data) = self.output_rx.try_recv() {
                    self.parser.parse(&mut self.buffer, &data);
                }
                Task::perform(
                    async { tokio::time::sleep(Duration::from_millis(16)).await },
                    |_| Message::PollPty,
                )
            }
            Message::KeyPressed(key, key_text) => {
                let input = translate_key(key, key_text);
                if !input.is_empty() {
                    self.input_tx.send(input).ok();
                }
                Task::none()
            }
            Message::TabBar(msg) => {
                self.tab_bar.update(msg);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let lines: Vec<Element<Message>> = self
            .buffer
            .lines()
            .iter()
            .map(|line| {
                let s: String = line.cells.iter().map(|c| c.char).collect();
                text(s).size(14).into()
            })
            .collect();

        let terminal_content = lines
            .into_iter()
            .fold(column![].width(Length::Fill), |acc, elem| acc.push(elem));

        let scroll = scrollable(terminal_content).height(Length::Fill);
        let tab_bar = self.tab_bar.view().map(Message::TabBar);

        column![tab_bar, scroll]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, _status| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key, text, ..
            }) = event
            {
                Some(Message::KeyPressed(key, text))
            } else {
                None
            }
        })
    }
}

fn translate_key(key: iced::keyboard::Key, text: Option<smol_str::SmolStr>) -> Vec<u8> {
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
        _ => {}
    }

    if let Some(text) = text {
        return text.as_bytes().to_vec();
    }

    vec![]
}
