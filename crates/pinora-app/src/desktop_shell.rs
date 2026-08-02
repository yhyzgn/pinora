//! 统一桌面事件循环：选区 Overlay + 贴图窗口（同一 EventLoop，适配 Wayland）。
//!
//! 选区策略：遮罩立即弹出；全屏预览在后台线程用当前 CaptureProvider 抓取
//! （KDE 优先 spectacle/KWin，避免 xcap→portal）；用户拖选期间通常已完成，
//! Enter 时优先内存裁剪，避免再等 Portal。

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    OnceLock,
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread;
use std::time::{Duration, Instant};

use crate::export_job::{ExportJobCompletion, ExportJobInput, ExportJobService};
use crate::frame_cache::{FrameCache, rgba_to_xrgb, rgba_to_xrgb_and_dim};
use crate::history_browser::{HistoryPanelAction, HistoryPanelKey};
use crate::history_export::{
    HistoryExportCandidate, cleanup_history_tombstones, clear_history_entries,
    delete_history_entry, history_candidate_for_export, load_history_index,
    record_history_candidate,
};
use crate::history_load_job::{
    HistoryLoadCompletion, HistoryLoadInput, HistoryLoadJobService, HistoryLoadPayload,
    HistoryLoadPreparation,
};
use crate::history_store::{HistoryStore, default_history_path};
use crate::history_window::HistoryWindow;
use crate::hotkey::GlobalHotkeyHub;
use crate::ocr::tesseract_available;
use crate::ocr_job::{OcrJobCompletion, OcrJobService};
use crate::overlay_preview_cache::OverlayPreviewCache;
use crate::overlay_toolbar::{
    ToolbarAction, ToolbarButton, hit_test as toolbar_hit, layout_toolbar, paint_toolbar,
    toolbar_bounds,
};
use crate::settings_panel::{SettingsPanelAction, SettingsPanelKey};
use crate::settings_window::SettingsWindow;
use crate::tray::{AppTray, TrayAction};
use crate::window_policy::{self, AuxiliaryWindowKind};
use pinora_core::{
    ActionId, AnnotateSession, AnnotateTool, AnnotationRevision, AssetGeneration, AssetRef,
    CaptureImage, CaptureProvider, CaptureRequest, CaptureWindowInfo, Command, CorrelationId,
    DisplayId, DisplayInfo, DomainEventKind, ErrorCode, HistoryEntry, HistoryIndex, ImageId,
    ImageSink, JobId, JobKind, JobOwner, JobSpec, OcrResult, OcrTextSelection, OcrWordRef, PinId,
    PinTransform, PinoraError, PixelPoint, PixelRect, SelectionSession, SessionId,
    bake_annotations, color_to_hex, render_preview_rgba, resolve_all_displays_rect, sample_rgba_at,
};
use softbuffer::{Context, Rect as DamageRect, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId, WindowLevel};

use crate::pin_context_menu::{self, PinContextMenu, PinMenuAction};
use crate::pin_layout::scaled_window_size;
use crate::platform::CapabilityProbe;
use crate::runtime::AppRuntime;
use crate::single_instance::SingleInstance;

const MIN_FRAME_INTERVAL: Duration = Duration::from_micros(16_666);
const OCR_JOB_TIMEOUT_MS: u64 = 30_000;
const EXPORT_JOB_TIMEOUT_MS: u64 = 30_000;
const HISTORY_LOAD_TIMEOUT_MS: u64 = 30_000;
const HISTORY_MAX_BYTES: u64 = u64::MAX;

fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn pending_asset_for_owner(
    pending_assets: &HashMap<JobId, (JobOwner, AssetRef)>,
    job_id: JobId,
    owner: JobOwner,
) -> Option<AssetRef> {
    pending_assets
        .get(&job_id)
        .and_then(|(pending_owner, asset)| (*pending_owner == owner).then_some(*asset))
}

fn snapshot_visible_ids<T: Copy>(items: impl IntoIterator<Item = (T, bool)>) -> Vec<T> {
    items
        .into_iter()
        .filter_map(|(id, visible)| visible.then_some(id))
        .collect()
}

/// 已确认 Overlay 选区的派生图像身份。
///
/// 选区内标注只改变 generation；重选来源像素时才生成新的图像身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OverlayAssetIdentity {
    image_id: ImageId,
}

impl OverlayAssetIdentity {
    fn new() -> Self {
        Self {
            image_id: ImageId::new(),
        }
    }

    fn current(self, revision: AnnotationRevision) -> AssetRef {
        let generation = AssetGeneration::from_raw(revision.raw())
            .expect("annotation revision is guaranteed non-zero");
        AssetRef::new(self.image_id, generation)
    }

    fn stamp(self, image: &mut CaptureImage) {
        image.id = self.image_id;
    }
}

fn overlay_current_asset(overlay: &OverlayState) -> Option<AssetRef> {
    overlay
        .annotation_asset
        .map(|identity| identity.current(overlay.annotate.doc.revision()))
}

fn require_tray(result: Result<AppTray, String>) -> Result<AppTray, PinoraError> {
    result.map_err(|error| {
        PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            format!("system tray is required for tray-only mode: {error}"),
        )
    })
}

/// 运行统一桌面 shell（阻塞直到退出）。
pub fn run_desktop_shell<L, P, C, S>(runtime: AppRuntime<L, P, C, S>) -> Result<(), PinoraError>
where
    L: SingleInstance + 'static,
    P: CapabilityProbe + 'static,
    C: CaptureProvider + Clone + Send + 'static,
    S: ImageSink + 'static,
{
    let event_loop = window_policy::auxiliary_event_loop()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop event loop: {e}")))?;

    let hotkeys = GlobalHotkeyHub::start();
    for note in &hotkeys.status().notes {
        println!("pinora: hotkey: {note}");
    }
    if hotkeys.status().available {
        println!(
            "pinora: global hotkeys: F2 / Ctrl+N / Ctrl+Shift+S → region, F3 → full display when registered"
        );
    } else {
        println!("pinora: global hotkeys unavailable — use window focus keys or `pinora capture`");
    }

    let tray_displays = match runtime.capture_provider().displays() {
        Ok(displays) => displays,
        Err(error) => {
            eprintln!("pinora: tray display enumeration failed: {error}");
            Vec::new()
        }
    };
    let tray_windows = match runtime.capture_provider().windows() {
        Ok(windows) => windows,
        Err(error) => {
            eprintln!("pinora: tray window capture unavailable ({})", error.code);
            Vec::new()
        }
    };
    let tray = require_tray(AppTray::try_new(&tray_displays, &tray_windows))?;
    println!("pinora: system tray ready (click / menu → capture)");

    // 后台预截屏：空闲时持续备帧，F2 时 overlay 瞬时弹出
    let provider = runtime.capture_provider().clone();
    let frame_cache = FrameCache::start(provider);
    println!("pinora: frame-cache started (pre-capture for instant overlay)");
    let settings = runtime.settings();
    let default_pin_opacity = opacity_from_settings_percent(settings.default_pin_opacity_percent);
    let history_store = HistoryStore::new(
        default_history_path(),
        usize::try_from(settings.history_limit).expect("history limit fits usize"),
        HISTORY_MAX_BYTES,
    );
    let history_index = match load_history_index(&history_store) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("pinora: history index invalid ({error}); using empty in-memory index");
            history_store.empty_index()
        }
    };
    println!(
        "pinora: settings policy history-limit={} pin-limit={} default-opacity={}%; theme rendering unavailable",
        settings.history_limit, settings.pin_limit, settings.default_pin_opacity_percent
    );

    let mut app = DesktopApp {
        runtime: Some(runtime),
        context: None,
        // 常驻时只保留托盘入口。FrameCache 仍在后台预热，但绝不因启动而弹出窗口。
        mode: Mode::Idle,
        loading: None,
        delayed_capture: None,
        overlay: None,
        settings: None,
        history: None,
        pins: HashMap::new(),
        drag_pin: None,
        modifiers: ModifiersState::empty(),
        error: None,
        quit: false,
        pending_messages: Vec::new(),
        hotkeys,
        frame_cache: Some(frame_cache),
        ocr_jobs: OcrJobService::new(),
        export_jobs: ExportJobService::new(),
        history_load_jobs: HistoryLoadJobService::new(),
        pending_exports: HashMap::new(),
        active_history_load: None,
        queued_history_load: None,
        history_store,
        history_index,
        start_capture_wait: None,
        capture_mode: CaptureMode::Region,
        capture_target: CaptureTarget::DefaultLargest,
        tray: Some(tray),
        default_pin_opacity,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop loop: {e}")))?;

    // 事件循环无论因何结束，都先恢复倒计时开始时由 Pinora 隐藏的贴图。
    app.cancel_delayed_capture();

    let ocr_shutdown = app.ocr_jobs.cancel_all_and_wait(Duration::from_secs(2));
    println!(
        "pinora: OCR shutdown cancelled={} joined={} panicked={} unfinished={}",
        ocr_shutdown.cancelled, ocr_shutdown.joined, ocr_shutdown.panicked, ocr_shutdown.unfinished
    );
    let export_shutdown = app.export_jobs.cancel_all_and_wait(Duration::from_secs(2));
    println!(
        "pinora: export shutdown cancelled={} joined={} panicked={} unfinished={}",
        export_shutdown.cancelled,
        export_shutdown.joined,
        export_shutdown.panicked,
        export_shutdown.unfinished
    );
    let history_load_shutdown = app
        .history_load_jobs
        .cancel_all_and_wait(Duration::from_secs(2));
    println!(
        "pinora: history load shutdown cancelled={} joined={} panicked={} unfinished={}",
        history_load_shutdown.cancelled,
        history_load_shutdown.joined,
        history_load_shutdown.panicked,
        history_load_shutdown.unfinished
    );
    if let Some(err) = app.error {
        return Err(err);
    }

    if let Some(mut rt) = app.runtime.take() {
        let _ = rt.dispatch(Command::shutdown());
        println!(
            "pinora: shutdown complete (pins={})",
            rt.state().pin_count()
        );
    }
    Ok(())
}

enum Mode {
    /// 下一帧启动：后台截屏（无全屏遮罩，避免截到自己）。
    StartCapture,
    /// 正在后台截屏，显示小加载窗。
    LoadingCapture,
    /// tray 发起的无窗口倒计时；到期后只能走冷捕获。
    DelayedCapture,
    /// 空闲：仅贴图窗口。
    Idle,
}

/// 新屏幕捕获会话的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureMode {
    Region,
    FullDisplay,
    AllDisplays,
    Window,
}

/// 捕获会话目标。窗口快照必须在实际捕获前由后端重新验证。
#[derive(Clone, PartialEq)]
enum CaptureTarget {
    DefaultLargest,
    Display(DisplayId),
    AllDisplays,
    Window(CaptureWindowInfo),
}

/// `LoadingState` 失败时必须采用的恢复路径。延时会话优先，因为它拥有需要恢复的
/// 贴图可见性快照；正常和窗口截图不应以失败退出 tray 主循环。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureFailureScope {
    Standard,
    Window,
    Delayed,
}

fn capture_failure_scope(target: &CaptureTarget, delayed_active: bool) -> CaptureFailureScope {
    if delayed_active {
        CaptureFailureScope::Delayed
    } else if matches!(target, CaptureTarget::Window(_)) {
        CaptureFailureScope::Window
    } else {
        CaptureFailureScope::Standard
    }
}

impl CaptureMode {
    fn label(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::FullDisplay => "full-display",
            Self::AllDisplays => "all-displays",
            Self::Window => "window",
        }
    }
}

impl CaptureTarget {
    fn log_label(&self) -> &'static str {
        match self {
            Self::DefaultLargest => "default-display",
            Self::Display(_) => "selected-display",
            Self::AllDisplays => "all-displays",
            Self::Window(_) => "selected-window",
        }
    }
}

/// Overlay 打开时的初始选区，不等同于图像如何取得。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayInitialSelection {
    Manual,
    FullImage,
}

fn initial_selection_for_capture(capture_mode: CaptureMode) -> OverlayInitialSelection {
    match capture_mode {
        CaptureMode::Region => OverlayInitialSelection::Manual,
        CaptureMode::FullDisplay | CaptureMode::AllDisplays | CaptureMode::Window => {
            OverlayInitialSelection::FullImage
        }
    }
}

fn resolve_capture_target(
    displays: &[DisplayInfo],
    target: &CaptureTarget,
) -> Result<DisplayInfo, PinoraError> {
    match target {
        CaptureTarget::DefaultLargest => displays
            .iter()
            .max_by_key(|display| display.bounds.size.area())
            .cloned()
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, "no display for capture")),
        CaptureTarget::Display(display_id) => displays
            .iter()
            .find(|display| &display.id == display_id)
            .cloned()
            .ok_or_else(|| {
                PinoraError::new(
                    ErrorCode::NotFound,
                    format!("selected display is no longer available: {}", display_id.0),
                )
            }),
        CaptureTarget::AllDisplays | CaptureTarget::Window(_) => Err(PinoraError::new(
            ErrorCode::InvalidState,
            "non-display capture target cannot be resolved as a display",
        )),
    }
}

fn apply_initial_selection(
    session: &mut SelectionSession,
    initial_selection: OverlayInitialSelection,
) -> Result<Option<PixelRect>, PinoraError> {
    match initial_selection {
        OverlayInitialSelection::Manual => Ok(None),
        OverlayInitialSelection::FullImage => session.select_all().map(Some),
    }
}

/// 后台线程准备好的全屏预览（原图像素 + 暗化底图）。
struct PreparedPreview {
    image: CaptureImage,
    base: Vec<u32>,
    dimmed: Vec<u32>,
}

fn prepare_preview(image: CaptureImage) -> PreparedPreview {
    let (base, dimmed) = rgba_to_xrgb_and_dim(&image.pixels.bytes);
    PreparedPreview {
        image,
        base,
        dimmed,
    }
}

fn preview_buffers_match_image(preview: &PreparedPreview) -> bool {
    let Some(expected_len) = usize::try_from(preview.image.pixels.size.area()).ok() else {
        return false;
    };
    preview.base.len() == expected_len && preview.dimmed.len() == expected_len
}

/// Overlay 的窗口呈现方式。历史编辑不能假装当前桌面仍是原始全屏捕获。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayPresentation {
    ScreenCapture,
    VirtualDesktop,
    WindowCapture,
    HistoryEditor,
    PinEditor,
}

/// 截屏中：后台抓当前屏（无全屏遮罩，避免截到自己）；完成后立刻开真实 overlay。
struct LoadingState {
    // 后台捕获错误只跨线程传递稳定错误码，避免平台后端文本泄露窗口身份或标题。
    preview_rx: Receiver<Result<PreparedPreview, ErrorCode>>,
    target: OverlayTarget,
}

/// 延时区域截图的清理所有者。
///
/// 快照只保存倒计时开始时由 Pinora 确认可见的贴图窗口；恢复时已经关闭的窗口
/// 会被忽略，因此不会复活用户已经关闭的贴图。
struct DelayedCapture {
    deadline: Instant,
    hidden_pin_ids: Vec<WindowId>,
}

impl DelayedCapture {
    fn new(delay: Duration, hidden_pin_ids: Vec<WindowId>) -> Self {
        Self {
            deadline: Instant::now() + delay,
            hidden_pin_ids,
        }
    }

    fn is_due(&self, now: Instant) -> bool {
        now >= self.deadline
    }
}

/// 打开 Overlay 所需的捕获来源与初始交互意图。
struct OverlayTarget {
    display_id: DisplayId,
    display_origin: PixelPoint,
    image_width: u32,
    image_height: u32,
    initial_selection: OverlayInitialSelection,
    presentation: OverlayPresentation,
    min_selection_edge: u32,
    edit_pin_id: Option<PinId>,
}

fn history_edit_target(image: &CaptureImage) -> OverlayTarget {
    OverlayTarget {
        display_id: image.metadata.display.clone(),
        // 输出保持历史图像原始来源坐标；窗口位置不假定旧显示器仍存在。
        display_origin: image.source_rect.origin,
        image_width: image.pixels.size.width,
        image_height: image.pixels.size.height,
        initial_selection: OverlayInitialSelection::FullImage,
        presentation: OverlayPresentation::HistoryEditor,
        min_selection_edge: 1,
        edit_pin_id: None,
    }
}

fn window_capture_overlay_target(window: &CaptureWindowInfo) -> OverlayTarget {
    OverlayTarget {
        display_id: window.display.clone(),
        display_origin: window.bounds.origin,
        image_width: window.bounds.size.width,
        image_height: window.bounds.size.height,
        initial_selection: OverlayInitialSelection::FullImage,
        presentation: OverlayPresentation::WindowCapture,
        min_selection_edge: 1,
        edit_pin_id: None,
    }
}

fn pin_edit_target(image: &CaptureImage, pin_id: PinId) -> OverlayTarget {
    OverlayTarget {
        display_id: image.metadata.display.clone(),
        display_origin: image.source_rect.origin,
        image_width: image.pixels.size.width,
        image_height: image.pixels.size.height,
        initial_selection: OverlayInitialSelection::FullImage,
        presentation: OverlayPresentation::PinEditor,
        min_selection_edge: 1,
        edit_pin_id: Some(pin_id),
    }
}

/// Overlay 内阶段：框选中 / 已出选区（工具栏就绪）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayPhase {
    Selecting,
    Ready,
}

/// 选区完成后的收尾动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayFinish {
    Copy,
    Pin,
    Save,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnnotationHistoryAction {
    Undo,
    Redo,
}

fn annotation_history_action(
    control_pressed: bool,
    shift_pressed: bool,
    character: &str,
) -> Option<AnnotationHistoryAction> {
    if !control_pressed {
        return None;
    }
    match character {
        "z" | "Z" if shift_pressed => Some(AnnotationHistoryAction::Redo),
        "z" | "Z" => Some(AnnotationHistoryAction::Undo),
        "y" | "Y" => Some(AnnotationHistoryAction::Redo),
        _ => None,
    }
}

#[derive(Debug)]
enum PendingExportAction {
    SavePng(PathBuf),
    CopyImage,
    CopyText,
}

