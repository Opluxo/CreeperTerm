use std::io::Read;
use std::net::TcpStream;

use ssh2::Session;

pub struct SshClient {
    session: Option<Session>,
    #[allow(dead_code)]
    host: String,
    #[allow(dead_code)]
    port: u16,
    #[allow(dead_code)]
    username: String,
}

#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
}

impl SshClient {
    pub fn new(config: SshConfig) -> Self {
        Self {
            session: None,
            host: config.host,
            port: config.port,
            username: config.username,
        }
    }

    pub fn connect(&mut self, config: &SshConfig) -> anyhow::Result<()> {
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        if let Some(password) = &config.password {
            session.userauth_password(&config.username, password)?;
        } else if let Some(key_path) = &config.key_path {
            session.userauth_pubkey_file(
                &config.username,
                None,
                std::path::Path::new(key_path),
                None,
            )?;
        } else {
            anyhow::bail!("No authentication method provided (password or key required)");
        }

        if !session.authenticated() {
            anyhow::bail!("Authentication failed");
        }

        self.session = Some(session);
        Ok(())
    }

    pub fn execute(&mut self, command: &str) -> anyhow::Result<String> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        let mut channel = session.channel_session()?;
        channel.exec(command)?;

        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        Ok(output)
    }

    pub fn is_connected(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.authenticated())
            .unwrap_or(false)
    }

    pub fn disconnect(&mut self) {
        self.session = None;
    }
}

pub struct SshManager {
    clients: Vec<SshClient>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    pub fn add_client(&mut self, client: SshClient) {
        self.clients.push(client);
    }

    pub fn remove_client(&mut self, index: usize) {
        if index < self.clients.len() {
            self.clients.remove(index);
        }
    }

    pub fn list_clients(&self) -> &[SshClient] {
        &self.clients
    }
}
