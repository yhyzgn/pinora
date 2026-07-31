//! 本地 OCR：通过系统 `tesseract` CLI 识别（不链 C++ 库）。
//!
//! 缺引擎或模型时返回可理解错误；不自动联网下载。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use pinora_core::{
    union_bboxes, CaptureImage, ErrorCode, OcrLine, OcrResult, OcrWord, PixelRect, PinoraError,
};
use png;

/// 探测 tesseract 是否可用。
pub fn tesseract_available() -> bool {
    which("tesseract").is_some()
}

/// 对截图做 OCR。优先 `eng`，有 `chi_sim` 时用 `chi_sim+eng`。
pub fn recognize_image(image: &CaptureImage) -> Result<OcrResult, PinoraError> {
    let bin = which("tesseract").ok_or_else(|| {
        PinoraError::new(
            ErrorCode::Internal,
            "未找到 tesseract：请安装 CLI，例如 `sudo dnf install tesseract tesseract-langpack-eng tesseract-langpack-chi_sim`",
        )
    })?;

    let langs = detect_languages();
    let lang_arg = langs.join("+");

    let png = encode_png_bytes(image)?;
    let tmp = write_temp_png(&png)?;

    let output = run_tesseract_tsv(&bin, &tmp, &lang_arg)?;
    let _ = std::fs::remove_file(&tmp);

    let lines = parse_tsv(&output)?;
    Ok(OcrResult::from_lines(lines, langs, format!("tesseract-cli:{lang_arg}")))
}

fn detect_languages() -> Vec<String> {
    // 优先中英；仅装 eng 时退回 eng
    let listed = list_tesseract_langs();
    let mut langs = Vec::new();
    if listed.iter().any(|l| l == "chi_sim") {
        langs.push("chi_sim".into());
    }
    if listed.iter().any(|l| l == "eng") || langs.is_empty() {
        langs.push("eng".into());
    }
    // 去重保持顺序
    let mut out = Vec::new();
    for l in langs {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

fn list_tesseract_langs() -> Vec<String> {
    let Some(bin) = which("tesseract") else {
        return Vec::new();
    };
    let Ok(out) = Command::new(bin)
        .arg("--list-langs")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    else {
        return probe_tessdata_files();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut langs: Vec<String> = text
        .lines()
        .skip(1) // "List of available languages..."
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if langs.is_empty() {
        langs = probe_tessdata_files();
    }
    langs
}

fn probe_tessdata_files() -> Vec<String> {
    let dirs = [
        "/usr/share/tesseract/tessdata",
        "/usr/share/tessdata",
        "/usr/share/tesseract-ocr/5/tessdata",
        "/usr/share/tesseract-ocr/4.00/tessdata",
    ];
    let mut langs = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let name = ent.file_name();
            let name = name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".traineddata") {
                if stem != "osd" && stem != "equ" {
                    langs.push(stem.to_string());
                }
            }
        }
    }
    langs
}

