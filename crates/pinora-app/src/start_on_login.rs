//! 用户级开机自启适配器。
//!
//! 该模块只管理 Pinora 自己创建的启动项。平台差异被限制在本文件，调用方只关心
//! `set_enabled` 的真实成功/失败结果；不通过 shell 解析可执行文件路径。

use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "linux")]
const MANAGED_MARKER: &str = "X-Pinora-Managed=true";
#[cfg(target_os = "macos")]
const MACOS_MANAGED_MARKER: &str = "<!-- PinoraManaged -->";
const AUTOSTART_ARG: &str = "--pinora-autostart";
#[cfg(target_os = "windows")]
const WINDOWS_VALUE_NAME: &str = "Pinora";
#[cfg(target_os = "windows")]
const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(any(target_os = "linux", target_os = "macos"))]
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartOnLoginError {
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Unsupported,
    InvalidExecutable,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Permission,
    Conflict,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Io,
    #[cfg(target_os = "windows")]
    PlatformCommand,
}

impl StartOnLoginError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            Self::Unsupported => "start_on_login_unsupported",
            Self::InvalidExecutable => "start_on_login_invalid_executable",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Permission => "start_on_login_permission_denied",
            Self::Conflict => "start_on_login_conflict",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Io => "start_on_login_io_failed",
            #[cfg(target_os = "windows")]
            Self::PlatformCommand => "start_on_login_platform_failed",
        }
    }
}

/// 应用设置保存前调用；返回成功才允许把 `start_on_login` 写入设置文件。
pub(crate) fn set_enabled(enabled: bool, executable: &Path) -> Result<(), StartOnLoginError> {
    validate_executable(executable)?;
    #[cfg(target_os = "linux")]
    {
        set_linux(enabled, executable)
    }
    #[cfg(target_os = "macos")]
    {
        set_macos(enabled, executable)
    }
    #[cfg(target_os = "windows")]
    {
        set_windows(enabled, executable)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (enabled, executable);
        Err(StartOnLoginError::Unsupported)
    }
}

fn validate_executable(executable: &Path) -> Result<(), StartOnLoginError> {
    if !executable.is_absolute()
        || executable.as_os_str().is_empty()
        || !executable.is_file()
        || executable.to_string_lossy().chars().any(|c| c.is_control())
    {
        return Err(StartOnLoginError::InvalidExecutable);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), StartOnLoginError> {
    let parent = path.parent().ok_or(StartOnLoginError::Io)?;
    std::fs::create_dir_all(parent).map_err(map_io)?;
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".pinora-start-on-login-{id}.tmp"));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary).map_err(map_io)?;
        use std::io::Write;
        file.write_all(content).map_err(map_io)?;
        file.sync_all().map_err(map_io)?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(map_io)?;
        let directory = std::fs::File::open(parent).map_err(map_io)?;
        directory.sync_all().map_err(map_io)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn map_io(error: std::io::Error) -> StartOnLoginError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => StartOnLoginError::Permission,
        _ => StartOnLoginError::Io,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn existing_regular_file(path: &Path) -> Result<Option<String>, StartOnLoginError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(map_io(error)),
    };
    if !metadata.file_type().is_file() {
        return Err(StartOnLoginError::Conflict);
    }
    std::fs::read_to_string(path).map(Some).map_err(map_io)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn remove_owned(path: &Path, marker: &str) -> Result<(), StartOnLoginError> {
    let Some(content) = existing_regular_file(path)? else {
        return Ok(());
    };
    if !content.lines().any(|line| line.trim() == marker) {
        return Err(StartOnLoginError::Conflict);
    }
    std::fs::remove_file(path).map_err(map_io)
}

#[cfg(target_os = "linux")]
fn set_linux(enabled: bool, executable: &Path) -> Result<(), StartOnLoginError> {
    let path = linux_autostart_path()?;
    set_linux_at(&path, enabled, executable)
}

#[cfg(target_os = "linux")]
fn set_linux_at(path: &Path, enabled: bool, executable: &Path) -> Result<(), StartOnLoginError> {
    if !enabled {
        return remove_owned(path, MANAGED_MARKER);
    }
    if let Some(existing) = existing_regular_file(path)?
        && !existing.lines().any(|line| line.trim() == MANAGED_MARKER)
    {
        return Err(StartOnLoginError::Conflict);
    }
    atomic_write(path, linux_desktop_entry(executable)?.as_bytes())
}

