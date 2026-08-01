//! 历史索引的版本化 codec 与原子文件存储。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use pinora_core::{
    AssetGeneration, ContentDigest, DisplayId, HISTORY_MAX_ENTRIES, HISTORY_SCHEMA_VERSION,
    HistoryEntry, HistoryEntrySpec, HistoryEntryState, HistoryIndex, HistoryOcrState, ImageId,
    PixelRect,
};

const MAGIC: [u8; 8] = *b"PINHIST\0";
const HEADER_LEN: usize = 24;
const RECORD_PREFIX_LEN: usize = 86;
const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryLoad {
    Missing(HistoryIndex),
    Loaded(HistoryIndex),
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
    max_entries: usize,
    max_bytes: u64,
}

impl HistoryStore {
    pub fn new(path: PathBuf, max_entries: usize, max_bytes: u64) -> Self {
        Self {
            path,
            max_entries: max_entries.clamp(1, HISTORY_MAX_ENTRIES),
            max_bytes: max_bytes.max(1),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn empty_index(&self) -> HistoryIndex {
        HistoryIndex::new(self.max_entries, self.max_bytes)
    }

    pub fn load(&self) -> HistoryLoad {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return HistoryLoad::Missing(self.empty_index());
            }
            Err(error) => return HistoryLoad::Invalid(format!("read history index: {error}")),
        };
        match decode(&bytes, self.max_entries, self.max_bytes) {
            Ok(index) => HistoryLoad::Loaded(index),
            Err(error) => HistoryLoad::Invalid(error),
        }
    }

    pub fn save(&self, index: &HistoryIndex) -> Result<(), String> {
        let bytes = encode(index)?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create history directory: {error}"))?;
        let mut temporary = AtomicHistoryTemp::create(parent)?;
        let mut file = temporary.take_file()?;
        file.write_all(&bytes)
            .map_err(|error| format!("write history index: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync history index: {error}"))?;
        drop(file);
        temporary.commit(&self.path)?;
        match self.load() {
            HistoryLoad::Loaded(read_back) if read_back == *index => Ok(()),
            HistoryLoad::Loaded(_) => Err("verify history index: values differ".into()),
            HistoryLoad::Missing(_) => Err("verify history index: file disappeared".into()),
            HistoryLoad::Invalid(error) => Err(format!("verify history index: {error}")),
        }
    }
}

/// 当前 Linux 实验路径的历史索引位置。文件内容只由 `HistoryStore` 写入。
pub fn default_history_path() -> PathBuf {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("pinora/history.bin");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/pinora/history.bin");
    }
    std::env::temp_dir().join("pinora-settings/history.bin")
}

