use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub terminal: TerminalSettings,
    pub ssh: SshSettings,
    pub plugins: PluginSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralSettings {
    pub shell: String,
    pub startup_command: Option<String>,
    pub window_title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppearanceSettings {
    pub theme: String,
    pub font_family: String,
    pub font_size: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub opacity: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalSettings {
    pub scroll_buffer_size: usize,
    pub cursor_style: CursorStyle,
    pub cursor_blink: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub enum CursorStyle {
    Block,
    Bar,
    Underline,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SshSettings {
    pub default_port: u16,
    pub keep_alive_interval: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginSettings {
    pub enabled: bool,
    pub plugin_dirs: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self::default_settings()
    }
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("creeper-term");

        let config_file = config_dir.join("config.toml");

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)?;
            let settings: Settings = toml::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(Self::default_settings())
        }
    }

    fn default_settings() -> Self {
        Self {
            general: GeneralSettings {
                shell: Self::default_shell(),
                startup_command: None,
                window_title: "CreeperTerm".to_string(),
            },
            appearance: AppearanceSettings {
                theme: "default".to_string(),
                font_family: "Fira Code".to_string(),
                font_size: 14,
                window_width: 1200,
                window_height: 800,
                opacity: 1.0,
            },
            terminal: TerminalSettings {
                scroll_buffer_size: 10000,
                cursor_style: CursorStyle::Block,
                cursor_blink: true,
            },
            ssh: SshSettings {
                default_port: 22,
                keep_alive_interval: 60,
            },
            plugins: PluginSettings {
                enabled: true,
                plugin_dirs: vec![dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("creeper-term")
                    .join("plugins")],
            },
        }
    }

    fn default_shell() -> String {
        if cfg!(target_os = "windows") {
            "powershell.exe".to_string()
        } else if cfg!(target_os = "macos") {
            "/bin/zsh".to_string()
        } else {
            "/bin/bash".to_string()
        }
    }
}
