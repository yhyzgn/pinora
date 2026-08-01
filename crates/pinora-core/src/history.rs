//! 历史索引的纯领域模型。
//!
//! 历史只保存受管文件的相对引用和不可变元数据。文件删除由应用层执行，
//! 领域层先写 tombstone，避免索引在文件操作之前丢失事实。

use std::path::Path;

use crate::{AssetGeneration, DisplayId, ImageId, PixelRect};

pub const HISTORY_SCHEMA_VERSION: u16 = 1;
pub const HISTORY_MAX_ENTRIES: usize = 10_000;
pub const HISTORY_MAX_FILE_NAME_BYTES: usize = 240;
pub const HISTORY_MAX_DISPLAY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOcrState {
    Unknown,
    Pending,
    Ready,
    Failed,
}

impl HistoryOcrState {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Pending => 1,
            Self::Ready => 2,
            Self::Failed => 3,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Unknown),
            1 => Some(Self::Pending),
            2 => Some(Self::Ready),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryEntryState {
    Active,
    Tombstone,
}

impl HistoryEntryState {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Tombstone => 1,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Active),
            1 => Some(Self::Tombstone),
            _ => None,
        }
    }
}

/// 用于去重和完整性检查的 SHA-256 内容摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn of(data: &[u8]) -> Self {
        Self(sha256(data))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub image_id: ImageId,
    pub generation: AssetGeneration,
    pub created_at_ms: u64,
    pub display: DisplayId,
    pub source_rect: PixelRect,
    /// 只能是受管目录下的单个相对文件名，不接受绝对路径或 `..`。
    pub file_name: String,
    pub byte_len: u64,
    pub digest: ContentDigest,
    pub ocr: HistoryOcrState,
    pub state: HistoryEntryState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntrySpec {
    pub image_id: ImageId,
    pub generation: AssetGeneration,
    pub created_at_ms: u64,
    pub display: DisplayId,
    pub source_rect: PixelRect,
    pub file_name: String,
    pub byte_len: u64,
    pub digest: ContentDigest,
    pub ocr: HistoryOcrState,
}

impl HistoryEntry {
    pub fn new(spec: HistoryEntrySpec) -> Result<Self, &'static str> {
        let entry = Self {
            image_id: spec.image_id,
            generation: spec.generation,
            created_at_ms: spec.created_at_ms,
            display: spec.display,
            source_rect: spec.source_rect,
            file_name: spec.file_name,
            byte_len: spec.byte_len,
            digest: spec.digest,
            ocr: spec.ocr,
            state: HistoryEntryState::Active,
        };
        entry.validate()?;
        Ok(entry)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.image_id.raw() == 0 {
            return Err("history image id is zero");
        }
        if self.source_rect.size.is_empty() {
            return Err("history source rect is empty");
        }
        if self.byte_len == 0 {
            return Err("history file length is zero");
        }
        validate_relative_file_name(&self.file_name)?;
        if self.display.0.is_empty() || self.display.0.len() > HISTORY_MAX_DISPLAY_BYTES {
            return Err("history display id length is invalid");
        }
        if !self.display.0.is_char_boundary(self.display.0.len()) {
            return Err("history display id is invalid utf-8");
        }
        Ok(())
    }

    pub const fn is_active(&self) -> bool {
        matches!(self.state, HistoryEntryState::Active)
    }

    pub fn mark_tombstone(&mut self) {
        self.state = HistoryEntryState::Tombstone;
    }
}