#[derive(Debug)]
struct PendingExport {
    owner: JobOwner,
    asset: AssetRef,
    action: PendingExportAction,
    history: Option<HistoryExportCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryLoadIntent {
    Preview,
    Reopen,
    Edit,
}

#[derive(Debug, Clone)]
struct HistoryLoadRequest {
    entry: HistoryEntry,
    intent: HistoryLoadIntent,
}

#[derive(Debug, Clone)]
struct ActiveHistoryLoad {
    job_id: JobId,
    request: HistoryLoadRequest,
}

fn current_history_load_asset(
    active: Option<&ActiveHistoryLoad>,
    selected: Option<&HistoryEntry>,
    job_id: JobId,
    owner: JobOwner,
) -> Option<AssetRef> {
    let active = active?;
    let selected = selected?;
    (active.job_id == job_id
        && owner == JobOwner::History(active.request.entry.image_id)
        && selected.image_id == active.request.entry.image_id
        && selected.generation == active.request.entry.generation)
        .then_some(AssetRef::new(selected.image_id, selected.generation))
}

struct OverlayState {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    /// 选区外：真实桌面暗化（与截图 1:1 像素）。
    dimmed: Vec<u32>,
    /// 选区内：真实桌面原图。
    base: Vec<u32>,
    frame: Vec<u32>,
    session: SelectionSession,
    phase: OverlayPhase,
    dragging: bool,
    /// Ready 下按下后尚未移动足够距离，暂不退出 Ready。
    pending_reselect: bool,
    drag_anchor: PixelPoint,
    /// 在选区内画标注。
    annotate_dragging: bool,
    annotate: AnnotateSession,
    /// 进入取色器前的绘图工具；采样后立即恢复，避免下次点击误进入取色模式。
    last_drawing_tool: AnnotateTool,
    /// 标注预览缓存（选区在缓冲分辨率下的 XRGB）。
    annotate_cache: Option<Vec<u32>>,
    annotate_cache_wh: (u32, u32),
    /// 当前选区的原始裁剪与已提交标注层；仅 Overlay 生命周期内有效。
    annotate_preview_cache: OverlayPreviewCache,
    annotate_dirty: bool,
    toolbar: Vec<ToolbarButton>,
    /// 按下工具栏按钮，抬起时若仍命中则触发。
    toolbar_pressed: Option<ToolbarAction>,
    last_toolbar_bounds: Option<PixelRect>,
    /// 仅工具栏外观变化（高亮），不重烤选区。
    toolbar_chrome_dirty: bool,
    /// softbuffer 是否已与 frame 全量同步过（之后只传脏区）。
    buffer_synced: bool,
    last_cursor: PixelPoint,
    needs_redraw: bool,
    last_drawn_rect: Option<PixelRect>,
    last_present: Instant,
    /// 拖选时节流：上次真正重绘时的光标。
    last_draw_cursor: PixelPoint,
    /// 双击检测。
    last_click_at: Option<Instant>,
    last_click_pos: PixelPoint,
    /// 原图像素尺寸（裁剪/导出用）。
    src_w: u32,
    src_h: u32,
    /// softbuffer / 选区坐标尺寸（与 src 1:1）。
    buf_w: u32,
    buf_h: u32,
    /// Ready 时对应的原图选区（导出/标注坐标系）。
    active_src_rect: Option<PixelRect>,
    /// 窗口客户区（鼠标映射）。
    win_w: u32,
    win_h: u32,
    display_id: DisplayId,
    display_origin: PixelPoint,
    full_image: CaptureImage,
    session_id: SessionId,
    /// 当前确认选区的派生图像身份；重选后必须更换。
    annotation_asset: Option<OverlayAssetIdentity>,
    /// 编辑既有贴图时保留其稳定领域身份；取消必须恢复原窗口。
    edit_pin_id: Option<PinId>,
}

struct PinWin {
    pin_id: PinId,
    title: String,
    image: CaptureImage,
    asset: AssetRef,
    pixels_xrgb: Vec<u32>,
    render_cache: Option<PinRenderCache>,
    scale: f64,
    opacity: f64,
    locked: bool,
    always_on_top: bool,
    context_menu: Option<PinContextMenu>,
    /// winit 没有跨平台的实际可见性查询；这是 Pinora 对自己贴图可见状态的事实记录。
    visible: bool,
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    /// 最近一次 OCR 结果。
    ocr: Option<OcrResult>,
    /// 是否绘制 OCR 词框。
    ocr_show_boxes: bool,
    /// 最近一次窗口内光标位置（物理像素）。
    cursor_position: (f64, f64),
    /// Ctrl+左键拖选的起点（物理像素）。
    ocr_drag_start: Option<(f64, f64)>,
    /// 当前选中的 OCR 词。
    ocr_selection: OcrTextSelection,
}

/// 贴图基础帧缓存：不包含 OCR、拖选或锁定边框等每帧变化的叠加层。
struct PinRenderCache {
    width: u32,
    height: u32,
    opacity_factor: u32,
    pixels: Vec<u32>,
}

impl PinRenderCache {
    fn matches(&self, width: u32, height: u32, opacity: f64) -> bool {
        self.width == width
            && self.height == height
            && self.opacity_factor == opacity_factor(opacity)
    }
}

/// 创建贴图窗口所需的呈现参数。预处理像素仅由受监督的历史读取 worker 提供。
struct PinPresentation {
    position: PixelPoint,
    scale: f64,
    opacity: f64,
    pixels_xrgb: Option<Vec<u32>>,
}

struct DesktopApp<L, P, C, S> {
    runtime: Option<AppRuntime<L, P, C, S>>,
    context: Option<Context<Rc<Window>>>,
    mode: Mode,
    loading: Option<LoadingState>,
    delayed_capture: Option<DelayedCapture>,
    overlay: Option<OverlayState>,
    /// 显式设置窗口；草稿只在保存成功后应用到 runtime。
    settings: Option<SettingsWindow>,
    /// 受管历史浏览窗口；文件读取和删除必须经 history_export 安全边界。
    history: Option<HistoryWindow>,
    pins: HashMap<WindowId, PinWin>,
    drag_pin: Option<WindowId>,
    modifiers: ModifiersState,
    error: Option<PinoraError>,
    quit: bool,
    pending_messages: Vec<String>,
    hotkeys: GlobalHotkeyHub,
    frame_cache: Option<FrameCache>,
    ocr_jobs: OcrJobService,
    export_jobs: ExportJobService,
    history_load_jobs: HistoryLoadJobService,
    pending_exports: HashMap<JobId, PendingExport>,
    active_history_load: Option<ActiveHistoryLoad>,
    queued_history_load: Option<HistoryLoadRequest>,
    history_store: HistoryStore,
    history_index: HistoryIndex,
    /// 等待 frame-cache 首帧的起始时间；超时走 cold path。
    start_capture_wait: Option<Instant>,
    /// 当前会话在 Overlay 打开后的初始选区；cold capture 必须保持这一意图。
    capture_mode: CaptureMode,
    /// 本次截图的显示器目标；显式目标不能隐式降级为默认屏幕。
    capture_target: CaptureTarget,
    tray: Option<AppTray>,
    /// 设置驱动的新建贴图默认不透明度；运行时手动调整后不再覆盖。
    default_pin_opacity: f64,
}

impl<L, P, C, S> ApplicationHandler for DesktopApp<L, P, C, S>
where
    L: SingleInstance,
    P: CapabilityProbe,
    C: CaptureProvider + Clone + Send + 'static,
    S: ImageSink,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_context(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 托盘菜单（先收集动作，避免与 self 可变借用冲突）
        let mut tray_actions = Vec::new();
        if let Some(tray) = &self.tray {
            while let Some(action) = tray.poll() {
                tray_actions.push(action);
            }
        }
        for action in tray_actions {
            if self.delayed_capture.is_some()
                && !matches!(action, TrayAction::CancelDelayedCapture | TrayAction::Quit)
            {
                println!("pinora: tray action ignored while delayed capture is active");
                continue;
            }
            match action {
                TrayAction::Capture => {
                    println!("pinora: tray → capture");
                    self.request_new_capture(event_loop);
                }
                TrayAction::CaptureRegionAfter(delay) => {
                    self.request_delayed_region_capture(delay);
                }
                TrayAction::CancelDelayedCapture => {
                    if self.cancel_delayed_capture() {
                        println!("pinora: tray → delayed capture cancelled");
                    }
                }
                TrayAction::CaptureFullDisplay => {
                    println!("pinora: tray → full-display capture");
                    self.request_full_display_capture(event_loop);
                }
                TrayAction::CaptureAllDisplays => {
                    println!("pinora: tray → all-displays capture");
                    self.request_all_displays_capture(event_loop);
                }
                TrayAction::CaptureDisplay(display_id) => {
                    println!("pinora: tray → display capture ({})", display_id.0);
                    self.request_display_capture(event_loop, display_id);
                }
                TrayAction::CaptureWindow(target) => {
                    println!("pinora: tray → window capture");
                    self.request_window_capture(event_loop, target);
                }
                TrayAction::Settings => {
                    println!("pinora: tray → settings");
                    if let Err(error) = self.open_settings(event_loop) {
                        self.error = Some(error);
                    }
                }
                TrayAction::History => {
                    println!("pinora: tray → history");
                    if let Err(error) = self.open_history(event_loop) {
                        self.error = Some(error);
                    }
                }
                TrayAction::ShowAllPins => {
                    println!("pinora: tray → show all pins");
                    self.set_all_pins_visible(true);
                }
                TrayAction::HideAllPins => {
                    println!("pinora: tray → hide all pins");
                    self.set_all_pins_visible(false);
                }
                TrayAction::CloseAllPins => {
                    println!("pinora: tray → close all pins");
                    self.close_all_pins();
                }
                TrayAction::Quit => {
                    println!("pinora: tray → quit");
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
            }
        }

        // 单实例 socket 转发 + 全局热键
        self.poll_external_actions(event_loop);
        self.poll_ocr_jobs();
        self.poll_export_jobs();
        self.poll_history_load_jobs(event_loop);

        for msg in self.pending_messages.drain(..) {
            println!("{msg}");
        }

        if self.quit {
            event_loop.exit();
            return;
        }

        if matches!(self.mode, Mode::DelayedCapture) {
            self.poll_delayed_capture(event_loop);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(30),
            ));
            return;
        }

        // 后台截屏完成 → 打开真实桌面遮罩
        if matches!(self.mode, Mode::LoadingCapture) {
            self.poll_loading_to_overlay(event_loop);
            if self.loading.is_some() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(30),
                ));
                return;
            }
        }

        // Overlay 帧合并
        if let Some(ov) = self.overlay.as_mut()
            && ov.needs_redraw
        {
            let elapsed = ov.last_present.elapsed();
            if elapsed >= MIN_FRAME_INTERVAL {
                ov.needs_redraw = false;
                ov.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + (MIN_FRAME_INTERVAL - elapsed),
                ));
            }
            return;
        }

        // 启动/再截：优先等 frame-cache 出帧再弹 overlay（瞬时）
        if matches!(self.mode, Mode::StartCapture)
            && self.overlay.is_none()
            && self.loading.is_none()
        {
            self.try_start_capture(event_loop);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(30),
            ));
            return;
        }

        // 短周期唤醒，以便轮询托盘、全局热键与单实例 socket。
        // 未能注册全局热键的 Wayland 会话仍可使用托盘或 IPC，不能靠常驻窗口抢焦点。
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(50),
        ));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(history) = self.history.as_ref()
            && history.window_id() == window_id
        {
            self.handle_history_event(event_loop, event);
            return;
        }
        if let Some(settings) = self.settings.as_ref()
            && settings.window_id() == window_id
        {
            self.handle_settings_event(event_loop, event);
            return;
        }
        if let Some(ov) = self.overlay.as_ref()
            && ov.window.id() == window_id
        {
            self.handle_overlay_event(event_loop, event);
            return;
        }
        self.handle_pin_event(event_loop, window_id, event);
    }
}

