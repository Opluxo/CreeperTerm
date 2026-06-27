use std::io::{Read, Write};

use portable_pty::Child;

pub struct Pty {
    child: Box<dyn Child + Send>,
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    pty_size: portable_pty::PtySize,
}

#[derive(Debug, Clone)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

impl Pty {
    pub fn new(shell: &str, size: PtySize) -> anyhow::Result<Self> {
        let pty_size = portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = portable_pty::native_pty_system().openpty(pty_size)?;

        let mut cmd = portable_pty::CommandBuilder::new(shell);
        cmd.cwd(std::env::current_dir()?);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("SHELL", shell);

        let child = pair.slave.spawn_command(cmd)?;
        let mut master = pair.master;
        let writer = master.take_writer()?;
        let reader = master.try_clone_reader()?;

        Ok(Self {
            child,
            writer,
            reader,
            master: Some(master),
            pty_size,
        })
    }

    pub fn resize(&mut self, size: PtySize) -> anyhow::Result<()> {
        let pty_size = portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        if let Some(master) = &self.master {
            master.resize(pty_size)?;
        }
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn try_read(&mut self, buffer: &mut [u8]) -> anyhow::Result<Option<usize>> {
        use std::io::ErrorKind;
        match self.reader.read(buffer) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(n)),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(ref e) if e.kind() == ErrorKind::TimedOut => Ok(None),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => Ok(None),
            Err(ref e) if e.kind() == ErrorKind::BrokenPipe => {
                log::warn!("PTY pipe broken");
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}
