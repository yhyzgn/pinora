//! 版本化设置的固定长度 codec 与原子本地存储。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pinora_core::{AppSettings, SETTINGS_SCHEMA_VERSION, SettingsRepairs, ThemeMode};

const MAGIC: [u8; 8] = *b"PINORA\0\0";
const RECORD_LEN: usize = 18;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsLoad {
    Missing(AppSettings),
    Loaded {
        settings: AppSettings,
        repairs: SettingsRepairs,
    },
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> SettingsLoad {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return SettingsLoad::Missing(AppSettings::default());
            }
            Err(error) => return SettingsLoad::Invalid(format!("read settings: {error}")),
        };
        match decode(&bytes) {
            Ok((settings, repairs)) => SettingsLoad::Loaded { settings, repairs },
            Err(error) => SettingsLoad::Invalid(error),
        }
    }

    pub fn save(&self, settings: AppSettings) -> Result<(), String> {
        let bytes = encode(settings)?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create settings directory: {error}"))?;
        let mut temporary = AtomicSettingsTemp::create(parent)?;
        let mut file = temporary.take_file()?;
        file.write_all(&bytes)
            .map_err(|error| format!("write settings: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync settings: {error}"))?;
        drop(file);
        temporary.commit(&self.path)?;
        match self.load() {
            SettingsLoad::Loaded {
                settings: read_back,
                ..
            } if read_back == settings => Ok(()),
            SettingsLoad::Loaded { .. } => Err("verify settings: repaired values differ".into()),
            SettingsLoad::Missing(_) => Err("verify settings: file disappeared".into()),
            SettingsLoad::Invalid(error) => Err(format!("verify settings: {error}")),
        }
    }
}

/// 当前 Linux 实验路径的设置位置。只负责定位，调用者决定是否读写。
pub fn default_settings_path() -> PathBuf {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("pinora/settings.bin");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/pinora/settings.bin");
    }
    std::env::temp_dir().join("pinora-settings/settings.bin")
}

fn encode(settings: AppSettings) -> Result<[u8; RECORD_LEN], String> {
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err("settings schema is not current".into());
    }
    let (repaired, repairs) = settings.with_repaired_values();
    if !repairs.is_empty() {
        return Err("settings contain invalid values".into());
    }
    let mut bytes = [0u8; RECORD_LEN];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&repaired.schema_version.to_le_bytes());
    bytes[10] = repaired.theme.to_wire();
    bytes[11..15].copy_from_slice(&repaired.history_limit.to_le_bytes());
    bytes[15..17].copy_from_slice(&repaired.pin_limit.to_le_bytes());
    bytes[17] = repaired.default_pin_opacity_percent;
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    if bytes.len() != RECORD_LEN {
        return Err("settings record length is invalid".into());
    }
    if bytes[..8] != MAGIC {
        return Err("settings magic is invalid".into());
    }
    let schema_version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if schema_version != SETTINGS_SCHEMA_VERSION {
        return Err("settings schema version is unsupported".into());
    }
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    Ok(AppSettings {
        schema_version,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
    }
    .with_repaired_values())
}

struct AtomicSettingsTemp {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl AtomicSettingsTemp {
    fn create(directory: &Path) -> Result<Self, String> {
        for _ in 0..16 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(".pinora-settings-{}-{id}.tmp", std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create settings temp: {error}")),
            }
        }
        Err("create settings temp: collision limit reached".into())
    }
    fn take_file(&mut self) -> Result<File, String> {
        self.file
            .take()
            .ok_or_else(|| "settings temp file already moved".into())
    }
    fn commit(mut self, target: &Path) -> Result<(), String> {
        if self.file.is_some() {
            return Err("settings temp file is still open".into());
        }
        std::fs::rename(&self.path, target)
            .map_err(|error| format!("publish settings: {error}"))?;
        self.committed = true;
        File::open(target).map_err(|error| format!("verify published settings: {error}"))?;
        Ok(())
    }
}

impl Drop for AtomicSettingsTemp {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pinora-settings-test-{}-{name}",
            std::process::id()
        ))
    }
    #[test]
    fn missing_uses_defaults_and_invalid_is_preserved() {
        let path = path("invalid.bin");
        let store = SettingsStore::new(path.clone());
        let _ = std::fs::remove_file(&path);
        assert!(matches!(store.load(), SettingsLoad::Missing(_)));
        std::fs::write(&path, b"bad").expect("write invalid");
        assert!(matches!(store.load(), SettingsLoad::Invalid(_)));
        assert_eq!(std::fs::read(&path).expect("read invalid"), b"bad");
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn save_replaces_existing_and_loads_exact_values() {
        let path = path("save.bin");
        let store = SettingsStore::new(path.clone());
        let _ = std::fs::remove_file(&path);
        let settings = AppSettings {
            theme: ThemeMode::Dark,
            history_limit: 88,
            pin_limit: 8,
            default_pin_opacity_percent: 75,
            ..AppSettings::default()
        };
        store.save(settings).expect("save");
        assert!(
            matches!(store.load(), SettingsLoad::Loaded { settings: loaded, repairs } if loaded == settings && repairs.is_empty())
        );
        assert!(
            !path
                .with_file_name(format!(".pinora-settings-{}-1.tmp", std::process::id()))
                .exists()
        );
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn unknown_schema_is_rejected() {
        let mut bytes = encode(AppSettings::default()).expect("encode");
        bytes[8] = 2;
        assert_eq!(
            decode(&bytes),
            Err("settings schema version is unsupported".into())
        );
    }

    #[test]
    fn default_path_has_a_stable_filename() {
        assert_eq!(
            default_settings_path().file_name(),
            Some(std::ffi::OsStr::new("settings.bin"))
        );
    }
}
