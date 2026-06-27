use crate::terminal::buffer::Buffer;
use crate::terminal::parser::Parser;
use crate::terminal::pty::{Pty, PtySize};

pub struct TerminalState {
    pub buffer: Buffer,
    pub parser: Parser,
    pub pty: Option<Pty>,
    pub size: PtySize,
    pub title: String,
}

impl TerminalState {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            buffer: Buffer::new(width, height, 10000),
            parser: Parser::new(),
            pty: None,
            size: PtySize {
                rows: height as u16,
                cols: width as u16,
            },
            title: "CreeperTerm".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn set_pty(&mut self, pty: Pty) {
        self.pty = Some(pty);
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.size = PtySize {
            rows: height as u16,
            cols: width as u16,
        };
        self.buffer.resize(width, height);

        if let Some(pty) = &mut self.pty {
            if let Err(e) = pty.resize(self.size.clone()) {
                log::error!("Failed to resize PTY: {}", e);
            }
        }
    }

    #[allow(dead_code)]
    pub fn process_input(&mut self, data: &[u8]) {
        if let Some(pty) = &mut self.pty {
            if let Err(e) = pty.write(data) {
                log::error!("Failed to write to PTY: {}", e);
            }
        }
    }

    #[allow(dead_code)]
    pub fn process_output(&mut self) {
        if let Some(pty) = &mut self.pty {
            let mut buffer = [0u8; 4096];
            match pty.try_read(&mut buffer) {
                Ok(Some(n)) => {
                    self.parser.parse(&mut self.buffer, &buffer[..n]);
                }
                Ok(None) => {}
                Err(e) => {
                    log::error!("Failed to read from PTY: {}", e);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_alive(&self) -> bool {
        self.pty.is_some()
    }

    #[allow(dead_code)]
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
}