fn validate_relative_file_name(file_name: &str) -> Result<(), &'static str> {
    if file_name.is_empty() || file_name.len() > HISTORY_MAX_FILE_NAME_BYTES {
        return Err("history file name length is invalid");
    }
    if file_name == "." || file_name == ".." || file_name.contains('/') || file_name.contains('\\')
    {
        return Err("history file name must be a single relative component");
    }
    let path = Path::new(file_name);
    if path.is_absolute() || path.components().count() != 1 {
        return Err("history file name must be relative");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryInsert {
    pub evicted: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryIndex {
    entries: Vec<HistoryEntry>,
    max_entries: usize,
    max_bytes: u64,
}

impl HistoryIndex {
    pub fn new(max_entries: usize, max_bytes: u64) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.clamp(1, HISTORY_MAX_ENTRIES),
            max_bytes: max_bytes.max(1),
        }
    }

    pub fn from_entries(
        entries: Vec<HistoryEntry>,
        max_entries: usize,
        max_bytes: u64,
    ) -> Result<Self, &'static str> {
        if entries.len() > HISTORY_MAX_ENTRIES {
            return Err("history entry count exceeds limit");
        }
        for entry in &entries {
            entry.validate()?;
        }
        let mut index = Self::new(max_entries, max_bytes);
        index.entries = entries;
        index.entries.sort_by(|a, b| {
            b.created_at_ms
                .cmp(&a.created_at_ms)
                .then_with(|| b.image_id.raw().cmp(&a.image_id.raw()))
        });
        Ok(index)
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn active_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.entries.iter().filter(|entry| entry.is_active())
    }

    pub fn active_count(&self) -> usize {
        self.active_entries().count()
    }

    pub fn active_bytes(&self) -> u64 {
        self.active_entries().map(|entry| entry.byte_len).sum()
    }

    /// 更新配额并将超出部分按最旧优先标记为 tombstone。
    ///
    /// 返回本次被标记的条目，应用层可在索引持久化成功后执行受管文件清理。
    pub fn set_limits(&mut self, max_entries: usize, max_bytes: u64) -> Vec<HistoryEntry> {
        self.max_entries = max_entries.clamp(1, HISTORY_MAX_ENTRIES);
        self.max_bytes = max_bytes.max(1);
        let mut evicted = Vec::new();
        while self.active_count() > self.max_entries || self.active_bytes() > self.max_bytes {
            let Some(index) = self.entries.iter().rposition(HistoryEntry::is_active) else {
                break;
            };
            let entry = &mut self.entries[index];
            entry.mark_tombstone();
            evicted.push(entry.clone());
        }
        evicted
    }

    pub fn insert(&mut self, entry: HistoryEntry) -> Result<HistoryInsert, &'static str> {
        entry.validate()?;
        if !entry.is_active() {
            return Err("history insert requires active entry");
        }
        let mut evicted = Vec::new();
        for existing in &mut self.entries {
            if existing.is_active()
                && (existing.image_id == entry.image_id
                    || (existing.byte_len == entry.byte_len && existing.digest == entry.digest))
            {
                existing.mark_tombstone();
                evicted.push(existing.clone());
            }
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| {
            b.created_at_ms
                .cmp(&a.created_at_ms)
                .then_with(|| b.image_id.raw().cmp(&a.image_id.raw()))
        });
        while self.active_count() > self.max_entries || self.active_bytes() > self.max_bytes {
            let Some(index) = self.entries.iter().rposition(HistoryEntry::is_active) else {
                break;
            };
            let existing = &mut self.entries[index];
            existing.mark_tombstone();
            evicted.push(existing.clone());
        }
        Ok(HistoryInsert { evicted })
    }

    pub fn mark_deleted(&mut self, image_id: ImageId) -> Option<HistoryEntry> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.is_active() && entry.image_id == image_id)?;
        entry.mark_tombstone();
        Some(entry.clone())
    }

    pub fn compact(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(HistoryEntry::is_active);
        before.saturating_sub(self.entries.len())
    }

    /// Physically remove only tombstones whose external file operation has completed.
    ///
    /// Callers must retain every tombstone for which deletion was not confirmed, so a later
    /// recovery pass can retry without losing the durable deletion intent.
    pub fn compact_confirmed_tombstones<F>(&mut self, mut is_confirmed: F) -> usize
    where
        F: FnMut(&HistoryEntry) -> bool,
    {
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.is_active() || !is_confirmed(entry));
        before.saturating_sub(self.entries.len())
    }
}