fn encode(index: &HistoryIndex) -> Result<Vec<u8>, String> {
    if index.entries().len() > HISTORY_MAX_ENTRIES {
        return Err("history entry count exceeds limit".into());
    }
    let mut payload = Vec::new();
    for entry in index.entries() {
        entry.validate().map_err(str::to_string)?;
        let display = entry.display.0.as_bytes();
        let file_name = entry.file_name.as_bytes();
        let display_len = u16::try_from(display.len()).map_err(|_| "history display too long")?;
        let file_name_len =
            u16::try_from(file_name.len()).map_err(|_| "history file name too long")?;
        payload.push(entry.state.to_wire());
        payload.extend_from_slice(&entry.image_id.raw().to_le_bytes());
        payload.extend_from_slice(&entry.generation.raw().to_le_bytes());
        payload.extend_from_slice(&entry.created_at_ms.to_le_bytes());
        payload.extend_from_slice(&display_len.to_le_bytes());
        payload.extend_from_slice(&file_name_len.to_le_bytes());
        payload.extend_from_slice(&entry.source_rect.origin.x.to_le_bytes());
        payload.extend_from_slice(&entry.source_rect.origin.y.to_le_bytes());
        payload.extend_from_slice(&entry.source_rect.size.width.to_le_bytes());
        payload.extend_from_slice(&entry.source_rect.size.height.to_le_bytes());
        payload.extend_from_slice(&entry.byte_len.to_le_bytes());
        payload.push(entry.ocr.to_wire());
        payload.extend_from_slice(&entry.digest.as_bytes());
        payload.extend_from_slice(display);
        payload.extend_from_slice(file_name);
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_| "history payload too large")?;
    let total_len = HEADER_LEN
        .checked_add(payload.len())
        .ok_or("history index length overflow")?;
    if total_len > MAX_INDEX_BYTES {
        return Err("history index exceeds size limit".into());
    }
    let count =
        u32::try_from(index.entries().len()).map_err(|_| "history entry count too large")?;
    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&HISTORY_SCHEMA_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&crc32(&payload).to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn decode(bytes: &[u8], max_entries: usize, max_bytes: u64) -> Result<HistoryIndex, String> {
    if bytes.len() < HEADER_LEN || bytes.len() > MAX_INDEX_BYTES {
        return Err("history index length is invalid".into());
    }
    if bytes[..8] != MAGIC {
        return Err("history index magic is invalid".into());
    }
    let schema = u16::from_le_bytes([bytes[8], bytes[9]]);
    if schema != HISTORY_SCHEMA_VERSION {
        return Err("history index schema version is unsupported".into());
    }
    if bytes[10] != 0 || bytes[11] != 0 {
        return Err("history index reserved bytes are invalid".into());
    }
    let count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if count > HISTORY_MAX_ENTRIES || count > max_entries.saturating_mul(2) {
        return Err("history entry count is invalid".into());
    }
    let payload_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    if payload_len != bytes.len() - HEADER_LEN {
        return Err("history payload length is invalid".into());
    }
    let checksum = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let payload = &bytes[HEADER_LEN..];
    if crc32(payload) != checksum {
        return Err("history payload checksum is invalid".into());
    }

    let mut reader = Reader::new(payload);
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if reader.remaining() < RECORD_PREFIX_LEN {
            return Err("history record is truncated".into());
        }
        let state = HistoryEntryState::from_wire(reader.u8()?)
            .ok_or_else(|| "history entry state is invalid".to_string())?;
        let image_id = ImageId::from_raw(reader.u64()?);
        let generation = AssetGeneration::from_raw(reader.u64()?)
            .ok_or_else(|| "history generation is invalid".to_string())?;
        let created_at_ms = reader.u64()?;
        let display_len = reader.u16()? as usize;
        let file_name_len = reader.u16()? as usize;
        if display_len > pinora_core::HISTORY_MAX_DISPLAY_BYTES
            || file_name_len > pinora_core::HISTORY_MAX_FILE_NAME_BYTES
        {
            return Err("history string length exceeds limit".into());
        }
        let source_rect =
            PixelRect::new(reader.i32()?, reader.i32()?, reader.u32()?, reader.u32()?);
        let byte_len = reader.u64()?;
        let ocr = HistoryOcrState::from_wire(reader.u8()?)
            .ok_or_else(|| "history OCR state is invalid".to_string())?;
        let mut digest = [0u8; 32];
        digest.copy_from_slice(reader.bytes(32)?);
        let display = String::from_utf8(reader.bytes(display_len)?.to_vec())
            .map_err(|_| "history display id is invalid utf-8".to_string())?;
        let file_name = String::from_utf8(reader.bytes(file_name_len)?.to_vec())
            .map_err(|_| "history file name is invalid utf-8".to_string())?;
        let mut entry = HistoryEntry::new(HistoryEntrySpec {
            image_id,
            generation,
            created_at_ms,
            display: DisplayId::new(display),
            source_rect,
            file_name,
            byte_len,
            digest: ContentDigest::from_bytes(digest),
            ocr,
        })
        .map_err(str::to_string)?;
        entry.state = state;
        entries.push(entry);
    }
    if reader.remaining() != 0 {
        return Err("history payload has trailing bytes".into());
    }
    HistoryIndex::from_entries(entries, max_entries, max_bytes).map_err(str::to_string)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "history record length overflow".to_string())?;
        if end > self.bytes.len() {
            return Err("history record is truncated".into());
        }
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
    fn bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        self.take(len)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("length checked"),
        ))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("length checked"),
        ))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("length checked"),
        ))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().expect("length checked"),
        ))
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb88320 & mask);
        }
    }
    !crc
}