impl<L, P, C, S> DesktopApp<L, P, C, S>
where
    L: SingleInstance,
    P: CapabilityProbe,
    C: CaptureProvider + Clone + Send + 'static,
    S: ImageSink,
{
    fn set_all_pins_visible(&mut self, visible: bool) {
        let window_ids: Vec<_> = self.pins.keys().copied().collect();
        self.set_pins_visible(&window_ids, visible);
        if visible && let Some(pin) = self.pins.values().next() {
            pin.window.focus_window();
        }
    }

    fn set_pins_visible(&mut self, window_ids: &[WindowId], visible: bool) {
        for window_id in window_ids {
            if let Some(pin) = self.pins.get_mut(window_id) {
                if visible {
                    window_policy::show_auxiliary_window(
                        AuxiliaryWindowKind::Pin,
                        &pin.window,
                        &pin.title,
                    );
                } else {
                    pin.window.set_visible(false);
                }
                pin.visible = visible;
            }
        }
    }

    fn snapshot_visible_pin_ids(&self) -> Vec<WindowId> {
        snapshot_visible_ids(
            self.pins
                .iter()
                .map(|(window_id, pin)| (*window_id, pin.visible)),
        )
    }

    fn set_delayed_capture_tray_state(&self, active: bool) {
        if let Some(tray) = &self.tray {
            tray.set_delayed_capture_active(active);
        }
    }

    fn restore_delayed_pins(&mut self) -> bool {
        let Some(delayed) = self.delayed_capture.take() else {
            return false;
        };
        let restored = delayed.hidden_pin_ids.len();
        self.set_pins_visible(&delayed.hidden_pin_ids, true);
        self.set_delayed_capture_tray_state(false);
        if restored > 0 {
            println!("pinora: restored {restored} delayed-capture pin(s)");
        }
        true
    }

    /// 取消倒计时或已经开始的冷捕获。CaptureProvider 没有可移植的强制取消接口，
    /// 因此已开始的 worker 会自然结束，但其结果接收端被丢弃，绝不会打开 Overlay。
    fn cancel_delayed_capture(&mut self) -> bool {
        if self.delayed_capture.is_none() {
            return false;
        }
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.start_capture_wait = None;
        let restored = self.restore_delayed_pins();
        self.resume_frame_cache();
        restored
    }

    fn close_all_pins(&mut self) {
        let window_ids: Vec<_> = self.pins.keys().copied().collect();
        for window_id in window_ids {
            self.close_pin(window_id);
        }
    }

    fn ensure_context(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_some() {
            return;
        }
        // 先建一个隐藏、跳过任务栏的占位窗以拿到 display handle（Wayland 需要）。
        let attrs = Window::default_attributes()
            .with_visible(false)
            .with_title("pinora-display-handle");
        if let Ok(w) = window_policy::create_auxiliary_window(
            event_loop,
            AuxiliaryWindowKind::DisplayHandle,
            attrs,
        ) {
            let w = Rc::new(w);
            if let Ok(ctx) = Context::new(w) {
                self.context = Some(ctx);
            }
        }
    }

    /// 处理全局热键与 `pinora capture` 等 IPC。
    fn poll_external_actions(&mut self, event_loop: &ActiveEventLoop) {
        // 1) 全局热键
        for action in self.hotkeys.poll_actions() {
            match action {
                ActionId::CaptureRegionAndPin => {
                    println!("pinora: global hotkey → capture");
                    self.request_new_capture(event_loop);
                }
                ActionId::CaptureFullDisplay => {
                    println!("pinora: global hotkey → full-display capture");
                    self.request_full_display_capture(event_loop);
                }
                ActionId::Quit => {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                _ => {}
            }
        }

        // 2) 单实例 socket — 先取出命令再处理，避免与 self 可变借用冲突
        let commands = match self.runtime.as_mut() {
            Some(rt) => match rt.take_forwarded() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("pinora: take_forwarded: {e}");
                    return;
                }
            },
            None => return,
        };
        if commands.is_empty() {
            return;
        }

        let mut need_capture = false;
        for command in commands {
            match command {
                Command::InvokeAction {
                    action: ActionId::CaptureRegionAndPin,
                    ..
                } => {
                    println!("pinora: ipc CAPTURE → capture");
                    need_capture = true;
                }
                Command::Shutdown { .. } => {
                    println!("pinora: ipc QUIT");
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                Command::Activate { .. } => {
                    if let Some(rt) = self.runtime.as_mut()
                        && rt.dispatch(command).is_ok()
                    {
                        self.pending_messages.push(format!(
                            "pinora: activated (count={})",
                            rt.state().activation_count
                        ));
                    }
                    need_capture = true;
                }
                other => {
                    if let Some(rt) = self.runtime.as_mut()
                        && let Err(e) = rt.dispatch(other)
                    {
                        eprintln!("pinora: forwarded command failed: {e}");
                    }
                }
            }
        }
        if need_capture {
            self.request_new_capture(event_loop);
        }
    }

    /// 若有缓存帧则开 overlay；若缓存还在暖机则跳过（由 about_to_wait 再试）。
    fn try_start_capture(&mut self, event_loop: &ActiveEventLoop) {
        if !matches!(self.mode, Mode::StartCapture)
            || self.overlay.is_some()
            || self.loading.is_some()
        {
            self.start_capture_wait = None;
            return;
        }
        let cache_ready = match &self.capture_target {
            CaptureTarget::DefaultLargest => {
                self.frame_cache.as_ref().is_some_and(FrameCache::is_ready)
            }
            CaptureTarget::Display(_) | CaptureTarget::AllDisplays | CaptureTarget::Window(_) => {
                true
            }
        };
        if !cache_ready {
            if let Some(cache) = &self.frame_cache {
                cache.resume();
            }
            let waited = self
                .start_capture_wait
                .get_or_insert_with(Instant::now)
                .elapsed();
            // 首帧超时才 cold，避免永远卡住
            if waited < Duration::from_secs(3) {
                return;
            }
            println!("pinora: frame-cache timeout, cold capture…");
        }
        self.start_capture_wait = None;
        if let Err(e) = self.begin_screen_grab(event_loop, true) {
            self.handle_capture_start_error(e);
        }
    }

    /// 弹出选区 overlay：优先用后台预截帧（瞬时），否则再等一次截屏。
    fn begin_screen_grab(
        &mut self,
        event_loop: &ActiveEventLoop,
        allow_cached_frame: bool,
    ) -> Result<(), PinoraError> {
        let capture_mode = self.capture_mode;
        let initial_selection = initial_selection_for_capture(capture_mode);
        let explicit_display = match &self.capture_target {
            CaptureTarget::DefaultLargest => None,
            CaptureTarget::Display(_) => {
                let runtime = self.runtime.as_ref().ok_or_else(|| {
                    PinoraError::new(ErrorCode::InvalidState, "capture runtime is unavailable")
                })?;
                let displays = runtime.capture_provider().displays()?;
                Some(resolve_capture_target(&displays, &self.capture_target)?)
            }
            CaptureTarget::AllDisplays | CaptureTarget::Window(_) => None,
        };
        // 1) 缓存命中 → 立刻开 overlay（目标 < 16ms）
        // 允许最多 2s 龄的帧；后台约每 0.5s 刷新一轮
        let cached_frame = allow_cached_frame
            .then(|| {
                if matches!(
                    self.capture_target,
                    CaptureTarget::AllDisplays | CaptureTarget::Window(_)
                ) {
                    return None;
                }
                self.frame_cache.as_ref().and_then(|cache| {
                    if let Some(display) = explicit_display.as_ref() {
                        cache
                            .take_for_display_if_fresh(display, Duration::from_secs(2))
                            .or_else(|| cache.take_for_display(display))
                    } else {
                        cache
                            .take_if_fresh(Duration::from_secs(2))
                            .or_else(|| cache.take_any())
                    }
                })
            })
            .flatten();
        // 截屏/选区期间暂停预截，避免截到自己的窗。暂停会清空任何竞态晚到帧。
        if let Some(cache) = &self.frame_cache {
            cache.pause();
        }

        if let Some(frame) = cached_frame {
            let age_ms = frame.age().as_secs_f64() * 1000.0;
            println!(
                "pinora: overlay INSTANT from cache (age {:.0}ms, {}x{})",
                age_ms, frame.image.pixels.size.width, frame.image.pixels.size.height
            );
            let prep = PreparedPreview {
                image: frame.image,
                base: frame.base,
                dimmed: frame.dimmed,
            };
            let img_w = prep.image.pixels.size.width.max(1);
            let img_h = prep.image.pixels.size.height.max(1);
            return self.open_overlay_with_preview(
                event_loop,
                prep,
                OverlayTarget {
                    display_id: frame.display_id,
                    display_origin: frame.display_origin,
                    image_width: img_w,
                    image_height: img_h,
                    initial_selection,
                    presentation: OverlayPresentation::ScreenCapture,
                    min_selection_edge: 2,
                    edit_pin_id: None,
                },
            );
        }

        // 2) 缓存未就绪或窗口目标：后台冷捕获。窗口绝不消费显示器预截帧。
        let rt = self.runtime.as_ref().unwrap();
        let (request, target, log_target) = match &self.capture_target {
            CaptureTarget::AllDisplays => {
                let displays = rt.capture_provider().displays()?;
                let workspace = resolve_all_displays_rect(&displays)?;
                (
                    CaptureRequest::AllDisplays,
                    OverlayTarget {
                        display_id: DisplayId::virtual_desktop(),
                        display_origin: workspace.origin,
                        image_width: workspace.size.width,
                        image_height: workspace.size.height,
                        initial_selection,
                        presentation: OverlayPresentation::VirtualDesktop,
                        min_selection_edge: 2,
                        edit_pin_id: None,
                    },
                    "all displays".to_owned(),
                )
            }
            CaptureTarget::Window(window) => (
                CaptureRequest::Window {
                    target: window.clone(),
                },
                window_capture_overlay_target(window),
                "selected window".to_owned(),
            ),
            CaptureTarget::DefaultLargest | CaptureTarget::Display(_) => {
                let display = match explicit_display {
                    Some(display) => display,
                    None => {
                        let displays = rt.capture_provider().displays()?;
                        resolve_capture_target(&displays, &self.capture_target)?
                    }
                };
                let image_width = display.bounds.size.width;
                let image_height = display.bounds.size.height;
                let display_name = display.name;
                (
                    CaptureRequest::FullDisplay {
                        display: display.id.clone(),
                    },
                    OverlayTarget {
                        display_id: display.id,
                        display_origin: display.bounds.origin,
                        image_width,
                        image_height,
                        initial_selection,
                        presentation: OverlayPresentation::ScreenCapture,
                        min_selection_edge: 2,
                        edit_pin_id: None,
                    },
                    display_name,
                )
            }
        };

        let provider = rt.capture_provider().clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let started = Instant::now();
            let result = provider
                .capture(request)
                .map(prepare_preview)
                .map_err(|error| error.code);
            println!(
                "pinora: capture done in {:.0}ms (cold path)",
                started.elapsed().as_secs_f64() * 1000.0
            );
            let _ = tx.send(result);
        });

        println!("pinora: cache miss — grabbing {log_target}…");

        self.loading = Some(LoadingState {
            preview_rx: rx,
            target,
        });
        self.mode = Mode::LoadingCapture;
        Ok(())
    }

    fn resume_frame_cache(&self) {
        if let Some(cache) = &self.frame_cache {
            cache.resume();
        }
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        if let Some(settings) = self.settings.as_ref() {
            settings.focus();
            return Ok(());
        }
        self.ensure_context(event_loop);
        let current = self
            .runtime
            .as_ref()
            .map(AppRuntime::settings)
            .unwrap_or_default();
        let settings = {
            let context = self.context.as_ref().ok_or_else(|| {
                PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
            })?;
            SettingsWindow::open(event_loop, context, current)?
        };
        settings.focus();
        settings.request_redraw();
        self.settings = Some(settings);
        println!("pinora: settings opened (arrows edit, Enter save, Esc cancel)");
        Ok(())
    }

    fn close_settings(&mut self) {
        if let Some(settings) = self.settings.take() {
            settings.close();
        }
    }

    fn handle_settings_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_settings(),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.set_cursor(PixelPoint::new(
                        position.x.round() as i32,
                        position.y.round() as i32,
                    ));
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let action = self.settings.as_ref().and_then(SettingsWindow::hit_test);
                if let Some(action) = action {
                    self.apply_settings_action(event_loop, action);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.is_quit_key(&event) {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                if self.handle_capture_shortcut(event_loop, &event) {
                    self.close_settings();
                    return;
                }
                let key = match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => Some(SettingsPanelKey::Up),
                    PhysicalKey::Code(KeyCode::ArrowDown) => Some(SettingsPanelKey::Down),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => Some(SettingsPanelKey::Left),
                    PhysicalKey::Code(KeyCode::ArrowRight) => Some(SettingsPanelKey::Right),
                    PhysicalKey::Code(KeyCode::Enter) => Some(SettingsPanelKey::Enter),
                    PhysicalKey::Code(KeyCode::Escape) => Some(SettingsPanelKey::Escape),
                    _ => None,
                };
                let Some(key) = key else { return };
                let action = self
                    .settings
                    .as_mut()
                    .and_then(|settings| settings.handle_key(key));
                if let Some(action) = action {
                    self.apply_settings_action(event_loop, action);
                } else if let Some(settings) = self.settings.as_ref() {
                    settings.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.paint_settings() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.resize(size.width, size.height);
                }
            }
            _ => {}
        }
    }

    fn apply_settings_action(
        &mut self,
        _event_loop: &ActiveEventLoop,
        action: SettingsPanelAction,
    ) {
        match action {
            SettingsPanelAction::Select(_)
            | SettingsPanelAction::Decrement
            | SettingsPanelAction::Increment => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.apply_action(action);
                    settings.request_redraw();
                }
            }
            SettingsPanelAction::Cancel => self.close_settings(),
            SettingsPanelAction::Save => self.save_settings(),
        }
    }

    fn save_settings(&mut self) {
        let Some(draft) = self.settings.as_ref().map(SettingsWindow::draft) else {
            return;
        };
        let save_result = self.settings.as_ref().map(|settings| settings.save(draft));
        match save_result {
            Some(Ok(())) => {
                if let Some(rt) = self.runtime.as_mut() {
                    rt.apply_settings(draft);
                }
                self.default_pin_opacity =
                    opacity_from_settings_percent(draft.default_pin_opacity_percent);
                let max_bytes = self.history_store.max_bytes();
                self.history_store
                    .set_limits(draft.history_limit as usize, max_bytes);
                let evicted = self
                    .history_index
                    .set_limits(draft.history_limit as usize, max_bytes);
                if !evicted.is_empty() {
                    if self.history_store.save(&self.history_index).is_err() {
                        eprintln!("pinora: history quota update deferred");
                    } else if let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir())
                        && cleanup_history_tombstones(
                            &self.history_store,
                            export_dir,
                            &mut self.history_index,
                        )
                        .is_err()
                    {
                        eprintln!("pinora: history quota cleanup deferred");
                    }
                    self.cancel_history_loads();
                    let active_entries = self.history_index.active_entries().cloned().collect();
                    if let Some(history) = self.history.as_mut() {
                        history.clear_preview();
                        history.panel_mut().replace_entries(active_entries);
                        history.request_redraw();
                    }
                    self.queue_history_load(HistoryLoadIntent::Preview);
                }
                if let Some(settings) = self.settings.as_mut() {
                    settings.mark_saved();
                    settings.request_redraw();
                }
                println!("pinora: settings saved (theme={:?})", draft.theme);
            }
            Some(Err(_)) => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.mark_save_failed("settings_save_failed");
                    settings.request_redraw();
                }
                eprintln!("pinora: settings save failed; runtime values unchanged");
            }
            None => {}
        }
    }

    fn paint_settings(&mut self) -> Result<(), PinoraError> {
        let Some(settings) = self.settings.as_mut() else {
            return Ok(());
        };
        settings.paint()
    }

    fn open_history(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        if let Some(history) = self.history.as_ref() {
            history.focus();
            return Ok(());
        }
        self.ensure_context(event_loop);
        let entries = self.history_index.active_entries().cloned().collect();
        let history = {
            let context = self.context.as_ref().ok_or_else(|| {
                PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
            })?;
            HistoryWindow::open(event_loop, context, entries)?
        };
        history.focus();
        history.request_redraw();
        self.history = Some(history);
        self.queue_history_load(HistoryLoadIntent::Preview);
        println!("pinora: history opened (Enter pin, Delete remove, Esc close)");
        Ok(())
    }

    fn close_history(&mut self) {
        self.cancel_history_loads();
        if let Some(history) = self.history.take() {
            history.close();
        }
    }

    fn queue_history_load(&mut self, intent: HistoryLoadIntent) {
        let Some(entry) = self
            .history
            .as_ref()
            .and_then(|history| history.panel().selected_entry())
            .cloned()
        else {
            return;
        };
        self.history_load_jobs.cancel_all();
        self.active_history_load = None;
        self.queued_history_load = Some(HistoryLoadRequest { entry, intent });
        if let Some(history) = self.history.as_mut() {
            history.panel_mut().mark_loading();
            history.request_redraw();
        }
    }

    fn cancel_history_loads(&mut self) {
        let cancelled = self.history_load_jobs.cancel_all();
        self.active_history_load = None;
        self.queued_history_load = None;
        if cancelled > 0 {
            println!("pinora: cancelled {cancelled} history load job(s)");
        }
    }

    fn start_queued_history_load(&mut self) {
        if !self.history_load_jobs.is_idle() {
            return;
        }
        let Some(request) = self.queued_history_load.take() else {
            return;
        };
        let still_selected = self
            .history
            .as_ref()
            .and_then(|history| history.panel().selected_entry())
            .is_some_and(|entry| {
                entry.image_id == request.entry.image_id
                    && entry.generation == request.entry.generation
            });
        if !still_selected {
            return;
        }
        let Some(export_dir) = self
            .runtime
            .as_ref()
            .map(|runtime| runtime.export_dir().clone())
        else {
            self.mark_history_load_error("history_load_failed");
            return;
        };
        let asset = AssetRef::new(request.entry.image_id, request.entry.generation);
        let spec = JobSpec::new(
            JobId::new(),
            CorrelationId::new(),
            asset,
            JobOwner::History(request.entry.image_id),
            JobKind::HistoryLoad,
            monotonic_ms().saturating_add(HISTORY_LOAD_TIMEOUT_MS),
        );
        let input = HistoryLoadInput {
            export_dir,
            entry: request.entry.clone(),
            preparation: match request.intent {
                HistoryLoadIntent::Preview => HistoryLoadPreparation::Preview,
                HistoryLoadIntent::Reopen => HistoryLoadPreparation::Pin,
                HistoryLoadIntent::Edit => HistoryLoadPreparation::Editor,
            },
        };
        match self.history_load_jobs.start(spec, input) {
            Ok(ticket) => {
                println!(
                    "pinora: history load {} started image={} intent={:?}",
                    ticket.id, request.entry.image_id, request.intent
                );
                self.active_history_load = Some(ActiveHistoryLoad {
                    job_id: ticket.id,
                    request,
                });
            }
            Err(error) => {
                eprintln!("pinora: history load start failed: {error}");
                self.mark_history_load_error("history_load_failed");
            }
        }
    }

    fn take_active_history_load(&mut self, job_id: JobId) -> Option<HistoryLoadRequest> {
        (self
            .active_history_load
            .as_ref()
            .is_some_and(|active| active.job_id == job_id))
        .then(|| self.active_history_load.take().map(|active| active.request))
        .flatten()
    }

    fn mark_history_load_error(&mut self, code: &'static str) {
        if let Some(history) = self.history.as_mut() {
            history.clear_preview();
            history.panel_mut().mark_error(code);
            history.request_redraw();
        }
    }

    fn poll_history_load_jobs(&mut self, event_loop: &ActiveEventLoop) {
        let active = self.active_history_load.clone();
        let selected = self
            .history
            .as_ref()
            .and_then(|history| history.panel().selected_entry())
            .cloned();
        let completions = self
            .history_load_jobs
            .poll(monotonic_ms(), |job_id, owner| {
                current_history_load_asset(active.as_ref(), selected.as_ref(), job_id, owner)
            });

        for completion in completions {
            match completion {
                HistoryLoadCompletion::Completed { job, payload } => {
                    let Some(request) = self.take_active_history_load(job.id) else {
                        continue;
                    };
                    match (request.intent, payload) {
                        (
                            HistoryLoadIntent::Preview,
                            HistoryLoadPayload::Preview { size, pixels_xrgb },
                        ) => {
                            if let Some(history) = self.history.as_mut() {
                                history.cache_preview(request.entry.image_id, pixels_xrgb, size);
                                history.panel_mut().clear_error();
                                history.request_redraw();
                            }
                        }
                        (
                            HistoryLoadIntent::Reopen,
                            HistoryLoadPayload::Pin { image, pixels_xrgb },
                        ) => {
                            self.open_loaded_history_pin(
                                event_loop,
                                request.entry,
                                image,
                                pixels_xrgb,
                            );
                        }
                        (
                            HistoryLoadIntent::Edit,
                            HistoryLoadPayload::Editor {
                                image,
                                base,
                                dimmed,
                            },
                        ) => {
                            self.open_loaded_history_editor(
                                event_loop,
                                request.entry,
                                image,
                                base,
                                dimmed,
                            );
                        }
                        (intent, payload) => {
                            let preparation = payload.preparation();
                            eprintln!(
                                "pinora: history load payload mismatch intent={intent:?} preparation={preparation:?}"
                            );
                            self.mark_history_load_error("history_load_failed");
                        }
                    }
                }
                HistoryLoadCompletion::Failed {
                    job_id,
                    owner,
                    error,
                } => {
                    if self.take_active_history_load(job_id).is_some() {
                        eprintln!("pinora: history load {job_id} failed owner={owner:?}: {error}");
                        self.mark_history_load_error("history_load_failed");
                    }
                }
                HistoryLoadCompletion::Discarded { job_id, terminal } => {
                    if self.take_active_history_load(job_id).is_some() {
                        println!("pinora: history load {job_id} discarded ({terminal:?})");
                        if !matches!(terminal, pinora_core::JobTerminalState::Cancelled) {
                            self.mark_history_load_error("history_load_failed");
                        }
                    }
                }
            }
        }
        self.start_queued_history_load();
    }

    fn handle_history_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_history(),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(history) = self.history.as_mut() {
                    history.set_cursor(PixelPoint::new(
                        position.x.round() as i32,
                        position.y.round() as i32,
                    ));
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let action = self
                    .history
                    .as_ref()
                    .and_then(|history| history.panel().hit_test(history.cursor()));
                if let Some(action) = action {
                    self.apply_history_action(event_loop, action);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.is_quit_key(&event) {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                if self.handle_capture_shortcut(event_loop, &event) {
                    self.close_history();
                    return;
                }
                let key = match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => Some(HistoryPanelKey::Up),
                    PhysicalKey::Code(KeyCode::ArrowDown) => Some(HistoryPanelKey::Down),
                    PhysicalKey::Code(KeyCode::Enter) => Some(HistoryPanelKey::Enter),
                    PhysicalKey::Code(KeyCode::KeyE) => Some(HistoryPanelKey::Edit),
                    PhysicalKey::Code(KeyCode::Delete) => Some(HistoryPanelKey::Delete),
                    PhysicalKey::Code(KeyCode::Backspace) => Some(HistoryPanelKey::Backspace),
                    PhysicalKey::Code(KeyCode::Escape) => Some(HistoryPanelKey::Escape),
                    _ => None,
                };
                let action = if let Some(key) = key {
                    self.history
                        .as_mut()
                        .and_then(|history| history.panel_mut().handle_key(key))
                } else if !self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && !self.modifiers.super_key()
                {
                    let text = match &event.logical_key {
                        Key::Character(text) => Some(text.as_str()),
                        _ => None,
                    };
                    if let Some(text) = text
                        && let Some(history) = self.history.as_mut()
                    {
                        for character in text.chars() {
                            let _ = history.panel_mut().input_char(character);
                        }
                    }
                    None
                } else {
                    None
                };
                if let Some(action) = action {
                    self.apply_history_action(event_loop, action);
                } else {
                    self.queue_history_load(HistoryLoadIntent::Preview);
                }
                if let Some(history) = self.history.as_ref() {
                    history.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.paint_history() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(history) = self.history.as_mut() {
                    history.resize(size.width, size.height);
                }
            }
            _ => {}
        }
    }

    fn apply_history_action(&mut self, _event_loop: &ActiveEventLoop, action: HistoryPanelAction) {
        match action {
            HistoryPanelAction::Select(index) => {
                if let Some(history) = self.history.as_mut() {
                    history.panel_mut().select(index);
                    history.clear_preview();
                }
                self.queue_history_load(HistoryLoadIntent::Preview);
            }
            HistoryPanelAction::Close => self.close_history(),
            HistoryPanelAction::Reopen => self.reopen_history_entry(),
            HistoryPanelAction::Edit => self.edit_history_entry(),
            HistoryPanelAction::Delete => self.delete_selected_history_entry(),
            HistoryPanelAction::RequestClear => {
                if let Some(history) = self.history.as_mut() {
                    let _ = history.panel_mut().request_clear();
                    history.request_redraw();
                }
            }
            HistoryPanelAction::CancelClear => {
                if let Some(history) = self.history.as_mut() {
                    history.panel_mut().cancel_clear();
                    history.request_redraw();
                }
            }
            HistoryPanelAction::ConfirmClear => self.clear_all_history_entries(),
        }
    }

    fn clear_all_history_entries(&mut self) {
        self.cancel_history_loads();
        let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir().clone()) else {
            return;
        };
        let result =
            clear_history_entries(&self.history_store, &export_dir, &mut self.history_index);
        let active = self.history_index.active_entries().cloned().collect();
        if let Some(history) = self.history.as_mut() {
            history.clear_preview();
            history.panel_mut().replace_entries(active);
            history.panel_mut().cancel_clear();
            match result {
                Err(error) => {
                    eprintln!("pinora: history clear failed: {error}");
                    history.panel_mut().mark_error("history_clear_failed");
                }
                Ok(cleanup) => {
                    if cleanup.failed_files > 0 || cleanup.protected_files > 0 {
                        history.panel_mut().mark_error("history_clear_partial");
                    } else {
                        history.panel_mut().clear_error();
                    }
                }
            }
            history.request_redraw();
        }
    }

    fn reopen_history_entry(&mut self) {
        self.queue_history_load(HistoryLoadIntent::Reopen);
    }

    fn open_loaded_history_pin(
        &mut self,
        event_loop: &ActiveEventLoop,
        entry: HistoryEntry,
        image: CaptureImage,
        pixels_xrgb: Vec<u32>,
    ) {
        match self.open_pin_from_prepared_image(
            event_loop,
            image,
            pixels_xrgb,
            entry.source_rect.origin,
            false,
        ) {
            Ok(()) => self.close_history(),
            Err(error) => {
                eprintln!("pinora: history reopen failed ({})", error.code);
                if let Some(history) = self.history.as_mut() {
                    history.panel_mut().mark_error("history_pin_failed");
                    history.request_redraw();
                }
            }
        }
    }

    fn edit_history_entry(&mut self) {
        self.queue_history_load(HistoryLoadIntent::Edit);
    }

    fn open_loaded_history_editor(
        &mut self,
        event_loop: &ActiveEventLoop,
        _entry: HistoryEntry,
        image: CaptureImage,
        base: Vec<u32>,
        dimmed: Vec<u32>,
    ) {
        // 历史编辑使用已验证的图像，既不能触发屏幕捕获，也不能复用旧显示器的全屏语义。
        let target = history_edit_target(&image);
        let preview = PreparedPreview {
            image,
            base,
            dimmed,
        };
        if let Some(cache) = &self.frame_cache {
            cache.pause();
        }
        match self.open_overlay_with_preview(event_loop, preview, target) {
            Ok(()) => self.close_history(),
            Err(error) => {
                self.resume_frame_cache();
                eprintln!("pinora: history edit failed ({})", error.code);
                if let Some(history) = self.history.as_mut() {
                    history.panel_mut().mark_error("history_edit_failed");
                    history.request_redraw();
                }
            }
        }
    }

    fn delete_selected_history_entry(&mut self) {
        self.cancel_history_loads();
        let Some(image_id) = self
            .history
            .as_ref()
            .and_then(|history| history.panel().selected_entry())
            .map(|entry| entry.image_id)
        else {
            return;
        };
        let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir().clone()) else {
            return;
        };
        let delete_result = delete_history_entry(
            &self.history_store,
            &export_dir,
            &mut self.history_index,
            image_id,
        );
        if delete_result.is_err() {
            let remaining = self.history_index.active_entries().cloned().collect();
            if let Some(history) = self.history.as_mut() {
                history.clear_preview();
                history.panel_mut().replace_entries(remaining);
                history.panel_mut().mark_error("history_delete_failed");
                history.request_redraw();
            }
            return;
        }
        if let Some(history) = self.history.as_mut() {
            history.clear_preview();
            history
                .panel_mut()
                .replace_entries(self.history_index.active_entries().cloned().collect());
            history.request_redraw();
        }
        self.queue_history_load(HistoryLoadIntent::Preview);
    }

    fn paint_history(&mut self) -> Result<(), PinoraError> {
        let Some(history) = self.history.as_mut() else {
            return Ok(());
        };
        history.paint()
    }

    fn request_new_capture(&mut self, event_loop: &ActiveEventLoop) {
        self.request_capture(
            event_loop,
            CaptureMode::Region,
            CaptureTarget::DefaultLargest,
        );
    }

    fn request_full_display_capture(&mut self, event_loop: &ActiveEventLoop) {
        self.request_capture(
            event_loop,
            CaptureMode::FullDisplay,
            CaptureTarget::DefaultLargest,
        );
    }

    fn request_display_capture(&mut self, event_loop: &ActiveEventLoop, display_id: DisplayId) {
        self.request_capture(
            event_loop,
            CaptureMode::FullDisplay,
            CaptureTarget::Display(display_id),
        );
    }

    fn request_all_displays_capture(&mut self, event_loop: &ActiveEventLoop) {
        self.request_capture(
            event_loop,
            CaptureMode::AllDisplays,
            CaptureTarget::AllDisplays,
        );
    }

    fn request_window_capture(&mut self, event_loop: &ActiveEventLoop, target: CaptureWindowInfo) {
        self.request_capture(
            event_loop,
            CaptureMode::Window,
            CaptureTarget::Window(target),
        );
    }

    fn request_delayed_region_capture(&mut self, delay: Duration) {
        if self.delayed_capture.is_some() {
            println!("pinora: delayed capture already active; use tray cancel first");
            return;
        }

        // 延时开始前关闭 Pinora 自己现有的短暂窗口；不创建新的倒计时窗口。
        if self.loading.is_some() {
            self.cancel_loading();
        }
        if self.overlay.is_some() {
            self.cancel_overlay();
        }
        self.close_settings();
        self.close_history();
        self.mode = Mode::Idle;
        self.start_capture_wait = None;
        self.capture_mode = CaptureMode::Region;
        self.capture_target = CaptureTarget::DefaultLargest;

        // 必须先拒绝任何在途预截帧，再隐藏原先可见的贴图，防止到期时拿到旧图像。
        if let Some(cache) = &self.frame_cache {
            cache.pause();
        }
        let hidden_pin_ids = self.snapshot_visible_pin_ids();
        self.set_pins_visible(&hidden_pin_ids, false);
        self.delayed_capture = Some(DelayedCapture::new(delay, hidden_pin_ids));
        self.set_delayed_capture_tray_state(true);
        self.mode = Mode::DelayedCapture;
        println!(
            "pinora: tray → delayed region capture in {}s (no countdown window)",
            delay.as_secs()
        );
    }

    fn poll_delayed_capture(&mut self, event_loop: &ActiveEventLoop) {
        let due = self
            .delayed_capture
            .as_ref()
            .is_some_and(|capture| capture.is_due(Instant::now()));
        if !due {
            return;
        }

        println!("pinora: delayed capture due; starting cold capture");
        if let Err(error) = self.begin_screen_grab(event_loop, false) {
            self.finish_delayed_capture_failure(error);
        }
    }

    /// 任意模式触发再截：立刻关 overlay/loading，开新一轮 grab。
    fn request_capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        capture_mode: CaptureMode,
        capture_target: CaptureTarget,
    ) {
        if self.delayed_capture.is_some() {
            println!("pinora: capture request ignored while delayed capture is active");
            return;
        }
        let edited_pin = if let Some(ov) = self.overlay.take() {
            let edited_pin = ov.edit_pin_id;
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            self.export_jobs
                .close_owner(JobOwner::Session(ov.session_id));
            ov.window.set_visible(false);
            edited_pin
        } else {
            None
        };
        if let Some(pin_id) = edited_pin {
            // 再截图相当于取消贴图编辑，不能让原贴图永远保持隐藏。
            self.restore_pin_visibility(pin_id);
        }
        self.close_settings();
        self.close_history();
        let _ = self.loading.take();
        self.capture_mode = capture_mode;
        self.capture_target = capture_target;
        self.mode = Mode::StartCapture;
        println!(
            "pinora: new {} capture requested ({})",
            capture_mode.label(),
            self.capture_target.log_label()
        );
        if let Err(e) = self.begin_screen_grab(event_loop, true) {
            self.handle_capture_start_error(e);
        }
    }

    fn handle_capture_start_error(&mut self, error: PinoraError) {
        self.finish_loading_capture_failure(error);
    }

    fn capture_failure_scope(&self) -> CaptureFailureScope {
        capture_failure_scope(&self.capture_target, self.delayed_capture.is_some())
    }

    fn finish_window_capture_failure(&mut self, error: PinoraError) {
        eprintln!(
            "pinora: window capture failed ({}); returning to tray",
            error.code
        );
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.start_capture_wait = None;
        self.resume_frame_cache();
    }

    fn finish_delayed_capture_failure(&mut self, error: PinoraError) {
        eprintln!(
            "pinora: delayed capture failed ({}); returning to tray",
            error.code
        );
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.start_capture_wait = None;
        self.restore_delayed_pins();
        self.resume_frame_cache();
    }

    fn cancel_loading(&mut self) {
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.restore_delayed_pins();
        self.resume_frame_cache();
        println!("pinora: capture cancelled (F2/Ctrl+N 再截，Ctrl+Q 退出)");
        if let Some(pin) = self.pins.values().next() {
            pin.window.focus_window();
        }
    }

    fn finish_standard_capture_failure(&mut self, error: PinoraError) {
        eprintln!("pinora: capture failed ({}); returning to tray", error.code);
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.start_capture_wait = None;
        self.resume_frame_cache();
    }

    fn finish_capture_failure_in_scope(&mut self, scope: CaptureFailureScope, error: PinoraError) {
        match scope {
            CaptureFailureScope::Standard => self.finish_standard_capture_failure(error),
            CaptureFailureScope::Window => self.finish_window_capture_failure(error),
            CaptureFailureScope::Delayed => self.finish_delayed_capture_failure(error),
        }
    }

    fn finish_loading_capture_failure(&mut self, error: PinoraError) {
        self.finish_capture_failure_in_scope(self.capture_failure_scope(), error);
    }

    fn poll_loading_to_overlay(&mut self, event_loop: &ActiveEventLoop) {
        let Some(loading) = self.loading.as_ref() else {
            return;
        };
        let prep = match loading.preview_rx.try_recv() {
            Ok(Ok(p)) => p,
            Ok(Err(code)) => {
                self.finish_loading_capture_failure(PinoraError::new(
                    code,
                    "capture provider returned an error",
                ));
                return;
            }
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.finish_loading_capture_failure(PinoraError::new(
                    ErrorCode::Internal,
                    "capture thread disconnected",
                ));
                return;
            }
        };

        let loading = self.loading.take().unwrap();
        if !preview_buffers_match_image(&prep) {
            self.finish_loading_capture_failure(PinoraError::new(
                ErrorCode::Internal,
                "capture buffer size mismatch",
            ));
            return;
        }
        let img_w = prep.image.pixels.size.width.max(1);
        let img_h = prep.image.pixels.size.height.max(1);

        let mut target = loading.target;
        target.image_width = img_w;
        target.image_height = img_h;
        let failure_scope = self.capture_failure_scope();
        // 此时 capture provider 已经取得真实像素，恢复不会进入本次截图。
        if self.delayed_capture.is_some() {
            self.restore_delayed_pins();
        }
        if let Err(error) = self.open_overlay_with_preview(event_loop, prep, target) {
            self.finish_capture_failure_in_scope(failure_scope, error);
        }
    }

    fn open_overlay_with_preview(
        &mut self,
        event_loop: &ActiveEventLoop,
        prep: PreparedPreview,
        target: OverlayTarget,
    ) -> Result<(), PinoraError> {
        let OverlayTarget {
            display_id,
            display_origin,
            image_width: img_w,
            image_height: img_h,
            initial_selection,
            presentation,
            min_selection_edge,
            edit_pin_id,
        } = target;
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let (title, attrs) = match presentation {
            OverlayPresentation::ScreenCapture => {
                let title = "Pinora — 拖选后工具栏 | 双击复制 中键贴图 Enter贴图 Esc取消";
                (
                    title,
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(PhysicalSize::new(img_w, img_h))
                        .with_fullscreen(Some(Fullscreen::Borderless(None)))
                        .with_cursor(CursorIcon::Crosshair)
                        .with_decorations(false)
                        .with_visible(false),
                )
            }
            OverlayPresentation::VirtualDesktop => {
                let title = "Pinora Virtual Desktop Capture";
                (
                    title,
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(PhysicalSize::new(img_w, img_h))
                        .with_position(PhysicalPosition::new(display_origin.x, display_origin.y))
                        .with_cursor(CursorIcon::Crosshair)
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_visible(false),
                )
            }
            OverlayPresentation::WindowCapture => {
                let title = "Pinora Window Capture";
                (
                    title,
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(PhysicalSize::new(img_w, img_h))
                        .with_cursor(CursorIcon::Crosshair)
                        .with_decorations(true)
                        .with_resizable(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_visible(false),
                )
            }
            OverlayPresentation::HistoryEditor => {
                let title = "Pinora History Edit";
                (
                    title,
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(PhysicalSize::new(img_w, img_h))
                        .with_cursor(CursorIcon::Crosshair)
                        .with_decorations(true)
                        .with_resizable(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_visible(false),
                )
            }
            OverlayPresentation::PinEditor => {
                let title = "Pinora Pin Edit";
                (
                    title,
                    Window::default_attributes()
                        .with_title(title)
                        .with_inner_size(PhysicalSize::new(img_w, img_h))
                        .with_cursor(CursorIcon::Crosshair)
                        .with_decorations(true)
                        .with_resizable(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_visible(false),
                )
            }
        };
        let window =
            window_policy::create_auxiliary_window(event_loop, AuxiliaryWindowKind::Overlay, attrs)
                .map_err(|e| {
                    PinoraError::new(ErrorCode::Internal, format!("overlay window: {e}"))
                })?;
        let window = Rc::new(window);

        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay surface: {e}")))?;

        // 1:1 原图像素显示（不降采样，避免全屏发糊）。
        // softbuffer 固定为截图尺寸；禁止跟窗口 resize 走（否则会整屏 scale 卡死）。
        // 性能靠：脏区 present、工具零重绘、拖选节流、release 构建。
        let src_w = img_w.max(1);
        let src_h = img_h.max(1);
        let buf_w = src_w;
        let buf_h = src_h;
        let dimmed = prep.dimmed;
        let base = prep.base;
        if let (Some(w), Some(h)) = (NonZeroU32::new(buf_w), NonZeroU32::new(buf_h)) {
            let _ = surface.resize(w, h);
        }
        let size = window.inner_size();
        let win_w = size.width.max(1);
        let win_h = size.height.max(1);

        let frame = dimmed.clone();
        println!(
            "pinora: overlay ready {src_w}x{src_h} 1:1 win={win_w}x{win_h} display={display_id:?}"
        );
        if cfg!(debug_assertions) {
            println!("pinora: tip: 4K 请用 `cargo run --release`，debug 会明显更卡");
        }
        window.set_ime_allowed(true);

        self.overlay = Some(OverlayState {
            window: window.clone(),
            surface,
            dimmed,
            base,
            frame,
            session: SelectionSession::new()
                .with_bounds(PixelRect::new(0, 0, buf_w, buf_h))
                .with_min_edge(min_selection_edge),
            phase: OverlayPhase::Selecting,
            dragging: false,
            pending_reselect: false,
            drag_anchor: PixelPoint::new(0, 0),
            annotate_dragging: false,
            annotate: AnnotateSession::new(1, 1),
            last_drawing_tool: AnnotateTool::Rect,
            annotate_cache: None,
            annotate_cache_wh: (0, 0),
            annotate_preview_cache: OverlayPreviewCache::default(),
            annotate_dirty: false,
            toolbar: Vec::new(),
            toolbar_pressed: None,
            last_toolbar_bounds: None,
            toolbar_chrome_dirty: false,
            buffer_synced: false,
            last_cursor: PixelPoint::new(0, 0),
            needs_redraw: true,
            last_drawn_rect: None,
            last_present: Instant::now()
                .checked_sub(MIN_FRAME_INTERVAL * 2)
                .unwrap_or_else(Instant::now),
            last_draw_cursor: PixelPoint::new(i32::MIN / 4, i32::MIN / 4),
            last_click_at: None,
            last_click_pos: PixelPoint::new(0, 0),
            src_w,
            src_h,
            buf_w,
            buf_h,
            active_src_rect: None,
            win_w,
            win_h,
            display_id,
            display_origin,
            full_image: prep.image,
            session_id: SessionId::new(),
            annotation_asset: None,
            edit_pin_id,
        });
        if let Some(selection) = apply_initial_selection(
            &mut self
                .overlay
                .as_mut()
                .expect("overlay was just created")
                .session,
            initial_selection,
        )? {
            let overlay = self.overlay.as_mut().expect("overlay was just created");
            overlay.phase = OverlayPhase::Ready;
            refresh_overlay_ready(overlay);
            println!(
                "pinora: initial selection ready {}x{}",
                selection.size.width, selection.size.height
            );
        }
        self.mode = Mode::Idle;
        window_policy::show_auxiliary_window(AuxiliaryWindowKind::Overlay, &window, title);
        window.focus_window();
        window.request_redraw();
        Ok(())
    }

    fn handle_overlay_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        // 先处理会拿走整个 overlay 的全局键，避免与 ov 可变借用冲突
        if let WindowEvent::KeyboardInput { event: ref key, .. } = event
            && key.state.is_pressed()
        {
            if self.is_quit_key(key) {
                self.quit = true;
                event_loop.exit();
                return;
            }
            if self.handle_capture_shortcut(event_loop, key) {
                return;
            }
        }

        // 会消费 overlay 的动作（贴图/复制等）先判定
        match &event {
            WindowEvent::CloseRequested => {
                self.cancel_overlay();
                return;
            }
            WindowEvent::KeyboardInput { event: key, .. } if key.state.is_pressed() => {
                if matches!(key.logical_key, Key::Named(NamedKey::Escape)) {
                    // 有草稿先取消草稿，否则关 overlay
                    if let Some(ov) = self.overlay.as_mut()
                        && ov.annotate.draft.is_some()
                    {
                        ov.annotate.cancel_draft();
                        ov.annotate_dragging = false;
                        ov.needs_redraw = true;
                        return;
                    }
                    self.cancel_overlay();
                    return;
                }
                if matches!(key.logical_key, Key::Named(NamedKey::Enter)) {
                    // 文本草稿：先提交文字；Ctrl+Enter 同样。裸 Enter 贴图。
                    if let Some(ov) = self.overlay.as_mut()
                        && ov.annotate.is_text_editing()
                    {
                        let revision = ov.annotate.doc.revision();
                        ov.annotate.commit();
                        if ov.annotate.doc.revision() != revision {
                            ov.annotate_dirty = true;
                        }
                        ov.needs_redraw = true;
                        println!("pinora: text committed on overlay");
                        return;
                    }
                    if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Pin) {
                        eprintln!("pinora: pin failed: {e}");
                    }
                    return;
                }
                if matches!(key.logical_key, Key::Named(NamedKey::Space)) {
                    if let Some(ov) = self.overlay.as_mut()
                        && ov.annotate.is_text_editing()
                    {
                        ov.annotate.text_push(" ");
                        ov.needs_redraw = true;
                        return;
                    }
                    if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Pin) {
                        eprintln!("pinora: pin failed: {e}");
                    }
                    return;
                }
            }
            _ => {}
        }

        let Some(ov) = self.overlay.as_mut() else {
            return;
        };

        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::Ime(Ime::Commit(text))
                if ov.phase == OverlayPhase::Ready
                    && ov.annotate.is_text_editing()
                    && !text.is_empty() =>
            {
                ov.annotate.text_push(&text);
                ov.needs_redraw = true;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let modifiers = self.modifiers;
                handle_overlay_key(modifiers, ov, &event);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.handle_overlay_left(event_loop, state);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Pin) {
                    eprintln!("pinora: middle-pin failed: {e}");
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let Some(ov) = self.overlay.as_mut() else {
                    return;
                };
                ov.last_cursor = window_to_image(
                    position.x, position.y, ov.win_w, ov.win_h, ov.buf_w, ov.buf_h,
                );
                if ov.dragging {
                    let p = ov.last_cursor;
                    if ov.pending_reselect {
                        let dx = (p.x - ov.drag_anchor.x).abs();
                        let dy = (p.y - ov.drag_anchor.y).abs();
                        if dx >= 4 || dy >= 4 {
                            // 确认重选：退出 Ready，清工具栏
                            ov.pending_reselect = false;
                            ov.phase = OverlayPhase::Selecting;
                            ov.toolbar.clear();
                            ov.toolbar_pressed = None;
                            ov.last_toolbar_bounds = None;
                            ov.annotate = AnnotateSession::new(1, 1);
                            ov.annotate_cache = None;
                            ov.annotate_preview_cache.clear();
                            ov.active_src_rect = None;
                            // 选区一旦确认重选，旧任务立即失效；不能等到松手才换身份。
                            ov.annotation_asset = Some(OverlayAssetIdentity::new());
                            ov.annotate_dirty = true;
                            ov.session.begin_drag(ov.drag_anchor);
                            ov.session.update_cursor(p);
                            ov.needs_redraw = true;
                        }
                    } else {
                        ov.session.update_cursor(p);
                        // 拖选节流：小抖动不重绘，降低 4K 调试下事件风暴
                        let dx = (p.x - ov.last_draw_cursor.x).abs();
                        let dy = (p.y - ov.last_draw_cursor.y).abs();
                        if dx >= 2
                            || dy >= 2
                            || ov.last_present.elapsed() >= Duration::from_millis(32)
                        {
                            ov.last_draw_cursor = p;
                            ov.needs_redraw = true;
                        }
                    }
                } else if ov.annotate_dragging
                    && let Some(local) = overlay_annotate_local(ov, ov.last_cursor)
                {
                    ov.annotate.drag(local);
                    ov.annotate_dirty = true;
                    ov.needs_redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = paint_overlay(ov) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                // 只更新鼠标映射用的窗口尺寸；softbuffer 保持 img 尺寸
                ov.win_w = size.width.max(1);
                ov.win_h = size.height.max(1);
                if let (Some(w), Some(h)) = (NonZeroU32::new(ov.buf_w), NonZeroU32::new(ov.buf_h)) {
                    let _ = ov.surface.resize(w, h);
                }
                // 不在此全量 clone dimmed / 不清 buffer_synced，避免拖一下窗口就卡死
            }
            _ => {}
        }
    }

    fn handle_overlay_left(&mut self, event_loop: &ActiveEventLoop, state: ElementState) {
        match state {
            ElementState::Pressed => {
                if let Some(sample) = self.take_overlay_color_sample() {
                    match sample {
                        Some((owner, asset, hex)) => {
                            if let Err(error) = self.submit_export_job(
                                owner,
                                asset,
                                ExportJobInput::CopyText { text: hex },
                                PendingExportAction::CopyText,
                            ) {
                                eprintln!("pinora: color clipboard submit failed: {error}");
                            }
                        }
                        None => eprintln!("pinora: color picker source pixel unavailable"),
                    }
                    return;
                }
                let Some(ov) = self.overlay.as_mut() else {
                    return;
                };
                let p = ov.last_cursor;
                ov.toolbar_pressed = None;

                // 1) 工具栏：只记录按下，抬起时触发（更稳）
                if ov.phase == OverlayPhase::Ready {
                    if let Some(action) = toolbar_hit(&ov.toolbar, p) {
                        ov.toolbar_pressed = Some(action);
                        println!("pinora: toolbar press {action:?}");
                        return;
                    }
                    // 点在工具栏间隙/边框上也不要开新选区
                    if let Some(bounds) = toolbar_bounds(&ov.toolbar) {
                        let pad = 6;
                        let expanded = PixelRect::new(
                            bounds.origin.x - pad,
                            bounds.origin.y - pad,
                            bounds.size.width + (pad * 2) as u32,
                            bounds.size.height + (pad * 2) as u32,
                        );
                        if expanded.contains_point(p) {
                            return;
                        }
                    }
                }

                // 2) 选区内：双击复制 / 标注
                if ov.phase == OverlayPhase::Ready
                    && let Ok(sel) = ov.session.try_confirm()
                    && sel.contains_point(p)
                {
                    let now = Instant::now();
                    let is_double = ov
                        .last_click_at
                        .map(|t| now.duration_since(t) < Duration::from_millis(400))
                        .unwrap_or(false)
                        && (p.x - ov.last_click_pos.x).abs() < 12
                        && (p.y - ov.last_click_pos.y).abs() < 12;
                    ov.last_click_at = Some(now);
                    ov.last_click_pos = p;
                    if overlay_click_finishes_copy(ov.annotate.tool, is_double) {
                        if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Copy)
                        {
                            eprintln!("pinora: double-click copy failed: {e}");
                        }
                        return;
                    }
                    if let Some(local) = overlay_annotate_local(ov, p) {
                        ov.annotate.begin(local);
                        ov.annotate_dragging = !matches!(
                            ov.annotate.tool,
                            AnnotateTool::Text | AnnotateTool::Number | AnnotateTool::ColorPicker
                        );
                        ov.annotate_dirty = true;
                        ov.needs_redraw = true;
                    }
                    return;
                }

                // 3) 选区外：准备拖新选区（Ready 下需移动阈值才真正重选）
                ov.dragging = true;
                ov.drag_anchor = p;
                ov.annotate_dragging = false;
                if ov.phase == OverlayPhase::Ready {
                    ov.pending_reselect = true;
                } else {
                    ov.pending_reselect = false;
                    ov.session.begin_drag(p);
                    ov.needs_redraw = true;
                }
            }
            ElementState::Released => {
                // 工具栏抬起触发
                let toolbar_action = self.overlay.as_mut().and_then(|ov| {
                    let pressed = ov.toolbar_pressed.take()?;
                    let p = ov.last_cursor;
                    // 抬起时仍在任意工具栏按钮上即触发原按下动作（允许轻微移动）
                    if toolbar_hit(&ov.toolbar, p).is_some()
                        || toolbar_bounds(&ov.toolbar).is_some_and(|b| {
                            let pad = 8;
                            PixelRect::new(
                                b.origin.x - pad,
                                b.origin.y - pad,
                                b.size.width + 16,
                                b.size.height + 16,
                            )
                            .contains_point(p)
                        })
                    {
                        Some(pressed)
                    } else {
                        None
                    }
                });
                if let Some(action) = toolbar_action {
                    println!("pinora: toolbar release → {action:?}");
                    self.apply_toolbar_action(event_loop, action);
                    return;
                }

                let Some(ov) = self.overlay.as_mut() else {
                    return;
                };
                if ov.dragging {
                    ov.dragging = false;
                    if ov.pending_reselect {
                        // 未移动够：当作误触，保持 Ready
                        ov.pending_reselect = false;
                        return;
                    }
                    if let Ok(sel) = ov.session.try_confirm() {
                        ov.phase = OverlayPhase::Ready;
                        ov.toolbar = layout_toolbar(sel, ov.buf_w, ov.buf_h);
                        let src_sel = buf_rect_to_src(sel, ov.buf_w, ov.buf_h, ov.src_w, ov.src_h);
                        ov.active_src_rect = Some(src_sel);
                        ov.annotation_asset = Some(OverlayAssetIdentity::new());
                        let tool = ov.annotate.tool;
                        let color = ov.annotate.color;
                        let stroke = ov.annotate.stroke;
                        let shape_fill_enabled = ov.annotate.shape_fill_enabled();
                        // 标注坐标系 = 原图选区像素
                        ov.annotate = AnnotateSession::new(src_sel.size.width, src_sel.size.height);
                        ov.annotate.tool = tool;
                        ov.annotate.color = color;
                        ov.annotate.stroke = stroke;
                        if shape_fill_enabled {
                            ov.annotate.toggle_shape_fill();
                        }
                        ov.annotate_cache = None;
                        ov.annotate_preview_cache.clear();
                        ov.annotate_dirty = true;
                        println!(
                            "pinora: selection buf={}x{} src={}x{} toolbar={} | 双击复制 中键/Enter贴图",
                            sel.size.width,
                            sel.size.height,
                            src_sel.size.width,
                            src_sel.size.height,
                            ov.toolbar.len()
                        );
                    } else {
                        ov.phase = OverlayPhase::Selecting;
                        ov.toolbar.clear();
                        ov.active_src_rect = None;
                    }
                    ov.needs_redraw = true;
                } else if ov.annotate_dragging {
                    ov.annotate.commit();
                    ov.annotate_dragging = false;
                    ov.annotate_dirty = true;
                    ov.needs_redraw = true;
                } else if ov.annotate.is_text_editing() {
                    ov.annotate_dirty = true;
                    ov.needs_redraw = true;
                }
            }
        }
    }

    fn apply_toolbar_action(&mut self, event_loop: &ActiveEventLoop, action: ToolbarAction) {
        match action {
            ToolbarAction::Copy => {
                if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Copy) {
                    eprintln!("pinora: toolbar copy: {e}");
                }
            }
            ToolbarAction::Pin => {
                if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Pin) {
                    eprintln!("pinora: toolbar pin: {e}");
                }
            }
            ToolbarAction::Save => {
                if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Save) {
                    eprintln!("pinora: toolbar save: {e}");
                }
            }
            ToolbarAction::Ocr => {
                self.overlay_ocr();
            }
            ToolbarAction::ToggleFill => {
                if let Some(ov) = self.overlay.as_mut() {
                    let enabled = ov.annotate.toggle_shape_fill();
                    ov.toolbar_chrome_dirty = true;
                    ov.needs_redraw = true;
                    println!("pinora: shape fill enabled={enabled}");
                }
            }
            ToolbarAction::Tool(tool) => {
                // 工具高亮和色块仅重绘工具栏，不重新烘焙选区。
                if let Some(ov) = self.overlay.as_mut() {
                    set_overlay_tool(ov, tool);
                    println!("pinora: tool = {tool:?}");
                }
            }
        }
    }

    fn submit_ocr_job(&mut self, owner: JobOwner, image: CaptureImage, asset: AssetRef) {
        let size = image.size();
        let spec = JobSpec::new(
            JobId::new(),
            CorrelationId::new(),
            asset,
            owner,
            JobKind::Ocr,
            monotonic_ms().saturating_add(OCR_JOB_TIMEOUT_MS),
        );
        match self.ocr_jobs.start(spec, image) {
            Ok(ticket) => println!(
                "pinora: OCR job {} started owner={owner:?} {}x{}",
                ticket.id, size.width, size.height
            ),
            Err(error) => eprintln!("pinora: OCR submit failed: {error}"),
        }
    }

    fn submit_export_job(
        &mut self,
        owner: JobOwner,
        asset: AssetRef,
        input: ExportJobInput,
        action: PendingExportAction,
    ) -> Result<JobId, PinoraError> {
        let history = self.runtime.as_ref().and_then(|runtime| {
            history_candidate_for_export(runtime.export_dir(), owner, asset, &input)
        });
        let kind = input.kind();
        let spec = JobSpec::new(
            JobId::new(),
            CorrelationId::new(),
            asset,
            owner,
            kind,
            monotonic_ms().saturating_add(EXPORT_JOB_TIMEOUT_MS),
        );
        let ticket = self.export_jobs.start(spec, input)?;
        self.pending_exports.insert(
            ticket.id,
            PendingExport {
                owner,
                asset,
                action,
                history,
            },
        );
        println!(
            "pinora: export job {} started owner={owner:?} kind={kind:?}",
            ticket.id
        );
        Ok(ticket.id)
    }

    fn poll_ocr_jobs(&mut self) {
        let pin_assets: HashMap<PinId, AssetRef> = self
            .pins
            .values()
            .map(|pin| (pin.pin_id, pin.asset))
            .collect();
        let overlay_asset = self
            .overlay
            .as_ref()
            .and_then(|ov| overlay_current_asset(ov).map(|asset| (ov.session_id, asset)));
        let completions = self.ocr_jobs.poll(monotonic_ms(), |owner| match owner {
            JobOwner::Pin(pin_id) => pin_assets.get(&pin_id).copied(),
            JobOwner::Session(session_id) => overlay_asset
                .filter(|(id, _)| *id == session_id)
                .map(|(_, asset)| asset),
            JobOwner::History(_) => None,
        });

        for completion in completions {
            match completion {
                OcrJobCompletion::Completed { job, result } => {
                    println!(
                        "pinora: OCR ok owner={:?} — {} words",
                        job.owner,
                        result.word_count()
                    );
                    if !result.full_text.trim().is_empty() {
                        let text = result.full_text.clone();
                        if let Err(error) = self.submit_export_job(
                            job.owner,
                            job.asset,
                            ExportJobInput::CopyText { text },
                            PendingExportAction::CopyText,
                        ) {
                            eprintln!("pinora: text clipboard submit failed: {error}");
                        }
                    }
                    if let JobOwner::Pin(pin_id) = job.owner
                        && let Some(pin) = self.pins.values_mut().find(|pin| pin.pin_id == pin_id)
                        && pin.asset == job.asset
                    {
                        pin.ocr = Some(result);
                        pin.ocr_show_boxes = true;
                        pin.ocr_drag_start = None;
                        pin.ocr_selection = OcrTextSelection::default();
                        pin.window.request_redraw();
                    }
                }
                OcrJobCompletion::Failed {
                    job_id,
                    owner,
                    error,
                } => eprintln!("pinora: OCR job {job_id} failed owner={owner:?}: {error}"),
                OcrJobCompletion::Discarded { job_id, terminal } => {
                    println!("pinora: OCR job {job_id} discarded ({terminal:?})");
                }
            }
        }
    }

    fn poll_export_jobs(&mut self) {
        let pin_assets: HashMap<PinId, AssetRef> = self
            .pins
            .values()
            .map(|pin| (pin.pin_id, pin.asset))
            .collect();
        let overlay_asset = self
            .overlay
            .as_ref()
            .and_then(|ov| overlay_current_asset(ov).map(|asset| (ov.session_id, asset)));
        let pending_assets: HashMap<JobId, (JobOwner, AssetRef)> = self
            .pending_exports
            .iter()
            .map(|(job_id, pending)| (*job_id, (pending.owner, pending.asset)))
            .collect();

        let completions = self
            .export_jobs
            .poll(monotonic_ms(), |job_id, owner| match owner {
                JobOwner::Pin(pin_id) => pin_assets
                    .get(&pin_id)
                    .copied()
                    .or_else(|| pending_asset_for_owner(&pending_assets, job_id, owner)),
                JobOwner::Session(session_id) => overlay_asset
                    .filter(|(id, _)| *id == session_id)
                    .map(|(_, asset)| asset)
                    .or_else(|| pending_asset_for_owner(&pending_assets, job_id, owner)),
                JobOwner::History(_) => None,
            });
        for completion in completions {
            match completion {
                ExportJobCompletion::Completed { job } => {
                    match self.pending_exports.remove(&job.id) {
                        Some(PendingExport {
                            owner,
                            asset,
                            action: PendingExportAction::SavePng(path),
                            history,
                        }) => {
                            println!("pinora: saved {} -> {}", job.asset.image_id, path.display());
                            if let Some(candidate) = history
                                && owner == job.owner
                                && asset == job.asset
                                && candidate.owner == job.owner
                                && candidate.asset == job.asset
                            {
                                match record_history_candidate(
                                    &self.history_store,
                                    &mut self.history_index,
                                    candidate,
                                ) {
                                    Ok(inserted) => {
                                        println!(
                                            "pinora: history indexed active={} tombstoned={}",
                                            self.history_index.active_count(),
                                            inserted.evicted.len()
                                        );
                                        if let Some(export_dir) = self
                                            .runtime
                                            .as_ref()
                                            .map(|runtime| runtime.export_dir().clone())
                                        {
                                            match cleanup_history_tombstones(
                                                &self.history_store,
                                                &export_dir,
                                                &mut self.history_index,
                                            ) {
                                                Ok(cleanup) if cleanup.compacted_entries > 0 => {
                                                    println!(
                                                        "pinora: history cleanup removed={} missing={} protected={} failed={} compacted={}",
                                                        cleanup.removed_files,
                                                        cleanup.missing_files,
                                                        cleanup.protected_files,
                                                        cleanup.failed_files,
                                                        cleanup.compacted_entries
                                                    );
                                                }
                                                Ok(_) => {}
                                                Err(_) => eprintln!(
                                                    "pinora: history cleanup index write failed"
                                                ),
                                            }
                                        }
                                    }
                                    Err(_) => eprintln!(
                                        "pinora: history index write failed after managed PNG export"
                                    ),
                                }
                            }
                        }
                        Some(PendingExport {
                            action: PendingExportAction::CopyImage,
                            ..
                        }) => {
                            println!("pinora: copied image {}", job.asset.image_id);
                        }
                        Some(PendingExport {
                            action: PendingExportAction::CopyText,
                            ..
                        }) => {
                            println!("pinora: copied text for {}", job.asset.image_id);
                        }
                        None => println!("pinora: export job {} completed", job.id),
                    }
                }
                ExportJobCompletion::Failed {
                    job_id,
                    owner,
                    error,
                } => {
                    self.pending_exports.remove(&job_id);
                    eprintln!("pinora: export job {job_id} failed owner={owner:?}: {error}");
                }
                ExportJobCompletion::Discarded {
                    job_id,
                    owner,
                    terminal,
                } => {
                    self.pending_exports.remove(&job_id);
                    println!(
                        "pinora: export job {job_id} discarded owner={owner:?} ({terminal:?})"
                    );
                }
            }
        }
    }

    fn overlay_ocr(&mut self) {
        self.commit_overlay_draft();
        let image = match self.crop_overlay_image(true) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("pinora: OCR crop: {e}");
                return;
            }
        };
        let Some(ov) = self.overlay.as_ref() else {
            return;
        };
        let owner = JobOwner::Session(ov.session_id);
        let Some(asset) = overlay_current_asset(ov) else {
            eprintln!("pinora: OCR asset missing for overlay selection");
            return;
        };
        self.submit_ocr_job(owner, image, asset);
    }

    /// 对外部副作用冻结标注事务，避免预览草稿与任务 AssetRef 不一致。
    fn commit_overlay_draft(&mut self) {
        let Some(ov) = self.overlay.as_mut() else {
            return;
        };
        if ov.annotate.draft.is_none() {
            return;
        }
        let revision = ov.annotate.doc.revision();
        ov.annotate.commit();
        ov.annotate_dragging = false;
        if ov.annotate.doc.revision() != revision {
            ov.annotate_dirty = true;
            ov.needs_redraw = true;
        }
    }

    /// 若当前工具是取色器且光标位于已确认选区，采样原始像素并返回要复制的颜色文本。
    /// 外层在可变借用结束后才提交异步剪贴板任务。
    fn take_overlay_color_sample(&mut self) -> Option<Option<(JobOwner, AssetRef, String)>> {
        let ov = self.overlay.as_mut()?;
        if ov.phase != OverlayPhase::Ready || ov.annotate.tool != AnnotateTool::ColorPicker {
            return None;
        }
        let local = overlay_annotate_local(ov, ov.last_cursor)?;
        let Some(source_rect) = ov.active_src_rect else {
            return Some(None);
        };
        let Some(color) = sample_overlay_source_color(&ov.full_image, source_rect, local) else {
            return Some(None);
        };
        ov.annotate.set_color(color);
        ov.annotate.tool = ov.last_drawing_tool;
        ov.toolbar_chrome_dirty = true;
        ov.needs_redraw = true;
        let Some(asset) = overlay_current_asset(ov) else {
            return Some(None);
        };
        Some(Some((
            JobOwner::Session(ov.session_id),
            asset,
            color_to_hex(color),
        )))
    }

    /// 从当前 overlay 选区裁剪**原图像素**，可选烧录标注。
    fn crop_overlay_image(&self, bake: bool) -> Result<CaptureImage, PinoraError> {
        let ov = self
            .overlay
            .as_ref()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "overlay missing"))?;
        let src_rect = if let Some(r) = ov.active_src_rect {
            r
        } else {
            let disp = ov.session.try_confirm()?;
            buf_rect_to_src(disp, ov.buf_w, ov.buf_h, ov.src_w, ov.src_h)
        };
        let identity = ov.annotation_asset.ok_or_else(|| {
            PinoraError::new(
                ErrorCode::InvalidState,
                "overlay selection asset is not initialized",
            )
        })?;
        let crop = ov.full_image.crop_local(src_rect)?;
        let mut output = if bake && !ov.annotate.doc.is_empty() {
            bake_annotations(&crop, &ov.annotate.doc)
        } else if bake {
            if ov.annotate.draft.is_some() {
                let rgba = render_preview_rgba(&crop, &ov.annotate);
                let mut img = crop;
                if rgba.len() == img.pixels.bytes.len() {
                    img.pixels.bytes = rgba;
                }
                img
            } else {
                crop
            }
        } else {
            crop
        };
        identity.stamp(&mut output);
        Ok(output)
    }

    fn finish_overlay_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        action: OverlayFinish,
    ) -> Result<(), PinoraError> {
        self.commit_overlay_draft();
        let (src_rect, display_id, session_owner, asset, global, edit_pin_id) = {
            let ov = self
                .overlay
                .as_ref()
                .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "overlay missing"))?;
            let src_rect = if let Some(r) = ov.active_src_rect {
                r
            } else {
                match ov.session.try_confirm() {
                    Ok(disp) => buf_rect_to_src(disp, ov.buf_w, ov.buf_h, ov.src_w, ov.src_h),
                    Err(_) => {
                        println!("pinora: 尚无有效选区");
                        return Ok(());
                    }
                }
            };
            let asset = overlay_current_asset(ov).ok_or_else(|| {
                PinoraError::new(
                    ErrorCode::InvalidState,
                    "overlay selection asset is not initialized",
                )
            })?;
            let global = PixelRect::new(
                ov.display_origin.x.saturating_add(src_rect.origin.x),
                ov.display_origin.y.saturating_add(src_rect.origin.y),
                src_rect.size.width,
                src_rect.size.height,
            );
            (
                src_rect,
                ov.display_id.clone(),
                JobOwner::Session(ov.session_id),
                asset,
                global,
                ov.edit_pin_id,
            )
        };
        // 先裁切（仍持有 overlay），再立刻关窗
        let image = match self.crop_overlay_image(true) {
            Ok(image) => image,
            Err(error) => {
                // 贴图编辑的任何失败都恢复原窗口；普通截图仍按既有取消语义收尾。
                self.cancel_overlay();
                return Err(error);
            }
        };
        let position = PixelPoint::new(global.origin.x, global.origin.y);

        if let Some(ov) = self.overlay.take() {
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            self.export_jobs
                .close_owner(JobOwner::Session(ov.session_id));
            ov.window.set_visible(false);
            drop(ov);
        }
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        println!(
            "pinora: finish {action:?} {}x{} @ ({},{}) display={display_id:?}",
            src_rect.size.width, src_rect.size.height, global.origin.x, global.origin.y
        );

        if let Some(pin_id) = edit_pin_id {
            let pin_asset = match self.replace_pin_image(pin_id, image.clone()) {
                Ok(asset) => asset,
                Err(error) => {
                    self.restore_pin_visibility(pin_id);
                    return Err(error);
                }
            };
            match action {
                OverlayFinish::Copy => {
                    self.submit_export_job(
                        JobOwner::Pin(pin_id),
                        pin_asset,
                        ExportJobInput::CopyImage { image },
                        PendingExportAction::CopyImage,
                    )?;
                }
                OverlayFinish::Save => {
                    let path = self
                        .runtime
                        .as_ref()
                        .map(|rt| rt.export_dir().join(format!("{}.png", image.id)))
                        .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))?;
                    self.submit_export_job(
                        JobOwner::Pin(pin_id),
                        pin_asset,
                        ExportJobInput::SavePng {
                            image,
                            path: path.clone(),
                        },
                        PendingExportAction::SavePng(path),
                    )?;
                }
                // 贴图编辑中的“贴图”就是应用改动，保留同一 PinId，不创建新窗口。
                OverlayFinish::Pin => {}
            }
        } else {
            match action {
                OverlayFinish::Copy => {
                    self.submit_export_job(
                        session_owner,
                        asset,
                        ExportJobInput::CopyImage { image },
                        PendingExportAction::CopyImage,
                    )?;
                }
                OverlayFinish::Save => {
                    let path = self
                        .runtime
                        .as_ref()
                        .map(|rt| rt.export_dir().join(format!("{}.png", image.id)))
                        .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))?;
                    self.submit_export_job(
                        session_owner,
                        asset,
                        ExportJobInput::SavePng {
                            image,
                            path: path.clone(),
                        },
                        PendingExportAction::SavePng(path),
                    )?;
                }
                OverlayFinish::Pin => {
                    // 贴图：先出窗再异步保存/复制，避免主路径串行卡顿
                    self.open_pin_from_image(event_loop, image, position, true)?;
                }
            }
        }
        Ok(())
    }

    fn open_pin_from_image(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: CaptureImage,
        position: PixelPoint,
        export_after_open: bool,
    ) -> Result<(), PinoraError> {
        self.open_pin_from_image_with_pixels(event_loop, image, None, position, export_after_open)
    }

    fn open_pin_from_prepared_image(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: CaptureImage,
        pixels_xrgb: Vec<u32>,
        position: PixelPoint,
        export_after_open: bool,
    ) -> Result<(), PinoraError> {
        self.open_pin_from_image_with_pixels(
            event_loop,
            image,
            Some(pixels_xrgb),
            position,
            export_after_open,
        )
    }

    fn open_pin_from_image_with_pixels(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: CaptureImage,
        pixels_xrgb: Option<Vec<u32>>,
        position: PixelPoint,
        export_after_open: bool,
    ) -> Result<(), PinoraError> {
        let rt = self
            .runtime
            .as_mut()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))?;
        let pin = rt.dispatch(Command::create_pin(image.clone(), position))?;
        let pin_id = pin
            .events
            .iter()
            .find_map(|e| match e.event.kind {
                DomainEventKind::PinCreated { pin_id, .. } => Some(pin_id),
                _ => None,
            })
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "missing PinCreated"))?;
        let asset = AssetRef::initial(image.id);
        let export_image = image.clone();

        println!(
            "pinora: pin {pin_id} ({}x{}) — L锁定 [ ]透明度 O识别 T词框 滚轮 Esc{}",
            image.pixels.size.width,
            image.pixels.size.height,
            if tesseract_available() {
                ""
            } else {
                "（未装 tesseract）"
            }
        );

        // 先弹出贴图窗；导出/剪贴板放到后面，避免挡住贴图
        self.spawn_pin(
            event_loop,
            pin_id,
            image,
            PinPresentation {
                position,
                scale: 1.0,
                opacity: self.default_pin_opacity,
                pixels_xrgb,
            },
        )?;
        self.mode = Mode::Idle;
        self.resume_frame_cache();

        if !export_after_open {
            return Ok(());
        }

        let owner = JobOwner::Pin(pin_id);
        if let Some(path) = self
            .runtime
            .as_ref()
            .map(|rt| rt.export_dir().join(format!("{}.png", export_image.id)))
            && let Err(error) = self.submit_export_job(
                owner,
                asset,
                ExportJobInput::SavePng {
                    image: export_image.clone(),
                    path: path.clone(),
                },
                PendingExportAction::SavePng(path),
            )
        {
            eprintln!("pinora: save submit failed: {error}");
        }
        if let Err(error) = self.submit_export_job(
            owner,
            asset,
            ExportJobInput::CopyImage {
                image: export_image,
            },
            PendingExportAction::CopyImage,
        ) {
            eprintln!("pinora: image clipboard submit failed: {error}");
        }
        Ok(())
    }

    fn cancel_overlay(&mut self) {
        let edited_pin = if let Some(ov) = self.overlay.take() {
            let edited_pin = ov.edit_pin_id;
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            self.export_jobs
                .close_owner(JobOwner::Session(ov.session_id));
            ov.window.set_visible(false);
            edited_pin
        } else {
            None
        };
        if let Some(pin_id) = edited_pin {
            self.restore_pin_visibility(pin_id);
        }
        // Esc 只取消选区，绝不自动再截；再截仅 F2 / Ctrl+N
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        println!("pinora: selection cancelled (F2/Ctrl+N 再截，Ctrl+Q 退出)");
        if let Some(pin) = self.pins.values().next() {
            pin.window.focus_window();
        }
    }

    fn spawn_pin(
        &mut self,
        event_loop: &ActiveEventLoop,
        pin_id: PinId,
        image: CaptureImage,
        presentation: PinPresentation,
    ) -> Result<(), PinoraError> {
        let PinPresentation {
            position,
            scale,
            opacity,
            pixels_xrgb: prepared_pixels_xrgb,
        } = presentation;
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let image_size = image.size();
        let (w, h) = scaled_window_size(image_size, scale);
        let expected_pixels = pixel_count(image_size.width, image_size.height, "pin image")?;
        let pixels_xrgb = match prepared_pixels_xrgb {
            Some(pixels) if pixels.len() == expected_pixels => pixels,
            Some(_) => {
                return Err(PinoraError::new(
                    ErrorCode::InvalidState,
                    "prepared pin pixels do not match image dimensions",
                ));
            }
            None => rgba_to_xrgb(&image.pixels.bytes),
        };

        // 唯一标题，便于 KWin 脚本精确匹配
        let title = format!("Pinora-pin-{pin_id}");
        // 先 Normal 层级 + 不可见：建好、画完、钉位后，再 AlwaysOnTop。
        // 这样定位过程中不会盖过仍显示的 overlay（避免中央闪一下）。
        let attrs = Window::default_attributes()
            .with_title(title.clone())
            .with_inner_size(PhysicalSize::new(w, h))
            .with_position(PhysicalPosition::new(position.x, position.y))
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(false);

        let window =
            window_policy::create_auxiliary_window(event_loop, AuxiliaryWindowKind::Pin, attrs)
                .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin window: {e}")))?;
        let window = Rc::new(window);
        window.set_outer_position(PhysicalPosition::new(position.x, position.y));

        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin surface: {e}")))?;
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            surface.resize(nw, nh).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("pin surface resize: {e}"))
            })?;
        }

        let id = window.id();
        let asset = AssetRef::initial(image.id);
        self.pins.insert(
            id,
            PinWin {
                pin_id,
                title: title.clone(),
                image,
                asset,
                pixels_xrgb,
                render_cache: None,
                scale,
                opacity: opacity.clamp(0.15, 1.0),
                locked: false,
                always_on_top: true,
                context_menu: None,
                visible: true,
                window: window.clone(),
                surface,
                ocr: None,
                ocr_show_boxes: true,
                cursor_position: (0.0, 0.0),
                ocr_drag_start: None,
                ocr_selection: OcrTextSelection::default(),
            },
        );

        // 同步画第一帧，避免显示时白屏
        if let Err(e) = self.paint_pin(id) {
            eprintln!("pinora: initial pin paint: {e}");
        }

        // map 进合成器以便 KWin 能找到（仍在 overlay 下面）
        window_policy::show_auxiliary_window(AuxiliaryWindowKind::Pin, &window, &title);
        if crate::kwin_place::kwin_available() {
            if let Err(e) =
                crate::kwin_place::place_window_by_title_sync(&title, position.x, position.y, w, h)
            {
                eprintln!("pinora: kwin sync place: {e}");
            }
            crate::kwin_place::place_window_by_title(&title, position.x, position.y, w, h, 50);
            crate::kwin_place::place_window_by_title(&title, position.x, position.y, w, h, 150);
        } else {
            window.set_outer_position(PhysicalPosition::new(position.x, position.y));
        }

        // 钉位后再置顶，准备在 overlay 撤掉后露出来
        window.set_window_level(WindowLevel::AlwaysOnTop);
        window.focus_window();
        window.request_redraw();

        println!(
            "pinora: pin window ready ({w}x{h}) at ({}, {}) title={title}",
            position.x, position.y
        );
        Ok(())
    }

    fn handle_pin_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_pin(window_id);
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.is_quit_key(&event) {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                if self.handle_capture_shortcut(event_loop, &event) {
                    return;
                }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    self.close_pin(window_id);
                    return;
                }
                // L：锁定；[ ]：透明度；O：OCR；T：词框
                if let Key::Character(c) = &event.logical_key {
                    if c == "l" || c == "L" {
                        self.toggle_pin_locked(window_id);
                        return;
                    }
                    if c == "[" {
                        self.nudge_pin_opacity(window_id, -0.1);
                        return;
                    }
                    if c == "]" {
                        self.nudge_pin_opacity(window_id, 0.1);
                        return;
                    }
                    if c == "o" || c == "O" {
                        self.run_pin_ocr(window_id);
                        return;
                    }
                    if (c == "t" || c == "T")
                        && let Some(pin) = self.pins.get_mut(&window_id)
                    {
                        pin.ocr_show_boxes = !pin.ocr_show_boxes;
                        if !pin.ocr_show_boxes {
                            pin.ocr_drag_start = None;
                            pin.ocr_selection = OcrTextSelection::default();
                        }
                        println!(
                            "pinora: pin {} OCR boxes {}",
                            pin.pin_id,
                            if pin.ocr_show_boxes { "ON" } else { "OFF" }
                        );
                        pin.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let menu_hit = self.pins.get(&window_id).and_then(|pin| {
                    let point = PixelPoint::new(
                        pin.cursor_position.0.round() as i32,
                        pin.cursor_position.1.round() as i32,
                    );
                    pin.context_menu
                        .as_ref()
                        .map(|menu| (menu.hit_test(point), menu.contains(point)))
                });
                let Some((menu_action, menu_contains_cursor)) = menu_hit else {
                    // 菜单没有打开，继续正常贴图交互。
                    self.handle_pin_left_press(window_id);
                    return;
                };
                if let Some(action) = menu_action {
                    self.handle_pin_menu_action(event_loop, window_id, action);
                    return;
                }
                if menu_contains_cursor {
                    // 禁用项目没有副作用，也不应意外开始拖动贴图。
                    return;
                }
                if let Some(pin) = self.pins.get_mut(&window_id)
                    && pin.context_menu.take().is_some()
                {
                    pin.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let Some(pin) = self.pins.get_mut(&window_id) else {
                    return;
                };
                let size = pin.window.inner_size();
                let anchor = PixelPoint::new(
                    pin.cursor_position.0.round() as i32,
                    pin.cursor_position.1.round() as i32,
                );
                pin.context_menu = Some(PinContextMenu::open(
                    anchor,
                    size.width,
                    size.height,
                    pin.locked,
                ));
                pin.window.request_redraw();
                self.drag_pin = None;
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let (handled, selection) = self.finish_pin_text_selection(window_id);
                if handled {
                    if let Some((owner, asset, text)) = selection
                        && let Err(error) = self.submit_export_job(
                            owner,
                            asset,
                            ExportJobInput::CopyText { text },
                            PendingExportAction::CopyText,
                        )
                    {
                        eprintln!("pinora: selected OCR text submit failed: {error}");
                    }
                    return;
                }
                self.drag_pin = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let selecting = if let Some(pin) = self.pins.get_mut(&window_id) {
                    pin.cursor_position = (position.x, position.y);
                    let selecting = pin.ocr_drag_start.is_some();
                    if selecting {
                        pin.window.request_redraw();
                    }
                    selecting
                } else {
                    false
                };
                if selecting {
                    return;
                }
                // 回退拖动（X11 / drag_window 失败时）
                let Some(id) = self.drag_pin else {
                    return;
                };
                if id != window_id {
                    return;
                }
                let Some(pin) = self.pins.get(&window_id) else {
                    return;
                };
                if let Ok(outer) = pin.window.outer_position() {
                    // 简单跟随：将窗口左上角移到近似位置
                    pin.window.set_outer_position(PhysicalPosition::new(
                        outer.x + position.x as i32 / 8,
                        outer.y + position.y as i32 / 8,
                    ));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                if steps.abs() < f64::EPSILON {
                    return;
                }
                let Some(pin) = self.pins.get_mut(&window_id) else {
                    return;
                };
                if pin.locked {
                    return;
                }
                let factor = if steps > 0.0 { 1.1_f64 } else { 1.0 / 1.1 };
                pin.scale = (pin.scale * factor).clamp(0.1, 8.0);
                pin.render_cache = None;
                let (w, h) = scaled_window_size(pin.image.size(), pin.scale);
                let _ = pin.window.request_inner_size(PhysicalSize::new(w, h));
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = pin.surface.resize(nw, nh);
                }
                pin.window.request_redraw();
                self.sync_pin_transform(window_id);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint_pin(window_id) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(pin) = self.pins.get_mut(&window_id) {
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(size.width.max(1)),
                        NonZeroU32::new(size.height.max(1)),
                    ) {
                        let _ = pin.surface.resize(w, h);
                    }
                    pin.render_cache = None;
                    pin.window.request_redraw();
                }
            }
            WindowEvent::Focused(true) => {
                // 确保键盘可用
            }
            _ => {}
        }
    }

    fn run_pin_ocr(&mut self, window_id: WindowId) {
        let Some(pin) = self.pins.get(&window_id) else {
            return;
        };
        let pin_id = pin.pin_id;
        let image = pin.image.clone();
        let asset = pin.asset;
        self.submit_ocr_job(JobOwner::Pin(pin_id), image, asset);
    }

    fn handle_pin_menu_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        action: PinMenuAction,
    ) {
        if let Some(pin) = self.pins.get_mut(&window_id) {
            pin.context_menu = None;
            pin.window.request_redraw();
        }
        match action {
            PinMenuAction::Copy => self.copy_pin_image(window_id),
            PinMenuAction::Ocr => self.run_pin_ocr(window_id),
            PinMenuAction::Edit => self.begin_pin_edit(event_loop, window_id),
            PinMenuAction::ToggleLock => self.toggle_pin_locked(window_id),
            PinMenuAction::OpacityDown => self.nudge_pin_opacity(window_id, -0.1),
            PinMenuAction::OpacityUp => self.nudge_pin_opacity(window_id, 0.1),
            PinMenuAction::ToggleAlwaysOnTop => self.toggle_pin_always_on_top(window_id),
            PinMenuAction::Save => self.save_pin_image(window_id),
            PinMenuAction::Close => self.close_pin(window_id),
        }
    }

    fn handle_pin_left_press(&mut self, window_id: WindowId) {
        if let Some(pin) = self.pins.get(&window_id) {
            if self.modifiers.control_key() && pin.ocr_show_boxes && pin.ocr.is_some() {
                let cursor = pin.cursor_position;
                if let Some(pin) = self.pins.get_mut(&window_id) {
                    pin.ocr_drag_start = Some(cursor);
                    pin.ocr_selection = OcrTextSelection::default();
                    pin.window.request_redraw();
                }
                self.drag_pin = None;
                return;
            }
            if pin.locked {
                println!("pinora: pin locked — press L to unlock");
                return;
            }
            // Wayland 下用协议级拖动
            if let Err(error) = pin.window.drag_window() {
                eprintln!("pinora: drag_window failed: {error:?}");
                self.drag_pin = Some(window_id);
            } else {
                self.drag_pin = None;
            }
        }
    }

    fn copy_pin_image(&mut self, window_id: WindowId) {
        let Some(pin) = self.pins.get(&window_id) else {
            return;
        };
        let owner = JobOwner::Pin(pin.pin_id);
        let asset = pin.asset;
        let image = pin.image.clone();
        if let Err(error) = self.submit_export_job(
            owner,
            asset,
            ExportJobInput::CopyImage { image },
            PendingExportAction::CopyImage,
        ) {
            eprintln!("pinora: pin copy submit failed: {error}");
        }
    }

    fn save_pin_image(&mut self, window_id: WindowId) {
        let Some(pin) = self.pins.get(&window_id) else {
            return;
        };
        let Some(path) = self
            .runtime
            .as_ref()
            .map(|runtime| runtime.export_dir().join(format!("{}.png", pin.image.id)))
        else {
            eprintln!("pinora: pin save skipped because runtime is unavailable");
            return;
        };
        let owner = JobOwner::Pin(pin.pin_id);
        let asset = pin.asset;
        let image = pin.image.clone();
        if let Err(error) = self.submit_export_job(
            owner,
            asset,
            ExportJobInput::SavePng {
                image,
                path: path.clone(),
            },
            PendingExportAction::SavePng(path),
        ) {
            eprintln!("pinora: pin save submit failed: {error}");
        }
    }

    fn toggle_pin_locked(&mut self, window_id: WindowId) {
        let Some((pin_id, locked)) = self
            .pins
            .get(&window_id)
            .map(|pin| (pin.pin_id, !pin.locked))
        else {
            return;
        };
        let result = self
            .runtime
            .as_mut()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))
            .and_then(|runtime| runtime.dispatch(Command::set_pin_locked(pin_id, locked)));
        if let Err(error) = result {
            eprintln!("pinora: pin lock update failed: {error}");
            return;
        }
        if let Some(pin) = self.pins.get_mut(&window_id) {
            pin.locked = locked;
            pin.window.request_redraw();
        }
    }

    fn toggle_pin_always_on_top(&mut self, window_id: WindowId) {
        let Some((pin_id, always_on_top)) = self
            .pins
            .get(&window_id)
            .map(|pin| (pin.pin_id, !pin.always_on_top))
        else {
            return;
        };
        let result = self
            .runtime
            .as_mut()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))
            .and_then(|runtime| {
                runtime.dispatch(Command::set_pin_always_on_top(pin_id, always_on_top))
            });
        if let Err(error) = result {
            eprintln!("pinora: pin level update failed: {error}");
            return;
        }
        if let Some(pin) = self.pins.get_mut(&window_id) {
            pin.always_on_top = always_on_top;
            pin.window.set_window_level(if always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
            pin.window.request_redraw();
        }
    }

    fn begin_pin_edit(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        let Some((pin_id, image, locked)) = self
            .pins
            .get(&window_id)
            .map(|pin| (pin.pin_id, pin.image.clone(), pin.locked))
        else {
            return;
        };
        if locked {
            return;
        }
        if let Some(pin) = self.pins.get_mut(&window_id) {
            pin.visible = false;
            pin.window.set_visible(false);
        }
        let target = pin_edit_target(&image, pin_id);
        if let Err(error) =
            self.open_overlay_with_preview(event_loop, prepare_preview(image), target)
        {
            self.restore_pin_visibility(pin_id);
            eprintln!("pinora: pin editor open failed: {error}");
        }
    }

    fn restore_pin_visibility(&mut self, pin_id: PinId) {
        if let Some(pin) = self.pins.values_mut().find(|pin| pin.pin_id == pin_id) {
            pin.visible = true;
            window_policy::show_auxiliary_window(AuxiliaryWindowKind::Pin, &pin.window, &pin.title);
            pin.window.request_redraw();
        }
    }

    fn replace_pin_image(
        &mut self,
        pin_id: PinId,
        image: CaptureImage,
    ) -> Result<AssetRef, PinoraError> {
        let (old_asset, scale) = self
            .pins
            .values()
            .find(|pin| pin.pin_id == pin_id)
            .map(|pin| (pin.asset, pin.scale))
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, "edited pin is no longer open"))?;
        let generation = old_asset.generation.advance().ok_or_else(|| {
            PinoraError::new(
                ErrorCode::Internal,
                "edited pin asset generation is exhausted",
            )
        })?;
        let asset = AssetRef::new(image.id, generation);
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "runtime missing"))?;
        runtime.dispatch(Command::replace_pin_image(pin_id, image.clone()))?;

        self.ocr_jobs.close_owner(JobOwner::Pin(pin_id));
        self.export_jobs.close_owner(JobOwner::Pin(pin_id));
        let pin = self
            .pins
            .values_mut()
            .find(|pin| pin.pin_id == pin_id)
            .expect("edited pin was present before runtime replacement");
        pin.image = image;
        pin.asset = asset;
        pin.pixels_xrgb = rgba_to_xrgb(&pin.image.pixels.bytes);
        pin.render_cache = None;
        pin.ocr = None;
        pin.ocr_selection = OcrTextSelection::default();
        pin.ocr_drag_start = None;
        pin.context_menu = None;
        let (width, height) = scaled_window_size(pin.image.size(), scale);
        let _ = pin
            .window
            .request_inner_size(PhysicalSize::new(width, height));
        if let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            let _ = pin.surface.resize(width, height);
        }
        pin.visible = true;
        window_policy::show_auxiliary_window(AuxiliaryWindowKind::Pin, &pin.window, &pin.title);
        pin.window.request_redraw();
        Ok(asset)
    }

    fn finish_pin_text_selection(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Option<(JobOwner, AssetRef, String)>) {
        let Some(pin) = self.pins.get_mut(&window_id) else {
            return (false, None);
        };
        let Some(start) = pin.ocr_drag_start.take() else {
            return (false, None);
        };
        let Some(ocr) = pin.ocr.as_ref() else {
            return (true, None);
        };
        let size = pin.window.inner_size();
        let region = selection_rect_from_window_points(
            start,
            pin.cursor_position,
            size.width,
            size.height,
            pin.image.size().width,
            pin.image.size().height,
        );
        let selection = ocr.select_words(region);
        let text = ocr.text_for_selection(&selection);
        pin.ocr_selection = selection;
        pin.window.request_redraw();
        if text.trim().is_empty() {
            (true, None)
        } else {
            (true, Some((JobOwner::Pin(pin.pin_id), pin.asset, text)))
        }
    }

    fn nudge_pin_opacity(&mut self, window_id: WindowId, delta: f64) {
        let Some(pin) = self.pins.get_mut(&window_id) else {
            return;
        };
        if pin.locked {
            return;
        }
        pin.opacity = (pin.opacity + delta).clamp(0.15, 1.0);
        pin.render_cache = None;
        println!(
            "pinora: pin {} opacity {:.0}%",
            pin.pin_id,
            pin.opacity * 100.0
        );
        // softbuffer 无真透明：用棋盘/变暗近似半透明观感
        pin.window.request_redraw();
        self.sync_pin_transform(window_id);
    }

    fn sync_pin_transform(&mut self, window_id: WindowId) {
        let Some(pin) = self.pins.get(&window_id) else {
            return;
        };
        let pin_id = pin.pin_id;
        let transform = PinTransform {
            position: PixelPoint::new(0, 0), // 位置由窗口管理，状态仅存缩放/透明
            scale: pin.scale,
            rotation_deg: 0.0,
            opacity: pin.opacity,
        }
        .clamped();
        if let Some(rt) = self.runtime.as_mut()
            && let Err(error) = rt.dispatch(Command::set_pin_transform(pin_id, transform))
        {
            eprintln!("pinora: pin transform update failed: {error}");
        }
    }

    fn close_pin(&mut self, window_id: WindowId) {
        if let Some(pin) = self.pins.remove(&window_id) {
            self.ocr_jobs.close_owner(JobOwner::Pin(pin.pin_id));
            self.export_jobs.close_owner(JobOwner::Pin(pin.pin_id));
            println!("pinora: pin {} closed", pin.pin_id);
            if let Some(rt) = self.runtime.as_mut() {
                let _ = rt.dispatch(Command::close_pin(pin.pin_id));
            }
        }
        self.drag_pin = None;
        if self.pins.is_empty() && self.overlay.is_none() {
            // Esc 关闭贴图 ≠ 再截图
            self.mode = Mode::Idle;
            println!("pinora: all pins closed (F2/Ctrl+N 再截，Ctrl+Q 退出)");
        }
    }

    fn paint_pin(&mut self, window_id: WindowId) -> Result<(), PinoraError> {
        let Some(pin) = self.pins.get_mut(&window_id) else {
            return Ok(());
        };
        let size = pin.window.inner_size();
        let bw = size.width.max(1);
        let bh = size.height.max(1);
        // 先对齐 surface 与窗口尺寸，避免 buffer 长度不一致导致退出
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(bw), NonZeroU32::new(bh))
            && let Err(e) = pin.surface.resize(nw, nh)
        {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                format!("pin surface resize: {e}"),
            ));
        }
        ensure_pin_render_cache(pin, bw, bh)?;
        let bw = bw as usize;
        let bh = bh as usize;
        let sw = pin.image.pixels.size.width as usize;
        let sh = pin.image.pixels.size.height as usize;
        let locked = pin.locked;
        let always_on_top = pin.always_on_top;
        let context_menu = pin.context_menu.clone();
        let show_ocr = pin.ocr_show_boxes;
        let ocr_selection = pin.ocr_selection.clone();
        let ocr_drag = pin.ocr_drag_start.map(|start| (start, pin.cursor_position));
        let ocr_boxes: Vec<(PixelRect, bool)> = if show_ocr {
            let selection = &ocr_selection;
            pin.ocr
                .as_ref()
                .map(|r| {
                    r.lines
                        .iter()
                        .enumerate()
                        .flat_map(|(line_index, line)| {
                            line.words
                                .iter()
                                .enumerate()
                                .map(move |(word_index, word)| {
                                    (
                                        word.bbox,
                                        selection.contains(OcrWordRef {
                                            line_index,
                                            word_index,
                                        }),
                                    )
                                })
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let cached_pixels = &pin
            .render_cache
            .as_ref()
            .expect("render cache is populated by ensure_pin_render_cache")
            .pixels;
        let mut buffer = pin
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin buffer: {e}")))?;
        if buffer.len() < bw * bh {
            // 尺寸尚未就绪时跳过本帧，不崩溃退出
            eprintln!(
                "pinora: pin buffer skip (have {} need {})",
                buffer.len(),
                bw * bh
            );
            return Ok(());
        }
        buffer[..bw * bh].copy_from_slice(cached_pixels);
        // OCR 词框（图像坐标 → 窗口坐标）
        if !ocr_boxes.is_empty() && sw > 0 && sh > 0 {
            let sx = bw as f64 / sw as f64;
            let sy = bh as f64 / sh as f64;
            for (rect, selected) in ocr_boxes {
                let x0 = (rect.origin.x as f64 * sx).round() as i32;
                let y0 = (rect.origin.y as f64 * sy).round() as i32;
                let x1 = (rect.right() as f64 * sx).round() as i32;
                let y1 = (rect.bottom() as f64 * sy).round() as i32;
                draw_rect_outline_xrgb(
                    &mut buffer[..bw * bh],
                    bw,
                    bh,
                    PixelPoint::new(x0, y0),
                    PixelPoint::new(x1.max(x0 + 1), y1.max(y0 + 1)),
                    if selected {
                        0x00_FF_B0_20
                    } else {
                        0x00_22_EE_66
                    },
                );
            }
        }
        if let Some((start, end)) = ocr_drag {
            let drag_rect = window_rect_from_points(start, end);
            draw_rect_outline_xrgb(
                &mut buffer[..bw * bh],
                bw,
                bh,
                PixelPoint::new(drag_rect.origin.x, drag_rect.origin.y),
                PixelPoint::new(drag_rect.right(), drag_rect.bottom()),
                0x00_FF_B0_20,
            );
        }
        let border = if locked {
            0x00_CC_44_22 // 锁定：偏红边
        } else {
            0x00_40_A0_FF
        };
        draw_border(&mut buffer[..bw * bh], bw, bh, border);
        if let Some(menu) = context_menu.as_ref() {
            pin_context_menu::paint(&mut buffer[..bw * bh], bw, bh, menu, locked, always_on_top);
        }
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin present: {e}")))?;
        Ok(())
    }

    fn is_quit_key(&self, event: &winit::event::KeyEvent) -> bool {
        self.modifiers.control_key()
            && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyQ))
    }

    fn is_new_capture_key(&self, event: &winit::event::KeyEvent) -> bool {
        // Wayland 上 logical F 键有时不是 NamedKey::F2，同时认物理键。
        let f2 = matches!(event.logical_key, Key::Named(NamedKey::F2))
            || matches!(event.physical_key, PhysicalKey::Code(KeyCode::F2));
        let ctrl_n = self.modifiers.control_key()
            && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyN));
        f2 || ctrl_n
    }

    fn is_full_display_capture_key(&self, event: &winit::event::KeyEvent) -> bool {
        matches!(event.logical_key, Key::Named(NamedKey::F3))
            || matches!(event.physical_key, PhysicalKey::Code(KeyCode::F3))
    }

    fn handle_capture_shortcut(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &winit::event::KeyEvent,
    ) -> bool {
        if self.is_full_display_capture_key(event) {
            self.request_full_display_capture(event_loop);
            true
        } else if self.is_new_capture_key(event) {
            self.request_new_capture(event_loop);
            true
        } else {
            false
        }
    }
}

