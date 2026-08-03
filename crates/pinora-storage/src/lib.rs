//! Pinora 本地持久化边界：版本化设置、历史索引和受管文件命名。
//!
//! 本 crate 不创建窗口、不运行任务、不访问系统剪贴板，也不拥有 OCR/捕获资源。

mod export_name;
mod history_store;
mod settings_store;

pub use export_name::ExportNameAllocator;
pub use history_store::{HistoryLoad, HistoryStore, default_history_path};
pub use settings_store::{SettingsLoad, SettingsStore, default_settings_path};