#[cfg(target_os = "linux")]
fn linux_autostart_path() -> Result<PathBuf, StartOnLoginError> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or(StartOnLoginError::Permission)?;
    if !config.is_absolute() {
        return Err(StartOnLoginError::Permission);
    }
    Ok(config.join("autostart/pinora.desktop"))
}

#[cfg(target_os = "linux")]
fn linux_desktop_entry(executable: &Path) -> Result<String, StartOnLoginError> {
    let exec = desktop_exec_arg(executable)?;
    let try_exec = desktop_try_exec(executable)?;
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Pinora\nComment=Pinora tray-only screenshot tool\nExec={exec} {AUTOSTART_ARG}\nTryExec={try_exec}\nTerminal=false\nStartupNotify=false\nX-GNOME-Autostart-enabled=true\n{MANAGED_MARKER}\n"
    ))
}

#[cfg(target_os = "linux")]
fn desktop_exec_arg(path: &Path) -> Result<String, StartOnLoginError> {
    let value = path.to_str().ok_or(StartOnLoginError::InvalidExecutable)?;
    if value.is_empty() || value.chars().any(|c| c.is_control()) {
        return Err(StartOnLoginError::InvalidExecutable);
    }
    let needs_quotes = value
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '\\' | '$' | '`'));
    if !needs_quotes {
        return Ok(value.to_string());
    }
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        if matches!(ch, '"' | '\\' | '$' | '`') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    Ok(escaped)
}

#[cfg(target_os = "linux")]
fn desktop_try_exec(path: &Path) -> Result<String, StartOnLoginError> {
    let value = path.to_str().ok_or(StartOnLoginError::InvalidExecutable)?;
    if value.is_empty() || value.chars().any(|c| c.is_control()) {
        return Err(StartOnLoginError::InvalidExecutable);
    }
    Ok(value.to_string())
}

#[cfg(target_os = "macos")]
fn set_macos(enabled: bool, executable: &Path) -> Result<(), StartOnLoginError> {
    let path = macos_launch_agent_path()?;
    if !enabled {
        return remove_owned(&path, MACOS_MANAGED_MARKER);
    }
    if let Some(existing) = existing_regular_file(&path)?
        && !existing
            .lines()
            .any(|line| line.trim() == MACOS_MANAGED_MARKER)
    {
        return Err(StartOnLoginError::Conflict);
    }
    atomic_write(&path, macos_launch_agent(executable)?.as_bytes())
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_path() -> Result<PathBuf, StartOnLoginError> {
    let home = std::env::var_os("HOME").ok_or(StartOnLoginError::Permission)?;
    Ok(PathBuf::from(home).join("Library/LaunchAgents/io.github.yhyzgn.pinora.plist"))
}

#[cfg(target_os = "macos")]
fn macos_launch_agent(executable: &Path) -> Result<String, StartOnLoginError> {
    let executable = xml_escape(
        executable
            .to_str()
            .ok_or(StartOnLoginError::InvalidExecutable)?,
    );
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n<key>Label</key>\n<string>io.github.yhyzgn.pinora</string>\n<key>ProgramArguments</key>\n<array>\n<string>{executable}</string>\n<string>{AUTOSTART_ARG}</string>\n</array>\n<key>RunAtLoad</key>\n<true/>\n{MACOS_MANAGED_MARKER}\n</dict>\n</plist>\n"
    ))
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "windows")]
fn set_windows(enabled: bool, executable: &Path) -> Result<(), StartOnLoginError> {
    let command_line = format!("{} {AUTOSTART_ARG}", windows_arg(executable)?);
    let existing = query_windows_value()?;
    if !enabled {
        if existing
            .as_ref()
            .is_some_and(|value| !value.contains(AUTOSTART_ARG))
        {
            return Err(StartOnLoginError::Conflict);
        }
        if existing.is_none() {
            return Ok(());
        }
        let status = std::process::Command::new("reg")
            .args(["delete", WINDOWS_RUN_KEY, "/v", WINDOWS_VALUE_NAME, "/f"])
            .status()
            .map_err(|_| StartOnLoginError::PlatformCommand)?;
        return status
            .success()
            .then_some(())
            .ok_or(StartOnLoginError::PlatformCommand);
    }
    if existing.is_some_and(|value| !value.contains(AUTOSTART_ARG)) {
        return Err(StartOnLoginError::Conflict);
    }
    let status = std::process::Command::new("reg")
        .args([
            "add",
            WINDOWS_RUN_KEY,
            "/v",
            WINDOWS_VALUE_NAME,
            "/t",
            "REG_SZ",
            "/d",
            &command_line,
            "/f",
        ])
        .status()
        .map_err(|_| StartOnLoginError::PlatformCommand)?;
    status
        .success()
        .then_some(())
        .ok_or(StartOnLoginError::PlatformCommand)
}

