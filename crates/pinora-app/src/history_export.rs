//! History persistence after a managed PNG export has passed its job gate.

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use pinora_core::{
        AssetRef, CaptureImage, CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize,
        RgbaBuffer, SessionId,
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
}