fn handle_overlay_key(
    modifiers: ModifiersState,
    ov: &mut OverlayState,
    event: &winit::event::KeyEvent,
) {
    let step = if modifiers.shift_key() { 10 } else { 1 };

    if ov.phase == OverlayPhase::Ready && ov.annotate.is_text_editing() {
        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => {
                ov.annotate.text_backspace();
                ov.needs_redraw = true;
                return;
            }
            Key::Character(c)
                if !modifiers.control_key()
                    && !modifiers.alt_key()
                    && !c.chars().any(|ch| ch.is_control()) =>
            {
                ov.annotate.text_push(c.as_str());
                ov.needs_redraw = true;
                return;
            }
            _ => {}
        }
    }

    if ov.phase == OverlayPhase::Ready
        && let Key::Character(character) = &event.logical_key
        && let Some(action) =
            annotation_history_action(modifiers.control_key(), modifiers.shift_key(), character)
    {
        let changed = match action {
            AnnotationHistoryAction::Undo => ov.annotate.doc.undo().is_some(),
            AnnotationHistoryAction::Redo => ov.annotate.doc.redo().is_some(),
        };
        if changed {
            ov.annotate_dirty = true;
            ov.needs_redraw = true;
        }
        return;
    }

    match &event.logical_key {
        Key::Named(NamedKey::ArrowLeft) => {
            ov.session.nudge(-step, 0);
            refresh_overlay_ready(ov);
        }
        Key::Named(NamedKey::ArrowRight) => {
            ov.session.nudge(step, 0);
            refresh_overlay_ready(ov);
        }
        Key::Named(NamedKey::ArrowUp) => {
            ov.session.nudge(0, -step);
            refresh_overlay_ready(ov);
        }
        Key::Named(NamedKey::ArrowDown) => {
            ov.session.nudge(0, step);
            refresh_overlay_ready(ov);
        }
        Key::Character(c)
            if (c == "c" || c == "C")
                && !modifiers.control_key()
                && ov.phase == OverlayPhase::Ready =>
        {
            ov.annotate.cycle_color();
            ov.annotate_dirty = true;
            ov.needs_redraw = true;
            println!("pinora: stroke color rgba{:?}", ov.annotate.color);
        }
        Key::Character(c) if (c == "+" || c == "=") && ov.phase == OverlayPhase::Ready => {
            ov.annotate.stroke_up();
            ov.annotate_dirty = true;
            ov.needs_redraw = true;
        }
        Key::Character(c) if (c == "-" || c == "_") && ov.phase == OverlayPhase::Ready => {
            ov.annotate.stroke_down();
            ov.annotate_dirty = true;
            ov.needs_redraw = true;
        }
        Key::Character(c)
            if (c == "f" || c == "F")
                && !modifiers.control_key()
                && ov.phase == OverlayPhase::Ready =>
        {
            let enabled = ov.annotate.toggle_shape_fill();
            ov.toolbar_chrome_dirty = true;
            ov.needs_redraw = true;
            println!("pinora: shape fill enabled={enabled}");
        }
        Key::Character(c) if c == "1" || c == "r" || c == "R" => {
            set_overlay_tool(ov, AnnotateTool::Rect);
            println!("pinora: tool = Rect");
        }
        Key::Character(c) if c == "q" || c == "Q" => {
            set_overlay_tool(ov, AnnotateTool::RoundedRect);
            println!("pinora: tool = RoundedRect");
        }
        Key::Character(c) if c == "l" || c == "L" => {
            set_overlay_tool(ov, AnnotateTool::Line);
            println!("pinora: tool = Line");
        }
        Key::Character(c) if c == "2" || c == "a" || c == "A" => {
            set_overlay_tool(ov, AnnotateTool::Arrow);
            println!("pinora: tool = Arrow");
        }
        Key::Character(c) if c == "3" => {
            set_overlay_tool(ov, AnnotateTool::Pen);
            println!("pinora: tool = Pen");
        }
        Key::Character(c) if c == "4" || c == "e" || c == "E" => {
            set_overlay_tool(ov, AnnotateTool::Ellipse);
            println!("pinora: tool = Ellipse");
        }
        Key::Character(c) if c == "n" || c == "N" => {
            set_overlay_tool(ov, AnnotateTool::Number);
            println!("pinora: tool = Number");
        }
        Key::Character(c) if c == "5" || c == "m" || c == "M" => {
            set_overlay_tool(ov, AnnotateTool::Mosaic);
            println!("pinora: tool = Mosaic");
        }
        Key::Character(c) if c == "b" || c == "B" => {
            set_overlay_tool(ov, AnnotateTool::Blur);
            println!("pinora: tool = Blur");
        }
        Key::Character(c) if c == "6" || c == "t" || c == "T" => {
            set_overlay_tool(ov, AnnotateTool::Text);
            println!("pinora: tool = Text");
        }
        Key::Character(c) if c == "i" || c == "I" => {
            set_overlay_tool(ov, AnnotateTool::ColorPicker);
            println!("pinora: tool = ColorPicker");
        }
        _ => {}
    }
}

