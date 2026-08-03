//! 版本化设置的固定长度 codec 与原子本地存储。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pinora_core::{
    AppSettings, DEFAULT_FULL_DISPLAY_HOTKEY, DEFAULT_OCR_CONFIDENCE_THRESHOLD,
    DEFAULT_PIN_ALWAYS_ON_TOP, DEFAULT_REGION_HOTKEY, HotkeyBinding, HotkeyCode, HotkeyModifiers,
    OcrLanguage, SETTINGS_SCHEMA_VERSION, SettingsRepairs, ThemeMode,
};

const MAGIC: [u8; 8] = *b"PINORA\0\0";
const V1_RECORD_LEN: usize = 18;
const V2_RECORD_LEN: usize = 19;
const V3_RECORD_LEN: usize = 23;
const V4_RECORD_LEN: usize = 24;
const RECORD_LEN: usize = 25;
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
    bytes[18] = repaired.ocr_language.to_wire();
    bytes[19] = repaired.region_hotkey.modifiers.to_wire();
    bytes[20] = repaired.region_hotkey.code.to_wire();
    bytes[21] = repaired.full_display_hotkey.modifiers.to_wire();
    bytes[22] = repaired.full_display_hotkey.code.to_wire();
    bytes[23] = u8::from(repaired.default_pin_always_on_top);
    bytes[24] = repaired.ocr_confidence_threshold;
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    match bytes.len() {
        V1_RECORD_LEN => decode_v1(bytes),
        V2_RECORD_LEN => decode_v2(bytes),
        V3_RECORD_LEN => decode_v3(bytes),
        V4_RECORD_LEN => decode_v4(bytes),
        RECORD_LEN => decode_v5(bytes),
        _ => Err("settings record length is invalid".into()),
    }
}

fn decode_v1(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    validate_magic_and_schema(bytes, 1)?;
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    let (settings, mut repairs) = AppSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
        default_pin_always_on_top: DEFAULT_PIN_ALWAYS_ON_TOP,
        ocr_language: OcrLanguage::Auto,
        ocr_confidence_threshold: DEFAULT_OCR_CONFIDENCE_THRESHOLD,
        region_hotkey: DEFAULT_REGION_HOTKEY,
        full_display_hotkey: DEFAULT_FULL_DISPLAY_HOTKEY,
    }
    .with_repaired_values();
    repairs.migrated_from_v1 = true;
    Ok((settings, repairs))
}

fn decode_v2(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    validate_magic_and_schema(bytes, 2)?;
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let ocr_language = OcrLanguage::from_wire(bytes[18])
        .ok_or_else(|| "settings OCR language is invalid".to_string())?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    let (settings, mut repairs) = AppSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
        default_pin_always_on_top: DEFAULT_PIN_ALWAYS_ON_TOP,
        ocr_language,
        ocr_confidence_threshold: DEFAULT_OCR_CONFIDENCE_THRESHOLD,
        region_hotkey: DEFAULT_REGION_HOTKEY,
        full_display_hotkey: DEFAULT_FULL_DISPLAY_HOTKEY,
    }
    .with_repaired_values();
    repairs.migrated_from_v2 = true;
    Ok((settings, repairs))
}

fn decode_v3(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    validate_magic_and_schema(bytes, 3)?;
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let ocr_language = OcrLanguage::from_wire(bytes[18])
        .ok_or_else(|| "settings OCR language is invalid".to_string())?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    let (region_hotkey, region_invalid) =
        decode_hotkey(bytes[19], bytes[20], DEFAULT_REGION_HOTKEY);
    let (full_display_hotkey, full_invalid) =
        decode_hotkey(bytes[21], bytes[22], DEFAULT_FULL_DISPLAY_HOTKEY);
    let (settings, mut repairs) = AppSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
        default_pin_always_on_top: DEFAULT_PIN_ALWAYS_ON_TOP,
        ocr_language,
        ocr_confidence_threshold: DEFAULT_OCR_CONFIDENCE_THRESHOLD,
        region_hotkey,
        full_display_hotkey,
    }
    .with_repaired_values();
    repairs.region_hotkey |= region_invalid;
    repairs.full_display_hotkey |= full_invalid;
    repairs.migrated_from_v3 = true;
    Ok((settings, repairs))
}

