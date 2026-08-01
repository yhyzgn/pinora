//! History persistence after a managed PNG export has passed its job gate.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pinora_core::{
    AssetRef, ContentDigest, HistoryEntry, HistoryEntrySpec, HistoryIndex, HistoryInsert,
    HistoryOcrState, JobOwner,
};

use crate::export_job::ExportJobInput;
use crate::history_store::{HistoryLoad, HistoryStore};

#[derive(Debug)]
pub(crate) struct HistoryExportCandidate {
    pub owner: JobOwner,
    pub asset: AssetRef,
    display: pinora_core::DisplayId,
    source_rect: pinora_core::PixelRect,
    created_at_ms: u64,
    file_name: String,
    path: PathBuf,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HistoryCleanup {
    pub removed_files: usize,
    pub missing_files: usize,
    pub protected_files: usize,
    pub failed_files: usize,
    pub compacted_entries: usize,
}

pub(crate) fn load_history_index(store: &HistoryStore) -> Result<HistoryIndex, String> {
    match store.load() {
        HistoryLoad::Missing(index) | HistoryLoad::Loaded(index) => Ok(index),
        HistoryLoad::Invalid(error) => Err(error),
    }
}

/// A history candidate exists only for a PNG written directly below the managed export directory.
pub(crate) fn history_candidate_for_export(
    export_dir: &Path,
    owner: JobOwner,
    asset: AssetRef,
    input: &ExportJobInput,
) -> Option<HistoryExportCandidate> {
    let ExportJobInput::SavePng { image, path } = input else {
        return None;
    };
    if image.id != asset.image_id {
        return None;
    }
    let file_name = managed_png_file_name(export_dir, path)?;
    Some(HistoryExportCandidate {
        owner,
        asset,
        display: image.metadata.display.clone(),
        source_rect: image.source_rect,
        created_at_ms: image.metadata.captured_at_ms,
        file_name,
        path: path.clone(),
    })
}

fn managed_png_file_name(export_dir: &Path, path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    if path != export_dir.join(file_name) {
        return None;
    }
    (Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        == Some("png"))
    .then(|| file_name.to_owned())
}

/// Insert an entry only after the PNG is readable; restore the in-memory index if persistence fails.
pub(crate) fn record_history_candidate(
    store: &HistoryStore,
    index: &mut HistoryIndex,
    candidate: HistoryExportCandidate,
) -> Result<HistoryInsert, String> {
    let bytes = std::fs::read(&candidate.path)
        .map_err(|error| format!("read managed history export: {error}"))?;
    let byte_len = u64::try_from(bytes.len()).map_err(|_| "history export is too large")?;
    let entry = HistoryEntry::new(HistoryEntrySpec {
        image_id: candidate.asset.image_id,
        generation: candidate.asset.generation,
        created_at_ms: candidate.created_at_ms,
        display: candidate.display,
        source_rect: candidate.source_rect,
        file_name: candidate.file_name,
        byte_len,
        digest: ContentDigest::of(&bytes),
        ocr: HistoryOcrState::Unknown,
    })
    .map_err(str::to_string)?;

    let previous = index.clone();
    let inserted = index.insert(entry).map_err(str::to_string)?;
    if let Err(error) = store.save(index) {
        *index = previous;
        return Err(format!("save history index: {error}"));
    }
    Ok(inserted)
}

/// Remove only tombstoned files directly below the managed export directory, then persist their
/// confirmed compaction. Tombstones remain durable when a delete or index save cannot complete.
pub(crate) fn cleanup_history_tombstones(
    store: &HistoryStore,
    export_dir: &Path,
    index: &mut HistoryIndex,
) -> Result<HistoryCleanup, String> {
    let active_names: HashSet<&str> = index
        .active_entries()
        .map(|entry| entry.file_name.as_str())
        .collect();
    let tombstone_names: HashSet<&str> = index
        .entries()
        .iter()
        .filter(|entry| !entry.is_active())
        .map(|entry| entry.file_name.as_str())
        .collect();
    let mut cleanup = HistoryCleanup::default();
    let mut completed_names = HashSet::new();

    for file_name in tombstone_names {
        if active_names.contains(file_name) {
            cleanup.protected_files += 1;
            continue;
        }
        let Some(path) = managed_history_png_path(export_dir, file_name) else {
            cleanup.failed_files += 1;
            continue;
        };
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
                match std::fs::remove_file(path) {
                    Ok(()) => {
                        cleanup.removed_files += 1;
                        completed_names.insert(file_name.to_owned());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        cleanup.missing_files += 1;
                        completed_names.insert(file_name.to_owned());
                    }
                    Err(_) => cleanup.failed_files += 1,
                }
            }
            Ok(_) => cleanup.failed_files += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                cleanup.missing_files += 1;
                completed_names.insert(file_name.to_owned());
            }
            Err(_) => cleanup.failed_files += 1,
        }
    }

    if completed_names.is_empty() {
        return Ok(cleanup);
    }

    let previous = index.clone();
    cleanup.compacted_entries = index
        .compact_confirmed_tombstones(|entry| completed_names.contains(entry.file_name.as_str()));
    if cleanup.compacted_entries == 0 {
        return Ok(cleanup);
    }
    if let Err(error) = store.save(index) {
        *index = previous;
        return Err(format!("save compacted history index: {error}"));
    }
    Ok(cleanup)
}