fn overlay_click_finishes_copy(tool: AnnotateTool, is_double_click: bool) -> bool {
    is_double_click && tool != AnnotateTool::Number
}

fn set_overlay_tool(ov: &mut OverlayState, tool: AnnotateTool) {
    if tool != AnnotateTool::ColorPicker {
        ov.last_drawing_tool = tool;
    }
    ov.annotate.tool = tool;
    ov.toolbar_chrome_dirty = true;
    ov.needs_redraw = true;
}

fn refresh_overlay_ready(ov: &mut OverlayState) {
    if let Ok(sel) = ov.session.try_confirm()
        && ov.phase == OverlayPhase::Ready
    {
        ov.toolbar = layout_toolbar(sel, ov.buf_w, ov.buf_h);
        let src_sel = buf_rect_to_src(sel, ov.buf_w, ov.buf_h, ov.src_w, ov.src_h);
        let source_changed = ov.active_src_rect != Some(src_sel);
        if source_changed {
            ov.active_src_rect = Some(src_sel);
            ov.annotation_asset = Some(OverlayAssetIdentity::new());
            ov.annotate_cache = None;
            ov.annotate_preview_cache.clear();
            ov.annotate_dirty = true;
        }
        if ov.annotate.image_w != src_sel.size.width || ov.annotate.image_h != src_sel.size.height {
            let tool = ov.annotate.tool;
            let color = ov.annotate.color;
            let stroke = ov.annotate.stroke;
            let shape_fill_enabled = ov.annotate.shape_fill_enabled();
            ov.annotate = AnnotateSession::new(src_sel.size.width, src_sel.size.height);
            ov.annotate.tool = tool;
            ov.annotate.color = color;
            ov.annotate.stroke = stroke;
            if shape_fill_enabled {
                ov.annotate.toggle_shape_fill();
            }
            ov.annotate_cache = None;
            ov.annotate_preview_cache.clear();
            ov.annotate_dirty = true;
        }
    }
    ov.needs_redraw = true;
}