// SHA-256 is kept local to avoid adding a serialization/hash dependency to the core crate.
fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).take(16).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (value, add) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *value = value.wrapping_add(add);
        }
    }
    let mut out = [0u8; 32];
    for (index, value) in h.into_iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, created: u64, bytes: u64, digest: ContentDigest) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: created,
            display: DisplayId::new("display-0"),
            source_rect: PixelRect::new(0, 0, 10, 10),
            file_name: format!("img-{id}.png"),
            byte_len: bytes,
            digest,
            ocr: HistoryOcrState::Unknown,
        })
        .expect("valid history entry")
    }

    #[test]
    fn digest_matches_sha256_vector() {
        let digest = ContentDigest::of(b"abc");
        assert_eq!(
            digest.as_bytes(),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn insert_deduplicates_and_applies_byte_quota_as_tombstones() {
        let digest = ContentDigest::of(b"same");
        let mut index = HistoryIndex::new(2, 10);
        index.insert(entry(1, 1, 6, digest)).expect("insert");
        let result = index.insert(entry(2, 2, 6, digest)).expect("dedupe");
        assert_eq!(result.evicted.len(), 1);
        assert_eq!(index.active_count(), 1);
        assert_eq!(index.entries()[0].image_id.raw(), 2);
        assert_eq!(index.compact(), 1);
    }

    #[test]
    fn mark_delete_is_recoverable_until_compact() {
        let mut index = HistoryIndex::new(4, 100);
        index
            .insert(entry(3, 3, 4, ContentDigest::of(b"3")))
            .expect("insert");
        let deleted = index.mark_deleted(ImageId::from_raw(3)).expect("delete");
        assert_eq!(deleted.state, HistoryEntryState::Tombstone);
        assert_eq!(index.active_count(), 0);
        assert_eq!(index.entries().len(), 1);
        assert_eq!(index.compact(), 1);
        assert!(index.entries().is_empty());
    }

    #[test]
    fn changing_limits_marks_oldest_entries_as_tombstones() {
        let mut index = HistoryIndex::new(4, 100);
        index
            .insert(entry(10, 10, 4, ContentDigest::of(b"10")))
            .expect("insert first");
        index
            .insert(entry(11, 11, 4, ContentDigest::of(b"11")))
            .expect("insert second");
        let evicted = index.set_limits(1, 100);
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0].image_id, ImageId::from_raw(10));
        assert_eq!(index.active_count(), 1);
    }

    #[test]
    fn confirmed_tombstone_compaction_preserves_unconfirmed_entries() {
        let mut index = HistoryIndex::new(4, 100);
        index
            .insert(entry(5, 5, 4, ContentDigest::of(b"5")))
            .expect("insert first");
        index
            .insert(entry(6, 6, 4, ContentDigest::of(b"6")))
            .expect("insert second");
        index
            .mark_deleted(ImageId::from_raw(5))
            .expect("mark first");
        index
            .mark_deleted(ImageId::from_raw(6))
            .expect("mark second");

        let compacted =
            index.compact_confirmed_tombstones(|entry| entry.image_id == ImageId::from_raw(5));

        assert_eq!(compacted, 1);
        assert_eq!(index.entries().len(), 1);
        assert_eq!(index.entries()[0].image_id, ImageId::from_raw(6));
        assert_eq!(index.entries()[0].state, HistoryEntryState::Tombstone);
    }

    #[test]
    fn file_name_must_not_escape_managed_directory() {
        let err = HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(4),
            generation: AssetGeneration::INITIAL,
            created_at_ms: 0,
            display: DisplayId::new("display-0"),
            source_rect: PixelRect::new(0, 0, 1, 1),
            file_name: "../outside.png".into(),
            byte_len: 1,
            digest: ContentDigest::of(b"x"),
            ocr: HistoryOcrState::Unknown,
        })
        .unwrap_err();
        assert!(err.contains("relative"));
    }
}