fn decode_v4(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    validate_magic_and_schema(bytes, 4)?;
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let ocr_language = OcrLanguage::from_wire(bytes[18])
        .ok_or_else(|| "settings OCR language is invalid".to_string())?;
    let default_pin_always_on_top = decode_bool(bytes[23])?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    let (region_hotkey, region_invalid) =
        decode_hotkey(bytes[19], bytes[20], DEFAULT_REGION_HOTKEY);
    let (full_display_hotkey, full_invalid) =
        decode_hotkey(bytes[21], bytes[22], DEFAULT_FULL_DISPLAY_HOTKEY);
    let (settings, mut repairs) = AppSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
        default_pin_always_on_top,
        ocr_language,
        ocr_confidence_threshold: DEFAULT_OCR_CONFIDENCE_THRESHOLD,
        region_hotkey,
        full_display_hotkey,
    }
    .with_repaired_values();
    repairs.region_hotkey |= region_invalid;
    repairs.full_display_hotkey |= full_invalid;
    repairs.migrated_from_v4 = true;
    Ok((settings, repairs))
}

fn decode_v5(bytes: &[u8]) -> Result<(AppSettings, SettingsRepairs), String> {
    validate_magic_and_schema(bytes, SETTINGS_SCHEMA_VERSION)?;
    let theme =
        ThemeMode::from_wire(bytes[10]).ok_or_else(|| "settings theme is invalid".to_string())?;
    let ocr_language = OcrLanguage::from_wire(bytes[18])
        .ok_or_else(|| "settings OCR language is invalid".to_string())?;
    let default_pin_always_on_top = decode_bool(bytes[23])?;
    let history_limit = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
    let pin_limit = u16::from_le_bytes([bytes[15], bytes[16]]);
    let (region_hotkey, region_invalid) =
        decode_hotkey(bytes[19], bytes[20], DEFAULT_REGION_HOTKEY);
    let (full_display_hotkey, full_invalid) =
        decode_hotkey(bytes[21], bytes[22], DEFAULT_FULL_DISPLAY_HOTKEY);
    let (settings, mut repairs) = AppSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        theme,
        history_limit,
        pin_limit,
        default_pin_opacity_percent: bytes[17],
        default_pin_always_on_top,
        ocr_language,
        ocr_confidence_threshold: bytes[24],
        region_hotkey,
        full_display_hotkey,
    }
    .with_repaired_values();
    repairs.region_hotkey |= region_invalid;
    repairs.full_display_hotkey |= full_invalid;
    Ok((settings, repairs))
}

fn decode_bool(value: u8) -> Result<bool, String> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("settings default pin always on top is invalid".into()),
    }
}

fn decode_hotkey(modifiers: u8, code: u8, default: HotkeyBinding) -> (HotkeyBinding, bool) {
    match (
        HotkeyModifiers::from_wire(modifiers),
        HotkeyCode::from_wire(code),
    ) {
        (Some(modifiers), Some(code)) => (HotkeyBinding::new(modifiers, code), false),
        _ => (default, true),
    }
}