fn run_tesseract_tsv(bin: &Path, image: &Path, langs: &str) -> Result<String, PinoraError> {
    // tesseract img stdout -l lang tsv
    let child = Command::new(bin)
        .args([
            image.to_str().unwrap_or(""),
            "stdout",
            "-l",
            langs,
            "--psm",
            "3",
            "tsv",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("spawn tesseract: {e}")))?;

    // 超时保护
    let (tx, rx) = std::sync::mpsc::channel();
    let child_id = child.id();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(out)) => {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(PinoraError::new(
                    ErrorCode::Internal,
                    format!("tesseract failed: {}", err.trim()),
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        Ok(Err(e)) => Err(PinoraError::new(
            ErrorCode::Internal,
            format!("tesseract wait: {e}"),
        )),
        Err(_) => {
            let _ = Command::new("kill")
                .args(["-9", &child_id.to_string()])
                .status();
            Err(PinoraError::new(
                ErrorCode::Internal,
                "tesseract timed out (30s)",
            ))
        }
    }
}

/// 解析 tesseract TSV（level=5 为 word）。
pub fn parse_tsv(tsv: &str) -> Result<Vec<OcrLine>, PinoraError> {
    let mut lines: Vec<OcrLine> = Vec::new();
    let mut current_key: Option<(i32, i32, i32, i32)> = None; // block,par,line page ignored
    let mut current_words: Vec<OcrWord> = Vec::new();

    for (i, row) in tsv.lines().enumerate() {
        if i == 0 {
            continue; // header
        }
        if row.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let level: i32 = cols[0].parse().unwrap_or(0);
        if level != 5 {
            continue;
        }
        let block: i32 = cols[2].parse().unwrap_or(0);
        let par: i32 = cols[3].parse().unwrap_or(0);
        let line: i32 = cols[4].parse().unwrap_or(0);
        let left: i32 = cols[6].parse().unwrap_or(0);
        let top: i32 = cols[7].parse().unwrap_or(0);
        let width: u32 = cols[8].parse().unwrap_or(0);
        let height: u32 = cols[9].parse().unwrap_or(0);
        let conf: f32 = cols[10].parse().unwrap_or(-1.0);
        let text = cols[11..].join("\t").trim().to_string();
        if text.is_empty() {
            continue;
        }
        let key = (block, par, line, 0);
        if current_key != Some(key) {
            if !current_words.is_empty() {
                let bbox = union_bboxes(current_words.iter().map(|w| w.bbox));
                lines.push(OcrLine {
                    words: std::mem::take(&mut current_words),
                    bbox,
                });
            }
            current_key = Some(key);
        }
        current_words.push(OcrWord {
            text,
            confidence: conf,
            bbox: PixelRect::new(left, top, width, height),
        });
    }
    if !current_words.is_empty() {
        let bbox = union_bboxes(current_words.iter().map(|w| w.bbox));
        lines.push(OcrLine {
            words: current_words,
            bbox,
        });
    }
    Ok(lines)
}

fn encode_png_bytes(image: &CaptureImage) -> Result<Vec<u8>, PinoraError> {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let width = image.pixels.size.width;
        let height = image.pixels.size.height;
        let mut encoder = png::Encoder::new(cursor, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("ocr png header: {e}"))
        })?;
        writer
            .write_image_data(&image.pixels.bytes)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr png data: {e}")))?;
    }
    Ok(buf)
}

fn write_temp_png(png: &[u8]) -> Result<PathBuf, PinoraError> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "pinora-ocr-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let mut f = std::fs::File::create(&path)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr temp: {e}")))?;
    f.write_all(png)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr temp write: {e}")))?;
    Ok(path)
}

fn which(name: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PATH") {
        for dir in std::env::split_paths(&p) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_tsv() {
        let tsv = "\
level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext
1\t1\t0\t0\t0\t0\t0\t0\t100\t50\t-1\t
5\t1\t1\t1\t1\t1\t10\t10\t20\t12\t96.5\tHello
5\t1\t1\t1\t1\t2\t35\t10\t18\t12\t94.0\tWorld
5\t1\t1\t1\t2\t1\t10\t30\t25\t12\t91.0\tOCR
";
        let lines = parse_tsv(tsv).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "Hello World");
        assert_eq!(lines[1].text(), "OCR");
        let r = OcrResult::from_lines(lines, vec!["eng".into()], "test");
        assert_eq!(r.full_text, "Hello World\nOCR");
    }

    #[test]
    fn tesseract_probe_does_not_panic() {
        let _ = tesseract_available();
        let _ = list_tesseract_langs();
    }

    #[test]
    fn live_tesseract_selects_chi_sim_and_eng_when_installed() {
        if !tesseract_available() {
            return;
        }
        let langs = detect_languages();
        assert!(
            langs.iter().any(|l| l == "eng"),
            "expected eng in {langs:?}"
        );
        // 本机已装 chi_sim 时应优先中英
        let listed = list_tesseract_langs();
        if listed.iter().any(|l| l == "chi_sim") {
            assert!(
                langs.iter().any(|l| l == "chi_sim"),
                "expected chi_sim in {langs:?}"
            );
        }

        use pinora_core::{
            CaptureImage, CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer,
        };
        let image = CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::solid(PixelSize::new(64, 32), [255, 255, 255, 255]),
            PixelRect::new(0, 0, 64, 32),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap();
        // 白图可能无字，但引擎调用应成功
        let result = recognize_image(&image).expect("tesseract should run");
        assert_eq!(result.languages, langs);
    }
}
