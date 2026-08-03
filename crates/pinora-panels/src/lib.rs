//! Pinora 辅助面板窗口适配。
//!
//! 本 crate 只绑定既有 Panel 模型与 winit/softbuffer 资源，并通过统一的
//! `pinora-desktop::window_policy` 创建和展示窗口。唯一 EventLoop、业务状态写入以及
//! 设置/历史/诊断副作用仍由 `pinora-app` 编排。

mod diagnostics_window;
mod history_window;
mod settings_window;

pub use diagnostics_window::DiagnosticsWindow;
pub use history_window::HistoryWindow;
pub use settings_window::SettingsWindow;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn panel_adapters_use_the_shared_window_policy() {
        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for path in rust_sources(&source_dir) {
            if path.file_name().and_then(|name| name.to_str()) == Some("lib.rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("read panel adapter source");
            assert!(
                !source.contains(".create_window(")
                    && !source.contains(".set_visible(true)")
                    && !source.contains(".with_visible(true)"),
                "{path:?} must not bypass the shared window policy"
            );
        }
        for module in [
            "settings_window.rs",
            "history_window.rs",
            "diagnostics_window.rs",
        ] {
            let path = source_dir.join(module);
            let source = fs::read_to_string(&path).expect("read panel window adapter");
            assert!(
                source.contains("window_policy::create_auxiliary_window"),
                "{path:?} must use the shared creation policy"
            );
            assert!(
                source.contains("window_policy::show_auxiliary_window"),
                "{path:?} must use the shared presentation policy"
            );
        }
    }

    fn rust_sources(dir: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        collect_rust_sources(dir, &mut paths);
        paths
    }

    fn collect_rust_sources(dir: &Path, paths: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(dir).expect("read panel source directory");
        for entry in entries {
            let path = entry.expect("read panel source entry").path();
            if path.is_dir() {
                collect_rust_sources(&path, paths);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                paths.push(path);
            }
        }
    }
}