fn paint_overlay(ov: &mut OverlayState) -> Result<(), PinoraError> {
    // softbuffer 缓冲 = 截图尺寸（与 frame 一致）；鼠标用 win_* 做坐标映射
    let img_w = ov.buf_w as usize;
    let img_h = ov.buf_h as usize;
    let new_rect = ov.session.preview_rect();
    let new_tb = if ov.phase == OverlayPhase::Ready && !ov.toolbar.is_empty() {
        toolbar_bounds(&ov.toolbar)
    } else {
        None
    };

    let sel_changed = ov.last_drawn_rect != new_rect;
    let tb_layout_changed = ov.last_toolbar_bounds != new_tb;
    let chrome_only = ov.toolbar_chrome_dirty
        && !ov.annotate_dirty
        && !sel_changed
        && !tb_layout_changed
        && ov.buffer_synced;

    let mut damage: Vec<PixelRect> = Vec::with_capacity(4);

    if chrome_only {
        if let Some(tb) = ov.last_toolbar_bounds.or(new_tb) {
            blit_rect(&mut ov.frame, &ov.dimmed, img_w, img_h, tb);
            if ov.phase == OverlayPhase::Ready && !ov.toolbar.is_empty() {
                paint_toolbar(
                    &mut ov.frame,
                    img_w,
                    img_h,
                    &ov.toolbar,
                    ov.annotate.tool,
                    ov.annotate.color,
                    ov.annotate.shape_fill_enabled(),
                );
            }
            damage.push(tb);
        }
        ov.toolbar_chrome_dirty = false;
    } else if ov.annotate_dirty || sel_changed || tb_layout_changed || !ov.buffer_synced {
        if let Some(old) = ov.last_drawn_rect {
            let expanded = expand_rect(old, 3, ov.buf_w, ov.buf_h);
            blit_rect(&mut ov.frame, &ov.dimmed, img_w, img_h, expanded);
            damage.push(expanded);
        }
        if let Some(old_tb) = ov.last_toolbar_bounds {
            blit_rect(&mut ov.frame, &ov.dimmed, img_w, img_h, old_tb);
            damage.push(old_tb);
        }

        if let Some(rect) = new_rect {
            let use_annotate = ov.phase == OverlayPhase::Ready
                && (!ov.annotate.doc.is_empty() || ov.annotate.draft.is_some())
                && ov.active_src_rect.is_some_and(|r| {
                    ov.annotate.image_w == r.size.width && ov.annotate.image_h == r.size.height
                });

            if use_annotate {
                ensure_annotate_cache(ov, rect);
                if let Some(cache) = ov.annotate_cache.as_ref() {
                    blit_xrgb_block(
                        &mut ov.frame,
                        img_w,
                        img_h,
                        rect,
                        cache,
                        rect.size.width as usize,
                        rect.size.height as usize,
                    );
                } else {
                    blit_rect(&mut ov.frame, &ov.base, img_w, img_h, rect);
                }
            } else {
                blit_rect(&mut ov.frame, &ov.base, img_w, img_h, rect);
            }
            draw_rect_border(&mut ov.frame, img_w, img_h, rect, 0x00_FF_CC_33);
            damage.push(expand_rect(rect, 3, ov.buf_w, ov.buf_h));
        }

        if ov.phase == OverlayPhase::Ready && !ov.toolbar.is_empty() {
            paint_toolbar(
                &mut ov.frame,
                img_w,
                img_h,
                &ov.toolbar,
                ov.annotate.tool,
                ov.annotate.color,
                ov.annotate.shape_fill_enabled(),
            );
            if let Some(tb) = new_tb {
                damage.push(tb);
            }
        }

        ov.last_drawn_rect = new_rect;
        ov.last_toolbar_bounds = new_tb;
        ov.annotate_dirty = false;
        ov.toolbar_chrome_dirty = false;
    } else {
        return Ok(());
    }

    // 确保 surface 仍是 img 尺寸（防止别处误 resize）
    if let (Some(w), Some(h)) = (NonZeroU32::new(ov.buf_w), NonZeroU32::new(ov.buf_h)) {
        let _ = ov.surface.resize(w, h);
    }

    let mut buffer = ov
        .surface
        .buffer_mut()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay buffer: {e}")))?;
    let needed = img_w * img_h;
    if buffer.len() < needed {
        return Err(PinoraError::new(
            ErrorCode::Internal,
            format!(
                "overlay buffer size mismatch have {} need {} (img {}x{}, win {}x{})",
                buffer.len(),
                needed,
                img_w,
                img_h,
                ov.win_w,
                ov.win_h
            ),
        ));
    }

    // 首帧全量；之后只上传脏区（与 frame 同分辨率，无 scale）
    if !ov.buffer_synced {
        buffer[..needed].copy_from_slice(&ov.frame[..needed]);
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay present: {e}")))?;
        ov.buffer_synced = true;
    } else {
        let mut sb_damage = Vec::with_capacity(damage.len());
        for r in &damage {
            let x0 = r.origin.x.max(0) as usize;
            let y0 = r.origin.y.max(0) as usize;
            let x1 = (r.right() as usize).min(img_w);
            let y1 = (r.bottom() as usize).min(img_h);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let row_w = x1 - x0;
            for y in y0..y1 {
                let off = y * img_w + x0;
                buffer[off..off + row_w].copy_from_slice(&ov.frame[off..off + row_w]);
            }
            if let (Some(nw), Some(nh)) = (
                NonZeroU32::new(row_w as u32),
                NonZeroU32::new((y1 - y0) as u32),
            ) {
                sb_damage.push(DamageRect {
                    x: x0 as u32,
                    y: y0 as u32,
                    width: nw,
                    height: nh,
                });
            }
        }
        if sb_damage.is_empty() {
            buffer[..needed].copy_from_slice(&ov.frame[..needed]);
            buffer.present().map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("overlay present: {e}"))
            })?;
        } else {
            buffer.present_with_damage(&sb_damage).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("overlay damage: {e}"))
            })?;
        }
    }
    ov.last_present = Instant::now();
    Ok(())
}