struct AtomicHistoryTemp {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl AtomicHistoryTemp {
    fn create(directory: &Path) -> Result<Self, String> {
        for _ in 0..16 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(".pinora-history-{}-{id}.tmp", std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create history temp: {error}")),
            }
        }
        Err("create history temp: collision limit reached".into())
    }

    fn take_file(&mut self) -> Result<File, String> {
        self.file
            .take()
            .ok_or_else(|| "history temp file already moved".into())
    }

    fn commit(mut self, target: &Path) -> Result<(), String> {
        if self.file.is_some() {
            return Err("history temp file is still open".into());
        }
        std::fs::rename(&self.path, target)
            .map_err(|error| format!("publish history index: {error}"))?;
        self.committed = true;
        let metadata = std::fs::metadata(target)
            .map_err(|error| format!("verify published history index: {error}"))?;
        if metadata.len() == 0 {
            return Err("verify published history index: empty file".into());
        }
        Ok(())
    }
}

impl Drop for AtomicHistoryTemp {
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
        std::env::temp_dir().join(format!("pinora-history-test-{}-{name}", std::process::id()))
    }

    fn sample_index() -> HistoryIndex {
        let store = HistoryStore::new(path("unused.bin"), 10, 100_000);
        let mut index = store.empty_index();
        let entry = HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(8),
            generation: AssetGeneration::INITIAL,
            created_at_ms: 42,
            display: DisplayId::new("display-0"),
            source_rect: PixelRect::new(1, 2, 3, 4),
            file_name: "img-8.png".into(),
            byte_len: 4,
            digest: ContentDigest::of(b"pixels"),
            ocr: HistoryOcrState::Ready,
        })
        .expect("entry");
        index.insert(entry).expect("insert");
        index
    }

    #[test]
    fn missing_and_corrupt_index_are_distinct_and_preserved() {
        let path = path("invalid.bin");
        let _ = std::fs::remove_file(&path);
        let store = HistoryStore::new(path.clone(), 10, 100_000);
        assert!(matches!(store.load(), HistoryLoad::Missing(_)));
        std::fs::write(&path, b"corrupt").expect("write corrupt");
        assert!(matches!(store.load(), HistoryLoad::Invalid(_)));
        assert_eq!(std::fs::read(&path).expect("read corrupt"), b"corrupt");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_round_trips_tombstone_and_removes_temp_file() {
        let path = path("save.bin");
        let _ = std::fs::remove_file(&path);
        let store = HistoryStore::new(path.clone(), 10, 100_000);
        let mut index = sample_index();
        index.mark_deleted(ImageId::from_raw(8));
        store.save(&index).expect("save");
        assert!(matches!(store.load(), HistoryLoad::Loaded(loaded) if loaded == index));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_schema_and_checksum_are_rejected() {
        let path = path("schema.bin");
        let _ = std::fs::remove_file(&path);
        let store = HistoryStore::new(path.clone(), 10, 100_000);
        let index = sample_index();
        store.save(&index).expect("save");
        let mut bytes = std::fs::read(&path).expect("read");
        bytes[8] = 2;
        std::fs::write(&path, &bytes).expect("write schema");
        assert!(matches!(store.load(), HistoryLoad::Invalid(error) if error.contains("schema")));
        bytes[8] = HISTORY_SCHEMA_VERSION as u8;
        bytes[20] ^= 0xff;
        std::fs::write(&path, &bytes).expect("write checksum");
        assert!(matches!(store.load(), HistoryLoad::Invalid(error) if error.contains("checksum")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn default_path_has_stable_file_name() {
        assert_eq!(
            default_history_path().file_name(),
            Some(std::ffi::OsStr::new("history.bin"))
        );
    }
}
