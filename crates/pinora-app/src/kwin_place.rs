//! 在 KDE Wayland 上把窗口钉到绝对坐标。
//!
//! 标准 xdg-shell / winit 的 `set_outer_position` 在 Wayland 上是空操作，
//! 新建窗会被 KWin 摆到屏幕中央。这里通过 KWin Scripting D-Bus
//! 设置 `frameGeometry`，与 Spectacle 等 KDE 应用同权。

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// 按窗口标题子串，把匹配到的第一个窗口放到 (x,y,w,h)。
/// `delay_ms`：等待窗口映射进 KWin 的毫秒数。
pub fn place_window_by_title(
    title_substr: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    delay_ms: u64,
) {
    let title = title_substr.to_string();
    thread::spawn(move || {
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        if let Err(e) = place_once(&title, x, y, width, height) {
            eprintln!("pinora: kwin place failed: {e}");
        } else {
            println!("pinora: kwin place '{title}' → ({x},{y}) {width}x{height}");
        }
        // 再钉一次，防止 configure 竞态
        thread::sleep(Duration::from_millis(80));
        let _ = place_once(&title, x, y, width, height);
    });
}

/// 同步放置：Enter 无缝转贴图时用（先钉好再关 overlay，避免闪桌面）。
pub fn place_window_by_title_sync(
    title_substr: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    // 短轮询：窗口刚 map 时 KWin 列表可能尚未收录
    let mut last_err = String::new();
    for attempt in 0..12 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(16));
        }
        match place_once(title_substr, x, y, width, height) {
            Ok(()) => {
                // 紧接再钉一次，压住合成器回弹
                let _ = place_once(title_substr, x, y, width, height);
                println!("pinora: kwin place sync '{title_substr}' → ({x},{y}) {width}x{height}");
                return Ok(());
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

fn place_once(title_substr: &str, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
    // 转义进 JS 字符串
    let title_js = escape_js(title_substr);
    let script = format!(
        r#"
// pinora place — generated
var list = workspace.windowList();
for (var i = 0; i < list.length; ++i) {{
    var c = list[i];
    var cap = "" + c.caption;
    if (cap.indexOf("{title_js}") !== -1) {{
        c.frameGeometry = {{
            x: {x},
            y: {y},
            width: {width},
            height: {height}
        }};
        // 保持置顶
        try {{ c.keepAbove = true; }} catch (e) {{}}
        break;
    }}
}}
"#
    );

    let path = script_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, script).map_err(|e| format!("write script: {e}"))?;

    // 唯一脚本名，避免并发 place 互相 unload
    let name = format!(
        "pinora-place-{}-{}",
        std::process::id(),
        now_ms() % 1_000_000
    );
    let _ = busctl(&[
        "call",
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
        "unloadScript",
        "s",
        &name,
    ]);

    let out = busctl(&[
        "call",
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
        "loadScript",
        "ss",
        &path.to_string_lossy(),
        &name,
    ])?;

    // 输出形如: i 3
    let script_id = parse_script_id(&out).unwrap_or(0);
    let obj = format!("/Scripting/Script{script_id}");
    busctl(&["call", "org.kde.KWin", &obj, "org.kde.kwin.Script", "run"])?;

    let _ = busctl(&[
        "call",
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
        "unloadScript",
        "s",
        &name,
    ]);

    Ok(())
}

fn now_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn script_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("pinora/kwin-place.js");
    }
    std::env::temp_dir().join("pinora-kwin-place.js")
}

fn escape_js(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
        .replace(['\n', '\r'], " ")
}

fn parse_script_id(busctl_out: &str) -> Option<u32> {
    // "i 3\n" or "i 3"
    let parts: Vec<&str> = busctl_out.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "i" {
        return parts[1].parse().ok();
    }
    // sometimes just the number
    busctl_out
        .split_whitespace()
        .find_map(|p| p.parse::<u32>().ok())
}

fn busctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("busctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| format!("busctl spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "busctl {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 当前是否可能在 KWin 会话（有 org.kde.KWin）。
pub fn kwin_available() -> bool {
    Command::new("busctl")
        .args(["--user", "status", "org.kde.KWin"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_js_basic() {
        assert_eq!(escape_js(r#"a"b"#), r#"a\"b"#);
    }

    #[test]
    fn parse_id() {
        assert_eq!(parse_script_id("i 7"), Some(7));
        assert_eq!(parse_script_id("i 0\n"), Some(0));
    }
}
