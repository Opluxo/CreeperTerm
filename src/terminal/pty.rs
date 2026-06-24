use std::io::{Read, Write};

pub struct Pty {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
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

        let child = pair.slave.spawn_command(cmd)?;
        let mut master = pair.master;
        let writer = master.take_writer()?;
        let reader = master.try_clone_reader()?;

        Ok(Self {
            child,
            writer,
            reader,
            pty_size,
        })
    }

    pub fn resize(&mut self, size: PtySize) -> anyhow::Result<()> {
        self.pty_size = portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        self.child.resize(self.pty_size)?;
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read(&mut self, buffer: &mut [u8]) -> anyhow::Result<usize> {
        Ok(self.reader.read(buffer)?)
    }

    pub fn try_read(&mut self, buffer: &mut [u8]) -> anyhow::Result<Option<usize>> {
        use std::io::ErrorKind;
        match self.reader.read(buffer) {
            Ok(n) => Ok(Some(n)),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(ref e) if e.kind() == ErrorKind::TimedOut => Ok(None),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    pub fn is_alive(&self) -> bool {
        self.child
            .exit_status()
            .map(|s| !s.success())
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn exit_status(&self) -> Option<i32> {
        self.child.exit_status().map(|s| s.code().unwrap_or(-1))
    }
}