fn managed_history_png_path(export_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let file = Path::new(file_name);
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file.is_absolute()
        || file.components().count() != 1
        || file.extension().and_then(|extension| extension.to_str()) != Some("png")
    {
        return None;
    }
    Some(export_dir.join(file))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use pinora_core::{
        AssetGeneration, AssetRef, CaptureImage, CaptureMetadata, DisplayId, HistoryEntryState,
        ImageId, PixelRect, PixelSize, RgbaBuffer, SessionId,
    };

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pinora-history-export-test-{}-{}-{name}",
            std::process::id(),
            id
        ));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn sample_image(id: ImageId) -> CaptureImage {
        CaptureImage::new(
            id,
            RgbaBuffer::solid(PixelSize::new(2, 2), [1, 2, 3, 255]),
            PixelRect::new(40, 50, 2, 2),
            CaptureMetadata::new(DisplayId::new("display-1"), 1.0, 1234),
        )
        .expect("image")
    }

    fn session_owner() -> JobOwner {
        JobOwner::Session(SessionId::from_raw(9))
    }

    fn history_entry(image_id: u64, file_name: &str, bytes: &[u8]) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(image_id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: image_id,
            display: DisplayId::new("display-1"),
            source_rect: PixelRect::new(40, 50, 2, 2),
            file_name: file_name.into(),
            byte_len: u64::try_from(bytes.len()).expect("test payload length"),
            digest: ContentDigest::of(bytes),
            ocr: HistoryOcrState::Unknown,
        })
        .expect("history entry")
    }

    #[test]
    fn managed_png_completion_records_relative_metadata() {
        let root = temp_root("records");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        let image = sample_image(ImageId::from_raw(41));
        let asset = AssetRef::initial(image.id);
        let path = export_dir.join("img-41.png");
        fs::write(&path, b"png payload").expect("write export");
        let input = ExportJobInput::SavePng {
            image: image.clone(),
            path,
        };
        let candidate = history_candidate_for_export(&export_dir, session_owner(), asset, &input)
            .expect("managed candidate");
        let store = HistoryStore::new(root.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();

        record_history_candidate(&store, &mut index, candidate).expect("record history");

        let entry = index.entries().first().expect("history entry");
        assert_eq!(entry.image_id, image.id);
        assert_eq!(entry.generation, asset.generation);
        assert_eq!(entry.display, image.metadata.display);
        assert_eq!(entry.source_rect, image.source_rect);
        assert_eq!(entry.file_name, "img-41.png");
        assert_eq!(entry.byte_len, u64::try_from(b"png payload".len()).unwrap());
        assert_eq!(entry.digest, ContentDigest::of(b"png payload"));
        assert_eq!(entry.ocr, HistoryOcrState::Unknown);
        assert!(matches!(store.load(), HistoryLoad::Loaded(loaded) if loaded == index));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_managed_png_saves_produce_candidates() {
        let root = temp_root("managed-only");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        let image = sample_image(ImageId::from_raw(42));
        let asset = AssetRef::initial(image.id);

        let external = ExportJobInput::SavePng {
            image: image.clone(),
            path: root.join("external.png"),
        };
        assert!(
            history_candidate_for_export(&export_dir, session_owner(), asset, &external).is_none()
        );
        let nested = ExportJobInput::SavePng {
            image: image.clone(),
            path: export_dir.join("nested/img-42.png"),
        };
        assert!(
            history_candidate_for_export(&export_dir, session_owner(), asset, &nested).is_none()
        );
        let non_png = ExportJobInput::SavePng {
            image: image.clone(),
            path: export_dir.join("img-42.jpg"),
        };
        assert!(
            history_candidate_for_export(&export_dir, session_owner(), asset, &non_png).is_none()
        );
        assert!(
            history_candidate_for_export(
                &export_dir,
                session_owner(),
                asset,
                &ExportJobInput::CopyImage {
                    image: image.clone()
                },
            )
            .is_none()
        );
        assert!(
            history_candidate_for_export(
                &export_dir,
                session_owner(),
                asset,
                &ExportJobInput::CopyText {
                    text: "OCR text".into()
                },
            )
            .is_none()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_index_save_restores_in_memory_index() {
        let root = temp_root("rollback");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        let image = sample_image(ImageId::from_raw(43));
        let asset = AssetRef::initial(image.id);
        let path = export_dir.join("img-43.png");
        fs::write(&path, b"png payload").expect("write export");
        let candidate = history_candidate_for_export(
            &export_dir,
            session_owner(),
            asset,
            &ExportJobInput::SavePng { image, path },
        )
        .expect("managed candidate");
        let blocked_parent = root.join("blocked-parent");
        fs::write(&blocked_parent, b"not a directory").expect("block history parent");
        let store = HistoryStore::new(blocked_parent.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();
        let before = index.clone();

        let error =
            record_history_candidate(&store, &mut index, candidate).expect_err("save fails");

        assert!(error.contains("save history index"));
        assert_eq!(index, before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_index_starts_with_empty_memory_without_overwrite() {
        let root = temp_root("invalid");
        let path = root.join("history.bin");
        fs::write(&path, b"corrupt history").expect("write corrupt index");
        let store = HistoryStore::new(path.clone(), 10, u64::MAX);

        let error = load_history_index(&store).expect_err("invalid index");

        assert!(!error.is_empty());
        assert_eq!(
            fs::read(&path).expect("read corrupt index"),
            b"corrupt history"
        );
        assert!(store.empty_index().entries().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_removes_managed_tombstone_and_persists_compaction() {
        let root = temp_root("cleanup");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        fs::write(export_dir.join("old.png"), b"old").expect("write old export");
        fs::write(export_dir.join("active.png"), b"active").expect("write active export");
        let store = HistoryStore::new(root.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();
        index
            .insert(history_entry(61, "old.png", b"old"))
            .expect("insert old");
        index.mark_deleted(ImageId::from_raw(61)).expect("mark old");
        index
            .insert(history_entry(62, "active.png", b"active"))
            .expect("insert active");
        store.save(&index).expect("save tombstone");

        let cleanup =
            cleanup_history_tombstones(&store, &export_dir, &mut index).expect("cleanup history");

        assert_eq!(cleanup.removed_files, 1);
        assert_eq!(cleanup.missing_files, 0);
        assert_eq!(cleanup.protected_files, 0);
        assert_eq!(cleanup.failed_files, 0);
        assert_eq!(cleanup.compacted_entries, 1);
        assert!(!export_dir.join("old.png").exists());
        assert!(export_dir.join("active.png").is_file());
        assert_eq!(index.entries().len(), 1);
        assert_eq!(index.entries()[0].file_name, "active.png");
        assert!(matches!(store.load(), HistoryLoad::Loaded(loaded) if loaded == index));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_compacts_missing_file_without_touching_active_same_name() {
        let root = temp_root("missing-and-protected");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        fs::write(export_dir.join("shared.png"), b"active").expect("write shared export");
        let store = HistoryStore::new(root.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();
        index
            .insert(history_entry(63, "missing.png", b"missing"))
            .expect("insert missing");
        index
            .mark_deleted(ImageId::from_raw(63))
            .expect("mark missing");
        index
            .insert(history_entry(64, "shared.png", b"retired"))
            .expect("insert shared tombstone");
        index
            .mark_deleted(ImageId::from_raw(64))
            .expect("mark shared tombstone");
        index
            .insert(history_entry(65, "shared.png", b"active"))
            .expect("insert shared active");
        store.save(&index).expect("save tombstones");

        let cleanup =
            cleanup_history_tombstones(&store, &export_dir, &mut index).expect("cleanup history");

        assert_eq!(cleanup.removed_files, 0);
        assert_eq!(cleanup.missing_files, 1);
        assert_eq!(cleanup.protected_files, 1);
        assert_eq!(cleanup.failed_files, 0);
        assert_eq!(cleanup.compacted_entries, 1);
        assert!(export_dir.join("shared.png").is_file());
        assert_eq!(index.entries().len(), 2);
        assert!(index.entries().iter().any(|entry| {
            entry.file_name == "shared.png" && entry.state == HistoryEntryState::Tombstone
        }));
        assert!(index.entries().iter().any(|entry| {
            entry.file_name == "shared.png" && entry.state == HistoryEntryState::Active
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_keeps_tombstone_when_file_is_not_removable() {
        let root = temp_root("directory");
        let export_dir = root.join("exports");
        fs::create_dir_all(export_dir.join("blocked.png")).expect("create blocking directory");
        let store = HistoryStore::new(root.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();
        index
            .insert(history_entry(66, "blocked.png", b"blocked"))
            .expect("insert blocked");
        index
            .mark_deleted(ImageId::from_raw(66))
            .expect("mark blocked");
        store.save(&index).expect("save tombstone");

        let cleanup = cleanup_history_tombstones(&store, &export_dir, &mut index)
            .expect("cleanup reports failure");

        assert_eq!(cleanup.failed_files, 1);
        assert_eq!(cleanup.compacted_entries, 0);
        assert_eq!(index.entries().len(), 1);
        assert_eq!(index.entries()[0].state, HistoryEntryState::Tombstone);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_save_failure_restores_tombstone_in_memory() {
        let root = temp_root("save-rollback");
        let export_dir = root.join("exports");
        fs::create_dir_all(&export_dir).expect("create exports");
        fs::write(export_dir.join("old.png"), b"old").expect("write old export");
        let blocked_parent = root.join("blocked-parent");
        fs::write(&blocked_parent, b"not a directory").expect("block history parent");
        let store = HistoryStore::new(blocked_parent.join("history.bin"), 10, u64::MAX);
        let mut index = store.empty_index();
        index
            .insert(history_entry(67, "old.png", b"old"))
            .expect("insert old");
        index.mark_deleted(ImageId::from_raw(67)).expect("mark old");
        let before = index.clone();

        let error = cleanup_history_tombstones(&store, &export_dir, &mut index)
            .expect_err("compacted index save fails");

        assert!(error.contains("save compacted history index"));
        assert_eq!(index, before);
        assert!(!export_dir.join("old.png").exists());
        let _ = fs::remove_dir_all(root);
    }
}
