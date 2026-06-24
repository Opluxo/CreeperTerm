use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;
use tar::Archive;

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    plugin: ManifestPlugin,
}

#[derive(Debug, Deserialize)]
struct ManifestPlugin {
    name: String,
    version: String,
    description: String,
    author: String,
}

pub struct PluginManager {
    plugins: Vec<Plugin>,
    plugin_dirs: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            plugin_dirs: vec![dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("creeper-term")
                .join("plugins")],
        }
    }

    pub fn add_plugin_dir(&mut self, dir: PathBuf) {
        if !self.plugin_dirs.contains(&dir) {
            self.plugin_dirs.push(dir);
        }
    }

    pub fn scan_plugins(&mut self) -> anyhow::Result<()> {
        self.plugins.clear();

        for dir in &self.plugin_dirs {
            if dir.exists() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.extension().and_then(|s| s.to_str()) == Some("ctp") {
                        if let Ok(plugin) = self.load_plugin_info(&path) {
                            self.plugins.push(plugin);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn load_plugin_info(&self, path: &Path) -> anyhow::Result<Plugin> {
        let file = File::open(path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?.to_path_buf();

            if entry_path == Path::new("manifest.toml") {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;

                let manifest: Manifest = toml::from_str(&content)?;

                return Ok(Plugin {
                    name: manifest.plugin.name,
                    version: manifest.plugin.version,
                    description: manifest.plugin.description,
                    author: manifest.plugin.author,
                    path: path.to_path_buf(),
                });
            }
        }

        anyhow::bail!("No manifest.toml found in plugin package")
    }

    pub fn list_plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn get_plugin(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.name == name)
    }

    pub fn install_plugin(&mut self, ctp_path: &Path) -> anyhow::Result<()> {
        let plugin_dir = self
            .plugin_dirs
            .first()
            .ok_or_else(|| anyhow::anyhow!("No plugin directory configured"))?;

        std::fs::create_dir_all(plugin_dir)?;

        let filename = ctp_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid plugin path"))?;

        let dest = plugin_dir.join(filename);
        std::fs::copy(ctp_path, &dest)?;

        self.scan_plugins()?;

        Ok(())
    }

    pub fn uninstall_plugin(&mut self, name: &str) -> anyhow::Result<()> {
        if let Some(plugin) = self.get_plugin(name) {
            let path = plugin.path.clone();
            std::fs::remove_file(&path)?;
            self.scan_plugins()?;
        }

        Ok(())
    }

    pub fn create_plugin_package(
        &self,
        name: &str,
        version: &str,
        description: &str,
        author: &str,
        source_dir: &Path,
        output_path: &Path,
    ) -> anyhow::Result<()> {
        let manifest = format!(
            r#"[plugin]
name = "{}"
version = "{}"
description = "{}"
author = "{}"
"#,
            name, version, description, author
        );

        let file = File::create(output_path)?;
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar = tar::Builder::new(enc);

        let manifest_bytes = manifest.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.toml")?;
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_bytes)?;

        if source_dir.exists() {
            tar.append_dir_all("src", source_dir)?;
        }

        tar.finish()?;

        Ok(())
    }
}