fn validate_magic_and_schema(bytes: &[u8], expected_schema: u16) -> Result<(), String> {
    if bytes[..8] != MAGIC {
        return Err("settings magic is invalid".into());
    }
    let schema_version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if schema_version != expected_schema {
        return Err("settings schema version is unsupported".into());
    }
    Ok(())
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
            default_pin_always_on_top: false,
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
        bytes[8] = 6;
        assert_eq!(
            decode(&bytes),
            Err("settings schema version is unsupported".into())
        );
    }

    #[test]
    fn v1_settings_migrate_without_losing_existing_fields() {
        let bytes = legacy_v1_bytes(ThemeMode::Dark, 88, 8, 75);

        let (settings, repairs) = decode(&bytes).expect("migrate v1");

        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.theme, ThemeMode::Dark);
        assert_eq!(settings.history_limit, 88);
        assert_eq!(settings.pin_limit, 8);
        assert_eq!(settings.default_pin_opacity_percent, 75);
        assert_eq!(
            settings.default_pin_always_on_top,
            DEFAULT_PIN_ALWAYS_ON_TOP
        );
        assert_eq!(settings.ocr_language, OcrLanguage::Auto);
        assert_eq!(
            settings.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert!(repairs.migrated_from_v1);
    }

    #[test]
    fn v5_round_trip_preserves_ocr_settings_hotkeys_and_default_pin_level() {
        let settings = AppSettings {
            default_pin_always_on_top: false,
            ocr_language: OcrLanguage::SimplifiedChinese,
            ocr_confidence_threshold: 85,
            region_hotkey: HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR),
            full_display_hotkey: HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4),
            ..AppSettings::default()
        };

        let (decoded, repairs) = decode(&encode(settings).expect("encode v5")).expect("decode v5");

        assert_eq!(decoded, settings);
        assert!(repairs.is_empty());
    }

    #[test]
    fn invalid_v5_ocr_language_is_rejected() {
        let mut bytes = encode(AppSettings::default()).expect("encode");
        bytes[18] = u8::MAX;

        assert_eq!(
            decode(&bytes),
            Err("settings OCR language is invalid".into())
        );
    }

    #[test]
    fn v2_settings_migrate_with_default_hotkeys() {
        let bytes = legacy_v2_bytes(ThemeMode::Light, OcrLanguage::English, 77, 7, 80);

        let (settings, repairs) = decode(&bytes).expect("migrate v2");

        assert_eq!(settings.theme, ThemeMode::Light);
        assert_eq!(settings.ocr_language, OcrLanguage::English);
        assert_eq!(settings.region_hotkey, DEFAULT_REGION_HOTKEY);
        assert_eq!(settings.full_display_hotkey, DEFAULT_FULL_DISPLAY_HOTKEY);
        assert_eq!(
            settings.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert_eq!(
            settings.default_pin_always_on_top,
            DEFAULT_PIN_ALWAYS_ON_TOP
        );
        assert!(repairs.migrated_from_v2);
    }

    #[test]
    fn v3_settings_migrate_with_default_always_on_top() {
        let region = HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR);
        let full_display = HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4);
        let bytes = legacy_v3_bytes(
            ThemeMode::Dark,
            OcrLanguage::SimplifiedChinese,
            91,
            9,
            85,
            region,
            full_display,
        );

        let (settings, repairs) = decode(&bytes).expect("migrate v3");

        assert_eq!(settings.theme, ThemeMode::Dark);
        assert_eq!(settings.ocr_language, OcrLanguage::SimplifiedChinese);
        assert_eq!(settings.history_limit, 91);
        assert_eq!(settings.pin_limit, 9);
        assert_eq!(settings.default_pin_opacity_percent, 85);
        assert_eq!(settings.region_hotkey, region);
        assert_eq!(settings.full_display_hotkey, full_display);
        assert_eq!(
            settings.default_pin_always_on_top,
            DEFAULT_PIN_ALWAYS_ON_TOP
        );
        assert_eq!(
            settings.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert!(repairs.migrated_from_v3);
    }

    #[test]
    fn v4_settings_migrate_with_default_ocr_confidence_threshold() {
        let region = HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR);
        let full_display = HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4);
        let mut bytes = [0u8; V4_RECORD_LEN];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&4u16.to_le_bytes());
        bytes[10] = ThemeMode::Dark.to_wire();
        bytes[11..15].copy_from_slice(&91u32.to_le_bytes());
        bytes[15..17].copy_from_slice(&9u16.to_le_bytes());
        bytes[17] = 85;
        bytes[18] = OcrLanguage::SimplifiedChinese.to_wire();
        bytes[19] = region.modifiers.to_wire();
        bytes[20] = region.code.to_wire();
        bytes[21] = full_display.modifiers.to_wire();
        bytes[22] = full_display.code.to_wire();
        bytes[23] = 0;

        let (settings, repairs) = decode(&bytes).expect("migrate v4");

        assert_eq!(settings.theme, ThemeMode::Dark);
        assert_eq!(settings.ocr_language, OcrLanguage::SimplifiedChinese);
        assert_eq!(
            settings.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert_eq!(settings.region_hotkey, region);
        assert_eq!(settings.full_display_hotkey, full_display);
        assert!(repairs.migrated_from_v4);
    }

    #[test]
    fn invalid_v5_hotkey_field_repairs_without_losing_other_field() {
        let settings = AppSettings {
            region_hotkey: HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR),
            full_display_hotkey: HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4),
            ..AppSettings::default()
        };
        let mut bytes = encode(settings).expect("encode");
        bytes[20] = u8::MAX;

        let (decoded, repairs) = decode(&bytes).expect("repair v5 hotkey");

        assert_eq!(decoded.region_hotkey, DEFAULT_REGION_HOTKEY);
        assert_eq!(decoded.full_display_hotkey, settings.full_display_hotkey);
        assert!(repairs.region_hotkey);
        assert!(!repairs.full_display_hotkey);
    }

    #[test]
    fn invalid_v5_default_pin_level_is_rejected_without_replacing_the_source_file() {
        let path = path("invalid-v5-default-pin-level.bin");
        let store = SettingsStore::new(path.clone());
        let _ = std::fs::remove_file(&path);
        let mut bytes = encode(AppSettings::default()).expect("encode");
        bytes[23] = 2;
        std::fs::write(&path, bytes).expect("write invalid v5");

        assert!(
            matches!(store.load(), SettingsLoad::Invalid(error) if error == "settings default pin always on top is invalid")
        );
        assert_eq!(std::fs::read(&path).expect("read invalid v5"), bytes);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_v5_ocr_confidence_threshold_repairs_without_losing_other_fields() {
        let settings = AppSettings {
            ocr_confidence_threshold: 85,
            region_hotkey: HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR),
            ..AppSettings::default()
        };
        let mut bytes = encode(settings).expect("encode");
        bytes[24] = 101;

        let (decoded, repairs) = decode(&bytes).expect("repair threshold");

        assert_eq!(
            decoded.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert_eq!(decoded.region_hotkey, settings.region_hotkey);
        assert!(repairs.ocr_confidence_threshold);
        assert!(!repairs.region_hotkey);
    }

    #[test]
    fn default_path_has_a_stable_filename() {
        assert_eq!(
            default_settings_path().file_name(),
            Some(std::ffi::OsStr::new("settings.bin"))
        );
    }

    fn legacy_v1_bytes(
        theme: ThemeMode,
        history_limit: u32,
        pin_limit: u16,
        opacity: u8,
    ) -> [u8; V1_RECORD_LEN] {
        let mut bytes = [0u8; V1_RECORD_LEN];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = theme.to_wire();
        bytes[11..15].copy_from_slice(&history_limit.to_le_bytes());
        bytes[15..17].copy_from_slice(&pin_limit.to_le_bytes());
        bytes[17] = opacity;
        bytes
    }

    fn legacy_v2_bytes(
        theme: ThemeMode,
        language: OcrLanguage,
        history_limit: u32,
        pin_limit: u16,
        opacity: u8,
    ) -> [u8; V2_RECORD_LEN] {
        let mut bytes = [0u8; V2_RECORD_LEN];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
        bytes[10] = theme.to_wire();
        bytes[11..15].copy_from_slice(&history_limit.to_le_bytes());
        bytes[15..17].copy_from_slice(&pin_limit.to_le_bytes());
        bytes[17] = opacity;
        bytes[18] = language.to_wire();
        bytes
    }

    fn legacy_v3_bytes(
        theme: ThemeMode,
        language: OcrLanguage,
        history_limit: u32,
        pin_limit: u16,
        opacity: u8,
        region_hotkey: HotkeyBinding,
        full_display_hotkey: HotkeyBinding,
    ) -> [u8; V3_RECORD_LEN] {
        let mut bytes = [0u8; V3_RECORD_LEN];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
        bytes[10] = theme.to_wire();
        bytes[11..15].copy_from_slice(&history_limit.to_le_bytes());
        bytes[15..17].copy_from_slice(&pin_limit.to_le_bytes());
        bytes[17] = opacity;
        bytes[18] = language.to_wire();
        bytes[19] = region_hotkey.modifiers.to_wire();
        bytes[20] = region_hotkey.code.to_wire();
        bytes[21] = full_display_hotkey.modifiers.to_wire();
        bytes[22] = full_display_hotkey.code.to_wire();
        bytes
    }
}