fn expand_rect(r: PixelRect, pad: i32, img_w: u32, img_h: u32) -> PixelRect {
    let x0 = (r.origin.x - pad).max(0);
    let y0 = (r.origin.y - pad).max(0);
    let x1 = (r.right() + pad).min(img_w as i32);
    let y1 = (r.bottom() + pad).min(img_h as i32);
    PixelRect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

/// 在选区变化/标注变化时合成局部预览（已提交层缓存 → 草稿叠加 → 显示缩放）。
fn ensure_annotate_cache(ov: &mut OverlayState, disp_rect: PixelRect) {
    let wh = (disp_rect.size.width, disp_rect.size.height);
    if !ov.annotate_dirty && ov.annotate_cache.is_some() && ov.annotate_cache_wh == wh {
        return;
    }
    ov.annotate_cache = None;
    let Some(src_rect) = ov.active_src_rect else {
        return;
    };
    let Some(rgba) = ov
        .annotate_preview_cache
        .compose(&ov.full_image, src_rect, &ov.annotate)
    else {
        return;
    };
    let xrgb = rgba_to_xrgb(&rgba);
    let sw = src_rect.size.width as usize;
    let sh = src_rect.size.height as usize;
    let dw = disp_rect.size.width as usize;
    let dh = disp_rect.size.height as usize;
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return;
    }
    if sw == dw && sh == dh {
        ov.annotate_cache = Some(xrgb);
    } else {
        let mut scaled = vec![0u32; dw * dh];
        scale_nearest(&xrgb, sw, sh, &mut scaled, dw, dh);
        ov.annotate_cache = Some(scaled);
    }
    ov.annotate_cache_wh = wh;
}