#[cfg(target_os = "windows")]
fn query_windows_value() -> Result<Option<String>, StartOnLoginError> {
    let output = std::process::Command::new("reg")
        .args(["query", WINDOWS_RUN_KEY, "/v", WINDOWS_VALUE_NAME])
        .output()
        .map_err(|_| StartOnLoginError::PlatformCommand)?;
    if !output.status.success() {
        let key = std::process::Command::new("reg")
            .args(["query", WINDOWS_RUN_KEY])
            .status()
            .map_err(|_| StartOnLoginError::PlatformCommand)?;
        return key
            .success()
            .then_some(None)
            .ok_or(StartOnLoginError::PlatformCommand);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

#[cfg(target_os = "windows")]
fn windows_arg(path: &Path) -> Result<String, StartOnLoginError> {
    let value = path.to_str().ok_or(StartOnLoginError::InvalidExecutable)?;
    if value.is_empty() || value.chars().any(|c| c.is_control()) {
        return Err(StartOnLoginError::InvalidExecutable);
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    let mut backslashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            out.push_str(&"\\".repeat(backslashes * 2 + 1));
            out.push(ch);
            backslashes = 0;
        } else {
            out.push_str(&"\\".repeat(backslashes));
            out.push(ch);
            backslashes = 0;
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_or_controlled_executable_paths() {
        assert_eq!(
            validate_executable(Path::new("pinora")),
            Err(StartOnLoginError::InvalidExecutable)
        );
        assert_eq!(
            validate_executable(Path::new("/definitely/not/a/pinora/executable")),
            Err(StartOnLoginError::InvalidExecutable)
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            desktop_exec_arg(Path::new("/tmp/pinora\n")),
            Err(StartOnLoginError::InvalidExecutable)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entry_is_tray_only_and_escapes_exec_path() {
        let entry =
            linux_desktop_entry(Path::new("/tmp/Pinora App/pinora")).expect("desktop entry");
        assert!(entry.contains("Type=Application"));
        assert!(entry.contains("Terminal=false"));
        assert!(entry.contains(MANAGED_MARKER));
        assert!(entry.contains("Exec=\"/tmp/Pinora App/pinora\" --pinora-autostart"));
        assert!(entry.contains("TryExec=/tmp/Pinora App/pinora"));
        assert!(entry.contains(AUTOSTART_ARG));
        assert!(!entry.contains("--capture"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn managed_linux_entry_is_atomic_and_unknown_entry_is_never_deleted() {
        let directory = std::env::temp_dir().join(format!(
            "pinora-start-on-login-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join("autostart/pinora.desktop");
        let executable = Path::new("/tmp/pinora-test-bin");

        set_linux_at(&path, true, executable).expect("enable managed entry");
        let content = std::fs::read_to_string(&path).expect("read managed entry");
        assert!(content.contains(MANAGED_MARKER));
        assert!(content.contains("Exec=/tmp/pinora-test-bin --pinora-autostart"));
        set_linux_at(&path, false, executable).expect("disable managed entry");
        assert!(!path.exists());

        std::fs::create_dir_all(path.parent().expect("autostart parent"))
            .expect("create autostart directory");
        std::fs::write(&path, "[Desktop Entry]\nName=Another App\n").expect("write foreign entry");
        assert_eq!(
            set_linux_at(&path, false, executable),
            Err(StartOnLoginError::Conflict)
        );
        assert!(path.exists());
        assert_eq!(
            set_linux_at(&path, true, executable),
            Err(StartOnLoginError::Conflict)
        );
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(directory);
    }
}
