//! 交互区域截图工作流：全屏捕获 → Overlay 选区 → 裁剪。

use pinora_core::{
    CaptureImage, CaptureProvider, CaptureRequest, DisplayInfo, ErrorCode, PinoraError, PixelPoint,
    PixelRect,
};

use crate::region_overlay::run_region_selection;

/// 交互选区结果。
#[derive(Debug, Clone)]
pub struct RegionCaptureResult {
    pub image: CaptureImage,
    /// 建议贴图位置（选区右下外侧一点）。
    pub pin_position: PixelPoint,
    pub selection_local: PixelRect,
}

/// 在主显示器上运行区域选区；取消返回 `None`。
pub fn capture_region_interactive(
    capture: &impl CaptureProvider,
) -> Result<Option<RegionCaptureResult>, PinoraError> {
    let displays = capture.displays()?;
    let display = select_primary(&displays).ok_or_else(|| {
        PinoraError::new(ErrorCode::NotFound, "no display available for region capture")
    })?;

    println!(
        "pinora: preparing region overlay on {} ({}x{}) …",
        display.name, display.bounds.size.width, display.bounds.size.height
    );

    let full = capture.capture(CaptureRequest::FullDisplay {
        display: display.id.clone(),
    })?;

    println!("pinora: drag to select region — Enter confirm, Esc cancel");
    let selection = run_region_selection(&full)?;
    let Some(local) = selection else {
        println!("pinora: region selection cancelled");
        return Ok(None);
    };

    let image = full.crop_local(local)?;
    let pin_position = PixelPoint::new(
        image.source_rect.origin.x.saturating_add(24),
        image.source_rect.origin.y.saturating_add(24),
    );

    println!(
        "pinora: selection {}x{} at ({}, {})",
        local.size.width, local.size.height, local.origin.x, local.origin.y
    );

    Ok(Some(RegionCaptureResult {
        image,
        pin_position,
        selection_local: local,
    }))
}

fn select_primary(displays: &[DisplayInfo]) -> Option<&DisplayInfo> {
    displays
        .iter()
        .find(|d| d.name.to_ascii_lowercase().contains("primary"))
        .or_else(|| displays.first())
}