fn buf_rect_to_src(disp: PixelRect, buf_w: u32, buf_h: u32, src_w: u32, src_h: u32) -> PixelRect {
    let buf_w = buf_w.max(1) as i64;
    let buf_h = buf_h.max(1) as i64;
    let src_w = src_w.max(1) as i64;
    let src_h = src_h.max(1) as i64;
    let x0 = (i64::from(disp.origin.x) * src_w / buf_w).clamp(0, src_w - 1);
    let y0 = (i64::from(disp.origin.y) * src_h / buf_h).clamp(0, src_h - 1);
    let x1 = ((i64::from(disp.right()) * src_w + buf_w - 1) / buf_w).clamp(x0 + 1, src_w);
    let y1 = ((i64::from(disp.bottom()) * src_h + buf_h - 1) / buf_h).clamp(y0 + 1, src_h);
    PixelRect::new(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// 缓冲坐标光标 → 原图选区内标注坐标。
fn overlay_annotate_local(ov: &OverlayState, buf_cursor: PixelPoint) -> Option<PixelPoint> {
    let disp_sel = ov.session.try_confirm().ok()?;
    let src_sel = ov.active_src_rect?;
    if !disp_sel.contains_point(buf_cursor) {
        return None;
    }
    let lx = buf_cursor.x - disp_sel.origin.x;
    let ly = buf_cursor.y - disp_sel.origin.y;
    let dw = disp_sel.size.width.max(1) as i64;
    let dh = disp_sel.size.height.max(1) as i64;
    let sw = src_sel.size.width.max(1) as i64;
    let sh = src_sel.size.height.max(1) as i64;
    Some(PixelPoint::new(
        (i64::from(lx) * sw / dw) as i32,
        (i64::from(ly) * sh / dh) as i32,
    ))
}

/// 从选区局部坐标映射到不可变原始截图后采样。取色不读取已烘焙的标注预览，
/// 因而不会在鼠标点击路径同步分配或重绘整张图。
fn sample_overlay_source_color(
    image: &CaptureImage,
    source_rect: PixelRect,
    local: PixelPoint,
) -> Option<[u8; 4]> {
    if local.x < 0
        || local.y < 0
        || local.x >= source_rect.size.width as i32
        || local.y >= source_rect.size.height as i32
    {
        return None;
    }
    let source = PixelPoint::new(
        source_rect.origin.x.checked_add(local.x)?,
        source_rect.origin.y.checked_add(local.y)?,
    );
    sample_rgba_at(image, source)
}

fn blit_xrgb_block(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    src: &[u32],
    sw: usize,
    sh: usize,
) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let rw = rect.size.width as usize;
    let rh = rect.size.height as usize;
    if sw == 0 || sh == 0 || src.len() < sw * sh {
        return;
    }
    for row in 0..rh.min(sh) {
        let dy = y0 + row;
        if dy >= height {
            break;
        }
        let copy_w = rw.min(sw).min(stride.saturating_sub(x0));
        if copy_w == 0 {
            continue;
        }
        let dst = dy * stride + x0;
        let src_i = row * sw;
        frame[dst..dst + copy_w].copy_from_slice(&src[src_i..src_i + copy_w]);
    }
}

fn window_to_image(x: f64, y: f64, win_w: u32, win_h: u32, img_w: u32, img_h: u32) -> PixelPoint {
    let ix = if win_w == 0 {
        0
    } else {
        ((x * f64::from(img_w)) / f64::from(win_w)).round() as i32
    };
    let iy = if win_h == 0 {
        0
    } else {
        ((y * f64::from(img_h)) / f64::from(win_h)).round() as i32
    };
    PixelPoint::new(
        ix.clamp(0, img_w.saturating_sub(1) as i32),
        iy.clamp(0, img_h.saturating_sub(1) as i32),
    )
}

fn selection_rect_from_window_points(
    start: (f64, f64),
    end: (f64, f64),
    window_w: u32,
    window_h: u32,
    image_w: u32,
    image_h: u32,
) -> PixelRect {
    let x0 = window_edge_to_image(start.0, window_w, image_w);
    let y0 = window_edge_to_image(start.1, window_h, image_h);
    let x1 = window_edge_to_image(end.0, window_w, image_w);
    let y1 = window_edge_to_image(end.1, window_h, image_h);
    PixelRect::new(x0.min(x1), y0.min(y1), x0.abs_diff(x1), y0.abs_diff(y1))
}

fn window_edge_to_image(value: f64, window_extent: u32, image_extent: u32) -> i32 {
    if window_extent == 0 || image_extent == 0 {
        return 0;
    }
    let clamped = value.clamp(0.0, f64::from(window_extent));
    ((clamped / f64::from(window_extent)) * f64::from(image_extent)).round() as i32
}

fn window_rect_from_points(start: (f64, f64), end: (f64, f64)) -> PixelRect {
    let x0 = start.0.min(end.0).max(0.0).round() as i32;
    let y0 = start.1.min(end.1).max(0.0).round() as i32;
    let x1 = start.0.max(end.0).max(0.0).round() as i32;
    let y1 = start.1.max(end.1).max(0.0).round() as i32;
    PixelRect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

fn ensure_pin_render_cache(pin: &mut PinWin, width: u32, height: u32) -> Result<(), PinoraError> {
    if pin
        .render_cache
        .as_ref()
        .is_some_and(|cache| cache.matches(width, height, pin.opacity))
    {
        return Ok(());
    }

    let source_size = pin.image.size();
    pin.render_cache = Some(build_pin_render_cache(
        &pin.pixels_xrgb,
        source_size.width,
        source_size.height,
        width,
        height,
        pin.opacity,
    )?);
    Ok(())
}

fn build_pin_render_cache(
    source: &[u32],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    opacity: f64,
) -> Result<PinRenderCache, PinoraError> {
    let source_len = pixel_count(source_width, source_height, "pin source")?;
    if source.len() != source_len {
        return Err(PinoraError::new(
            ErrorCode::InvalidState,
            "pin source pixels do not match image dimensions",
        ));
    }
    let target_len = pixel_count(width, height, "pin render target")?;
    let source_width = usize::try_from(source_width)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin source is too large"))?;
    let source_height = usize::try_from(source_height)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin source is too large"))?;
    let width_usize = usize::try_from(width)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin render target is too large"))?;
    let height_usize = usize::try_from(height)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin render target is too large"))?;
    let mut pixels = vec![0; target_len];
    if source_width == width_usize && source_height == height_usize {
        pixels.copy_from_slice(source);
    } else {
        scale_nearest(
            source,
            source_width,
            source_height,
            &mut pixels,
            width_usize,
            height_usize,
        );
    }
    apply_opacity_darken(&mut pixels, opacity);
    Ok(PinRenderCache {
        width,
        height,
        opacity_factor: opacity_factor(opacity),
        pixels,
    })
}

fn pixel_count(width: u32, height: u32, subject: &str) -> Result<usize, PinoraError> {
    let width = usize::try_from(width).map_err(|_| {
        PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large"))
    })?;
    let height = usize::try_from(height).map_err(|_| {
        PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large"))
    })?;
    width
        .checked_mul(height)
        .ok_or_else(|| PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large")))
}

/// 无窗口透明时，用压暗模拟 opacity（1.0 = 原色，0.15 = 很暗）。
fn apply_opacity_darken(buf: &mut [u32], opacity: f64) {
    let factor = opacity_factor(opacity);
    if factor == 256 {
        return;
    }
    for px in buf.iter_mut() {
        let r = ((*px >> 16) & 0xff) * factor / 256;
        let g = ((*px >> 8) & 0xff) * factor / 256;
        let b = (*px & 0xff) * factor / 256;
        *px = (r << 16) | (g << 8) | b;
    }
}

fn opacity_factor(opacity: f64) -> u32 {
    if opacity >= 0.999 {
        256
    } else {
        (opacity.clamp(0.05, 1.0) * 256.0) as u32
    }
}

fn opacity_from_settings_percent(percent: u8) -> f64 {
    f64::from(percent.clamp(15, 100)) / 100.0
}

fn blit_rect(dst: &mut [u32], src: &[u32], stride: usize, height: usize, rect: PixelRect) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let row_w = x1 - x0;
    for y in y0..y1 {
        let start = y * stride + x0;
        dst[start..start + row_w].copy_from_slice(&src[start..start + row_w]);
    }
}

fn scale_nearest(src: &[u32], sw: usize, sh: usize, dst: &mut [u32], dw: usize, dh: usize) {
    for y in 0..dh {
        let sy = y * sh / dh;
        let src_row = sy * sw;
        let dst_row = y * dw;
        for x in 0..dw {
            dst[dst_row + x] = src[src_row + x * sw / dw];
        }
    }
}

fn draw_rect_border(buf: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for t in 0..2usize {
        let yt = y0 + t;
        let yb = y1.saturating_sub(1 + t);
        if yt < height {
            for x in x0..x1 {
                buf[yt * stride + x] = color;
            }
        }
        if yb < height && yb >= y0 {
            for x in x0..x1 {
                buf[yb * stride + x] = color;
            }
        }
        let xl = x0 + t;
        let xr = x1.saturating_sub(1 + t);
        for y in y0..y1 {
            if xl < stride {
                buf[y * stride + xl] = color;
            }
            if xr < stride {
                buf[y * stride + xr] = color;
            }
        }
    }
}

/// XRGB 缓冲区上画轴对齐矩形轮廓（OCR 词框）。
fn draw_rect_outline_xrgb(
    buf: &mut [u32],
    stride: usize,
    height: usize,
    from: PixelPoint,
    to: PixelPoint,
    color: u32,
) {
    if stride == 0 || height == 0 {
        return;
    }
    let x0 = from.x.clamp(0, stride as i32 - 1) as usize;
    let x1 = to.x.clamp(0, stride as i32 - 1) as usize;
    let y0 = from.y.clamp(0, height as i32 - 1) as usize;
    let y1 = to.y.clamp(0, height as i32 - 1) as usize;
    if x1 < x0 || y1 < y0 {
        return;
    }
    for x in x0..=x1 {
        buf[y0 * stride + x] = color;
        buf[y1 * stride + x] = color;
    }
    for y in y0..=y1 {
        buf[y * stride + x0] = color;
        buf[y * stride + x1] = color;
    }
}

fn draw_border(buf: &mut [u32], w: usize, h: usize, color: u32) {
    if w == 0 || h == 0 {
        return;
    }
    for x in 0..w {
        buf[x] = color;
        buf[(h - 1) * w + x] = color;
    }
    for y in 0..h {
        buf[y * w] = color;
        buf[y * w + w - 1] = color;
    }
}

#[cfg(test)]
mod overlay_scale_tests {
    use super::*;
    use crate::job_supervisor::{JobResultDisposition, JobSupervisor};
    use pinora_core::{
        Annotation, AnnotationDoc, AssetGeneration, CaptureImage, CaptureMetadata, ContentDigest,
        DEFAULT_STROKE, DEFAULT_WIDTH, DisplayId, HistoryEntry, HistoryEntrySpec, HistoryOcrState,
        ImageId, JobResultRef, PixelSize, RgbaBuffer,
    };

    fn history_entry(id: u64) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: id,
            display: DisplayId::new("test-history"),
            source_rect: PixelRect::new(0, 0, 2, 2),
            file_name: format!("{id}.png"),
            byte_len: 1,
            digest: ContentDigest::of(b"history"),
            ocr: HistoryOcrState::Unknown,
        })
        .expect("history entry")
    }

    #[test]
    fn buf_to_src_identity_when_1to1() {
        let r = buf_rect_to_src(PixelRect::new(10, 20, 100, 50), 3840, 2160, 3840, 2160);
        assert_eq!(r.origin.x, 10);
        assert_eq!(r.origin.y, 20);
        assert_eq!(r.size.width, 100);
        assert_eq!(r.size.height, 50);
    }

    #[test]
    fn buf_to_src_maps_half_buffer() {
        let r = buf_rect_to_src(PixelRect::new(0, 0, 1920, 1080), 1920, 1080, 3840, 2160);
        assert_eq!(r.size.width, 3840);
        assert_eq!(r.size.height, 2160);
    }

    #[test]
    fn color_picker_samples_the_original_pixel_from_selection_local_coordinates() {
        let mut image = CaptureImage::new(
            ImageId::from_raw(29),
            RgbaBuffer::solid(PixelSize::new(5, 4), [1, 2, 3, 255]),
            PixelRect::new(0, 0, 5, 4),
            CaptureMetadata::new(DisplayId::new("picker"), 1.0, 0),
        )
        .expect("image");
        let index = (2 * 5 + 3) * 4;
        image.pixels.bytes[index..index + 4].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0x80]);

        assert_eq!(
            sample_overlay_source_color(&image, PixelRect::new(2, 1, 2, 2), PixelPoint::new(1, 1),),
            Some([0xAA, 0xBB, 0xCC, 0x80])
        );
        assert_eq!(
            sample_overlay_source_color(&image, PixelRect::new(2, 1, 2, 2), PixelPoint::new(2, 1),),
            None
        );
    }

    #[test]
    fn capture_modes_map_to_their_overlay_initial_selection() {
        let bounds = PixelRect::new(-20, 15, 1920, 1080);
        let mut region = SelectionSession::new().with_bounds(bounds).with_min_edge(2);
        let mut full = SelectionSession::new().with_bounds(bounds).with_min_edge(2);

        assert_eq!(
            initial_selection_for_capture(CaptureMode::Region),
            OverlayInitialSelection::Manual
        );
        assert_eq!(
            apply_initial_selection(
                &mut region,
                initial_selection_for_capture(CaptureMode::Region)
            )
            .unwrap(),
            None
        );
        assert_eq!(region.preview_rect(), None);
        assert_eq!(
            initial_selection_for_capture(CaptureMode::FullDisplay),
            OverlayInitialSelection::FullImage
        );
        assert_eq!(
            apply_initial_selection(
                &mut full,
                initial_selection_for_capture(CaptureMode::FullDisplay)
            )
            .unwrap(),
            Some(bounds)
        );
        assert_eq!(
            initial_selection_for_capture(CaptureMode::AllDisplays),
            OverlayInitialSelection::FullImage
        );
        assert_eq!(
            initial_selection_for_capture(CaptureMode::Window),
            OverlayInitialSelection::FullImage
        );
    }

    #[test]
    fn delayed_failure_recovery_precedes_window_and_standard_capture_scopes() {
        let window = CaptureTarget::Window(CaptureWindowInfo {
            id: pinora_core::CaptureWindowId::from_raw(4),
            app_name: "Example".into(),
            title: "Private window".into(),
            bounds: PixelRect::new(1, 2, 3, 4),
            display: DisplayId::new("display"),
            scale: 1.0,
            is_minimized: false,
        });

        assert_eq!(
            capture_failure_scope(&CaptureTarget::DefaultLargest, false),
            CaptureFailureScope::Standard
        );
        assert_eq!(
            capture_failure_scope(&window, false),
            CaptureFailureScope::Window
        );
        assert_eq!(
            capture_failure_scope(&window, true),
            CaptureFailureScope::Delayed
        );
    }

    #[test]
    fn window_capture_opens_a_full_image_editor_without_a_display_capture_target() {
        let window = CaptureWindowInfo {
            id: pinora_core::CaptureWindowId::from_raw(3),
            app_name: "Example".into(),
            title: "Private window".into(),
            bounds: PixelRect::new(40, 50, 800, 600),
            display: DisplayId::new("window-display"),
            scale: 1.25,
            is_minimized: false,
        };

        let target = window_capture_overlay_target(&window);

        assert_eq!(target.display_id, window.display);
        assert_eq!(target.display_origin, window.bounds.origin);
        assert_eq!(target.image_width, 800);
        assert_eq!(target.image_height, 600);
        assert_eq!(target.initial_selection, OverlayInitialSelection::FullImage);
        assert_eq!(target.presentation, OverlayPresentation::WindowCapture);
        assert_eq!(target.min_selection_edge, 1);
        assert_eq!(target.edit_pin_id, None);
    }

    #[test]
    fn pin_edit_opens_a_full_image_editor_for_the_existing_pin() {
        let pin_id = PinId::from_raw(37);
        let image = CaptureImage::new(
            ImageId::from_raw(38),
            RgbaBuffer::solid(PixelSize::new(800, 600), [1, 2, 3, 255]),
            PixelRect::new(240, -30, 800, 600),
            CaptureMetadata::new(DisplayId::new("pin-display"), 1.25, 77),
        )
        .unwrap();

        let target = pin_edit_target(&image, pin_id);

        assert_eq!(target.display_id, image.metadata.display);
        assert_eq!(target.display_origin, image.source_rect.origin);
        assert_eq!(target.image_width, 800);
        assert_eq!(target.image_height, 600);
        assert_eq!(target.initial_selection, OverlayInitialSelection::FullImage);
        assert_eq!(target.presentation, OverlayPresentation::PinEditor);
        assert_eq!(target.min_selection_edge, 1);
        assert_eq!(target.edit_pin_id, Some(pin_id));
    }

    #[test]
    fn preview_buffer_validation_requires_both_render_buffers_to_match_the_image() {
        let image = CaptureImage::new(
            ImageId::from_raw(71),
            RgbaBuffer::solid(PixelSize::new(2, 2), [1, 2, 3, 255]),
            PixelRect::new(0, 0, 2, 2),
            CaptureMetadata::new(DisplayId::new("preview"), 1.0, 1),
        )
        .unwrap();
        let mut preview = prepare_preview(image);

        assert!(preview_buffers_match_image(&preview));
        preview.dimmed.pop();
        assert!(!preview_buffers_match_image(&preview));
    }

    #[test]
    fn delayed_capture_snapshot_keeps_only_previously_visible_ids() {
        let snapshot = snapshot_visible_ids([(11_u8, true), (12_u8, false), (13_u8, true)]);

        assert_eq!(snapshot, vec![11, 13]);
    }

    #[test]
    fn delayed_capture_is_not_due_before_its_deadline() {
        let delayed = DelayedCapture::new(Duration::from_secs(60), Vec::new());

        assert!(!delayed.is_due(Instant::now()));
    }

    #[test]
    fn explicit_capture_target_never_falls_back_to_another_display() {
        let displays = vec![
            DisplayInfo {
                id: DisplayId::new("left"),
                name: "Left".into(),
                bounds: PixelRect::new(-1920, 0, 1920, 1080),
                scale: 1.0,
            },
            DisplayInfo {
                id: DisplayId::new("right"),
                name: "Right".into(),
                bounds: PixelRect::new(0, 0, 2560, 1440),
                scale: 1.25,
            },
        ];

        let selected =
            resolve_capture_target(&displays, &CaptureTarget::Display(DisplayId::new("left")))
                .expect("selected display");
        assert_eq!(selected.id, DisplayId::new("left"));

        let error = resolve_capture_target(
            &displays,
            &CaptureTarget::Display(DisplayId::new("unplugged")),
        )
        .expect_err("missing display must not fall back");
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    #[test]
    fn default_capture_target_keeps_largest_display_behavior() {
        let displays = vec![
            DisplayInfo {
                id: DisplayId::new("small"),
                name: "Small".into(),
                bounds: PixelRect::new(0, 0, 1920, 1080),
                scale: 1.0,
            },
            DisplayInfo {
                id: DisplayId::new("large"),
                name: "Large".into(),
                bounds: PixelRect::new(1920, 0, 2560, 1440),
                scale: 1.0,
            },
        ];

        let selected = resolve_capture_target(&displays, &CaptureTarget::DefaultLargest)
            .expect("largest display");
        assert_eq!(selected.id, DisplayId::new("large"));
    }

    #[test]
    fn history_image_opens_an_ordinary_full_image_editor() {
        let display = DisplayId::new("historic-display");
        let image = CaptureImage::new(
            ImageId::from_raw(33),
            RgbaBuffer::solid(PixelSize::new(1, 1), [1, 2, 3, 255]),
            PixelRect::new(240, -30, 1, 1),
            CaptureMetadata::new(display.clone(), 1.5, 77),
        )
        .unwrap();

        let target = history_edit_target(&image);

        assert_eq!(target.display_id, display);
        assert_eq!(target.display_origin, PixelPoint::new(240, -30));
        assert_eq!(target.initial_selection, OverlayInitialSelection::FullImage);
        assert_eq!(target.presentation, OverlayPresentation::HistoryEditor);
        assert_eq!(target.min_selection_edge, 1);
        assert_eq!(target.edit_pin_id, None);
    }

    #[test]
    fn tray_initialization_failure_does_not_leave_an_unreachable_process() {
        let error = match require_tray(Err("status notifier unavailable".to_owned())) {
            Ok(_) => panic!("tray failure must prevent tray-only startup"),
            Err(error) => error,
        };

        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
        assert!(error.message.contains("tray-only mode"));
    }

    #[test]
    fn pending_export_asset_requires_matching_owner() {
        let job_id = JobId::from_raw(7);
        let owner = JobOwner::Session(SessionId::from_raw(8));
        let asset = AssetRef::initial(pinora_core::ImageId::from_raw(9));
        let mut pending = HashMap::new();
        pending.insert(job_id, (owner, asset));

        assert_eq!(
            pending_asset_for_owner(&pending, job_id, owner),
            Some(asset)
        );
        assert_eq!(
            pending_asset_for_owner(&pending, job_id, JobOwner::Session(SessionId::from_raw(10))),
            None
        );
    }

    #[test]
    fn history_load_result_requires_current_selected_entry() {
        let entry = history_entry(31);
        let active = ActiveHistoryLoad {
            job_id: JobId::from_raw(32),
            request: HistoryLoadRequest {
                entry: entry.clone(),
                intent: HistoryLoadIntent::Preview,
            },
        };
        let asset = AssetRef::new(entry.image_id, entry.generation);

        assert_eq!(
            current_history_load_asset(
                Some(&active),
                Some(&entry),
                JobId::from_raw(32),
                JobOwner::History(entry.image_id),
            ),
            Some(asset)
        );
        assert_eq!(
            current_history_load_asset(
                Some(&active),
                Some(&entry),
                JobId::from_raw(33),
                JobOwner::History(entry.image_id),
            ),
            None
        );
        let changed = HistoryEntry {
            generation: entry.generation.advance().expect("advance generation"),
            ..entry
        };
        assert_eq!(
            current_history_load_asset(
                Some(&active),
                Some(&changed),
                JobId::from_raw(32),
                JobOwner::History(changed.image_id),
            ),
            None
        );
    }

    #[test]
    fn annotation_history_shortcuts_distinguish_undo_and_redo() {
        assert_eq!(
            annotation_history_action(true, false, "z"),
            Some(AnnotationHistoryAction::Undo)
        );
        assert_eq!(
            annotation_history_action(true, true, "Z"),
            Some(AnnotationHistoryAction::Redo)
        );
        assert_eq!(
            annotation_history_action(true, false, "y"),
            Some(AnnotationHistoryAction::Redo)
        );
        assert_eq!(annotation_history_action(false, false, "z"), None);
    }

    #[test]
    fn sequence_tool_consumes_a_double_click_instead_of_copying_the_overlay() {
        assert!(overlay_click_finishes_copy(AnnotateTool::Rect, true));
        assert!(!overlay_click_finishes_copy(AnnotateTool::Rect, false));
        assert!(!overlay_click_finishes_copy(AnnotateTool::Number, true));
    }

    #[test]
    fn settings_opacity_is_converted_to_bounded_runtime_value() {
        assert!((opacity_from_settings_percent(72) - 0.72).abs() < f64::EPSILON);
        assert!((opacity_from_settings_percent(0) - 0.15).abs() < f64::EPSILON);
        assert!((opacity_from_settings_percent(255) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pin_render_cache_scales_and_darkens_for_its_exact_key() {
        let cache =
            build_pin_render_cache(&[0x00ff_0000, 0x0000_00ff], 2, 1, 4, 1, 0.5).expect("cache");

        assert_eq!(
            cache.pixels,
            vec![0x007f_0000, 0x007f_0000, 0x0000_007f, 0x0000_007f]
        );
        assert!(cache.matches(4, 1, 0.5));
        assert!(!cache.matches(2, 1, 0.5));
        assert!(!cache.matches(4, 1, 0.75));
    }

    #[test]
    fn near_opaque_pin_render_cache_keeps_existing_pixel_semantics() {
        let cache = build_pin_render_cache(&[0x0011_2233], 1, 1, 1, 1, 0.999).expect("cache");

        assert_eq!(cache.pixels, vec![0x0011_2233]);
        assert!(cache.matches(1, 1, 1.0));
    }

    #[test]
    fn ocr_selection_rect_maps_scaled_window_to_image_pixels() {
        assert_eq!(
            selection_rect_from_window_points((20.0, 10.0), (120.0, 60.0), 200, 100, 100, 50),
            PixelRect::new(10, 5, 50, 25)
        );
        assert_eq!(
            selection_rect_from_window_points((120.0, 60.0), (20.0, 10.0), 200, 100, 100, 50),
            PixelRect::new(10, 5, 50, 25)
        );
    }

    #[test]
    fn annotation_revision_changes_overlay_asset_and_rejects_late_result() {
        let identity = OverlayAssetIdentity::new();
        let mut doc = AnnotationDoc::new();
        let submitted = identity.current(doc.revision());

        doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        let current = identity.current(doc.revision());
        assert_eq!(submitted.image_id, current.image_id);
        assert_ne!(submitted.generation, current.generation);

        let spec = JobSpec::new(
            JobId::from_raw(11),
            CorrelationId::from_raw(12),
            submitted,
            JobOwner::Session(SessionId::from_raw(13)),
            JobKind::Ocr,
            100,
        );
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(spec).expect("submit overlay OCR");
        assert_eq!(
            supervisor
                .accept_result(JobResultRef::new(ticket.id, submitted), current, 1)
                .expect("known job"),
            JobResultDisposition::Rejected(pinora_core::JobTerminalState::StaleAsset)
        );

        let before_empty_undo = identity.current(doc.revision());
        assert!(doc.undo().is_some());
        assert_ne!(identity.current(doc.revision()), before_empty_undo);
        let after_undo = identity.current(doc.revision());
        assert_eq!(doc.undo(), None);
        assert_eq!(identity.current(doc.revision()), after_undo);
    }

    #[test]
    fn redo_produces_a_fresh_overlay_asset_generation() {
        let identity = OverlayAssetIdentity::new();
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        let committed = identity.current(doc.revision());
        assert!(doc.undo().is_some());
        let undone = identity.current(doc.revision());
        assert!(doc.redo().is_some());
        let redone = identity.current(doc.revision());

        assert_eq!(committed.image_id, undone.image_id);
        assert_eq!(undone.image_id, redone.image_id);
        assert_ne!(committed.generation, undone.generation);
        assert_ne!(undone.generation, redone.generation);
        assert_ne!(committed.generation, redone.generation);
    }

    #[test]
    fn reselection_uses_a_new_image_identity_and_stamps_derived_image() {
        let first = OverlayAssetIdentity::new();
        let second = OverlayAssetIdentity::new();
        let revision = AnnotationRevision::INITIAL;
        assert_ne!(
            first.current(revision).image_id,
            second.current(revision).image_id
        );
        assert_eq!(
            first.current(revision).generation,
            second.current(revision).generation
        );

        let mut image = CaptureImage::new(
            ImageId::from_raw(99),
            RgbaBuffer::solid(pinora_core::PixelSize::new(2, 2), [1, 2, 3, 255]),
            PixelRect::new(0, 0, 2, 2),
            CaptureMetadata::new(DisplayId::new("test"), 1.0, 0),
        )
        .expect("derived test image");
        first.stamp(&mut image);
        assert_eq!(image.id, first.current(revision).image_id);
    }
}
