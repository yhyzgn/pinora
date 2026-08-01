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
use crate::frame_cache::{FrameCache, rgba_to_xrgb_and_dim};
use crate::history_browser::{
    self, HistoryPanel, HistoryPanelAction, HistoryPanelKey, HistoryPreview,
};
use crate::history_export::{
    HistoryExportCandidate, cleanup_history_tombstones, clear_history_entries,
    delete_history_entry, history_candidate_for_export, load_history_image, load_history_index,
    record_history_candidate,
};
use crate::history_store::{HistoryStore, default_history_path};
use crate::hotkey::GlobalHotkeyHub;
use crate::ocr::tesseract_available;
use crate::ocr_job::{OcrJobCompletion, OcrJobService};
use crate::overlay_toolbar::{
    ToolbarAction, ToolbarButton, hit_test as toolbar_hit, layout_toolbar, paint_toolbar,
    toolbar_bounds,
};
use crate::settings_panel::{self, SettingsPanel, SettingsPanelAction, SettingsPanelKey};
use crate::settings_store::{SettingsStore, default_settings_path};
use crate::tray::{AppTray, TrayAction};
use pinora_core::{
    ActionId, AnnotateSession, AnnotateTool, AnnotationRevision, AssetGeneration, AssetRef,
    CaptureImage, CaptureProvider, CaptureRequest, Command, CorrelationId, DisplayId,
    DomainEventKind, ErrorCode, HistoryIndex, ImageId, ImageSink, JobId, JobKind, JobOwner,
    JobSpec, OcrResult, OcrTextSelection, OcrWordRef, PinId, PinTransform, PinoraError, PixelPoint,
    PixelRect, SelectionSession, SessionId, bake_annotations, render_preview_rgba,
};
use softbuffer::{Context, Rect as DamageRect, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId, WindowLevel};

use crate::pin_window::scaled_window_size;
use crate::platform::CapabilityProbe;
use crate::runtime::AppRuntime;
use crate::single_instance::SingleInstance;

const MIN_FRAME_INTERVAL: Duration = Duration::from_micros(16_666);
const OCR_JOB_TIMEOUT_MS: u64 = 30_000;
const EXPORT_JOB_TIMEOUT_MS: u64 = 30_000;
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

/// 运行统一桌面 shell（阻塞直到退出）。
pub fn run_desktop_shell<L, P, C, S>(runtime: AppRuntime<L, P, C, S>) -> Result<(), PinoraError>
where
    L: SingleInstance + 'static,
    P: CapabilityProbe + 'static,
    C: CaptureProvider + Clone + Send + 'static,
    S: ImageSink + 'static,
{
    let event_loop = EventLoop::new()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop event loop: {e}")))?;

    let hotkeys = GlobalHotkeyHub::start();
    for note in &hotkeys.status().notes {
        println!("pinora: hotkey: {note}");
    }
    if hotkeys.status().available {
        println!("pinora: global hotkeys: F2 / Ctrl+N / Ctrl+Shift+S → capture");
    } else {
        println!("pinora: global hotkeys unavailable — use window focus keys or `pinora capture`");
    }

    // 后台预截屏：空闲时持续备帧，F2 时 overlay 瞬时弹出
    let provider = runtime.capture_provider().clone();
    let frame_cache = FrameCache::start(provider);
    println!("pinora: frame-cache started (pre-capture for instant overlay)");

    let tray = match AppTray::try_new() {
        Ok(t) => {
            println!("pinora: system tray ready (click / menu → capture)");
            Some(t)
        }
        Err(e) => {
            eprintln!("pinora: system tray unavailable: {e}");
            None
        }
    };
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
        // 先 Idle，等缓存出第一帧再自动截；若用户立刻 F2 也会走缓存/等待
        mode: Mode::StartCapture,
        loading: None,
        overlay: None,
        control: None,
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
        pending_exports: HashMap::new(),
        history_store,
        history_index,
        start_capture_wait: None,
        tray,
        default_pin_opacity,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop loop: {e}")))?;

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
    /// 空闲：仅贴图窗口。
    Idle,
}

/// 后台线程准备好的全屏预览（原图像素 + 暗化底图）。
struct PreparedPreview {
    image: CaptureImage,
    base: Vec<u32>,
    dimmed: Vec<u32>,
}

/// 截屏中：后台抓当前屏（无全屏遮罩，避免截到自己）；完成后立刻开真实 overlay。
struct LoadingState {
    preview_rx: Receiver<Result<PreparedPreview, String>>,
    display_id: DisplayId,
    display_origin: PixelPoint,
}

/// Idle 时保持一个小控制窗，否则 Wayland 下无焦点窗口时 F2 永远收不到。
struct ControlState {
    window: Rc<Window>,
}

struct SettingsState {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: SettingsPanel,
    store: SettingsStore,
    cursor: PixelPoint,
    width: u32,
    height: u32,
}

struct HistoryPreviewCache {
    entry_image_id: ImageId,
    pixels_xrgb: Vec<u32>,
    size: pinora_core::PixelSize,
}

struct HistoryState {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: HistoryPanel,
    cursor: PixelPoint,
    width: u32,
    height: u32,
    preview: Option<HistoryPreviewCache>,
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
    /// 标注预览缓存（选区在缓冲分辨率下的 XRGB）。
    annotate_cache: Option<Vec<u32>>,
    annotate_cache_wh: (u32, u32),
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
}

struct PinWin {
    pin_id: PinId,
    image: CaptureImage,
    asset: AssetRef,
    pixels_xrgb: Vec<u32>,
    scale: f64,
    opacity: f64,
    locked: bool,
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

struct DesktopApp<L, P, C, S> {
    runtime: Option<AppRuntime<L, P, C, S>>,
    context: Option<Context<Rc<Window>>>,
    mode: Mode,
    loading: Option<LoadingState>,
    overlay: Option<OverlayState>,
    /// 无选区/加载时的常驻控制窗（收 F2 / Ctrl+N / Ctrl+Q）。
    control: Option<ControlState>,
    /// 显式设置窗口；草稿只在保存成功后应用到 runtime。
    settings: Option<SettingsState>,
    /// 受管历史浏览窗口；文件读取和删除必须经 history_export 安全边界。
    history: Option<HistoryState>,
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
    pending_exports: HashMap<JobId, PendingExport>,
    history_store: HistoryStore,
    history_index: HistoryIndex,
    /// 等待 frame-cache 首帧的起始时间；超时走 cold path。
    start_capture_wait: Option<Instant>,
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
        self.try_start_capture(event_loop);
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
            match action {
                TrayAction::Capture => {
                    println!("pinora: tray → capture");
                    self.request_new_capture(event_loop);
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

        for msg in self.pending_messages.drain(..) {
            println!("{msg}");
        }

        if self.quit {
            event_loop.exit();
            return;
        }

        // 后台截屏完成 → 打开真实桌面遮罩
        if matches!(self.mode, Mode::LoadingCapture) {
            if let Err(e) = self.poll_loading_to_overlay(event_loop) {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
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

        // Idle 且无贴图：必须有控制窗收 F2（有贴图时贴图窗自身收键）
        if self.settings.is_none()
            && self.history.is_none()
            && matches!(self.mode, Mode::Idle)
            && self.overlay.is_none()
            && self.loading.is_none()
            && self.pins.is_empty()
        {
            if let Err(e) = self.ensure_control_window(event_loop) {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        } else if !self.pins.is_empty() {
            // 有贴图时收起控制窗，避免抢焦点
            self.hide_control();
        }

        // 短周期唤醒，以便 poll 全局热键 / 单实例 socket（Wait 不会因 channel 醒来）
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
            && history.window.id() == window_id
        {
            self.handle_history_event(event_loop, event);
            return;
        }
        if let Some(settings) = self.settings.as_ref()
            && settings.window.id() == window_id
        {
            self.handle_settings_event(event_loop, event);
            return;
        }
        if let Some(control) = self.control.as_ref()
            && control.window.id() == window_id
        {
            self.handle_control_event(event_loop, event);
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
    fn ensure_context(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_some() {
            return;
        }
        // 先建一个隐藏占位窗以拿到 display handle（Wayland 需要）
        let attrs = Window::default_attributes()
            .with_visible(false)
            .with_title("pinora-display-handle");
        if let Ok(w) = event_loop.create_window(attrs) {
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
        let cache_ready = self.frame_cache.as_ref().and_then(|c| c.peek()).is_some();
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
        if let Err(e) = self.begin_screen_grab(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    /// 弹出选区 overlay：优先用后台预截帧（瞬时），否则再等一次截屏。
    fn begin_screen_grab(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        self.hide_control();
        // 截屏/选区期间暂停预截，避免截到自己的窗
        if let Some(cache) = &self.frame_cache {
            cache.pause();
        }

        // 1) 缓存命中 → 立刻开 overlay（目标 < 16ms）
        // 允许最多 2s 龄的帧；后台约每 0.5s 刷新一轮
        if let Some(cache) = &self.frame_cache
            && let Some(frame) = cache
                .take_if_fresh(Duration::from_secs(2))
                .or_else(|| cache.take_any())
        {
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
                frame.display_id,
                frame.display_origin,
                img_w,
                img_h,
            );
        }

        // 2) 缓存未就绪（刚启动）：同步等待路径
        let rt = self.runtime.as_ref().unwrap();
        let displays = rt.capture_provider().displays()?;
        let display = displays
            .iter()
            .max_by_key(|d| d.bounds.size.area())
            .cloned()
            .ok_or_else(|| {
                PinoraError::new(ErrorCode::NotFound, "no display for region capture")
            })?;

        let provider = rt.capture_provider().clone();
        let display_id = display.id.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let started = Instant::now();
            let result = provider
                .capture(CaptureRequest::FullDisplay {
                    display: display_id,
                })
                .map(|image| {
                    let (base, dimmed) = rgba_to_xrgb_and_dim(&image.pixels.bytes);
                    PreparedPreview {
                        image,
                        base,
                        dimmed,
                    }
                })
                .map_err(|e| e.to_string());
            println!(
                "pinora: capture done in {:.0}ms (cold path)",
                started.elapsed().as_secs_f64() * 1000.0
            );
            let _ = tx.send(result);
        });

        println!(
            "pinora: cache miss — grabbing {} ({}x{})…",
            display.name, display.bounds.size.width, display.bounds.size.height
        );

        self.loading = Some(LoadingState {
            preview_rx: rx,
            display_id: display.id,
            display_origin: display.bounds.origin,
        });
        self.mode = Mode::LoadingCapture;
        Ok(())
    }

    fn resume_frame_cache(&self) {
        if let Some(cache) = &self.frame_cache {
            cache.resume();
        }
    }

    /// Idle 控制窗：Wayland 无全局热键时，必须有窗口持有键盘焦点才能收到 F2。
    fn ensure_control_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        self.ensure_context(event_loop);
        if let Some(control) = self.control.as_ref() {
            control.window.set_visible(true);
            control.window.focus_window();
            return Ok(());
        }
        let attrs = Window::default_attributes()
            .with_title("Pinora — F2 截图 · H 历史 · S 设置 · Ctrl+Q 退出")
            .with_inner_size(PhysicalSize::new(420, 64))
            .with_decorations(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("control window: {e}")))?;
        let window = Rc::new(window);
        window.focus_window();
        self.control = Some(ControlState { window });
        println!("pinora: idle — focus control window, then F2/Ctrl+N to capture");
        Ok(())
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        if let Some(settings) = self.settings.as_ref() {
            settings.window.focus_window();
            return Ok(());
        }
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;
        let attrs = Window::default_attributes()
            .with_title("Pinora Settings")
            .with_inner_size(PhysicalSize::new(
                settings_panel::PANEL_WIDTH,
                settings_panel::PANEL_HEIGHT,
            ))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings window: {e}")))?;
        let window = Rc::new(window);
        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings surface: {e}")))?;
        if let (Some(w), Some(h)) = (
            NonZeroU32::new(settings_panel::PANEL_WIDTH),
            NonZeroU32::new(settings_panel::PANEL_HEIGHT),
        ) {
            surface.resize(w, h).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("settings resize: {e}"))
            })?;
        }
        let current = self
            .runtime
            .as_ref()
            .map(AppRuntime::settings)
            .unwrap_or_default();
        let panel = SettingsPanel::new(current);
        window.focus_window();
        self.hide_control();
        self.settings = Some(SettingsState {
            window: window.clone(),
            surface,
            panel,
            store: SettingsStore::new(default_settings_path()),
            cursor: PixelPoint::new(0, 0),
            width: settings_panel::PANEL_WIDTH,
            height: settings_panel::PANEL_HEIGHT,
        });
        window.request_redraw();
        println!("pinora: settings opened (arrows edit, Enter save, Esc cancel)");
        Ok(())
    }

    fn close_settings(&mut self) {
        if let Some(settings) = self.settings.take() {
            settings.window.set_visible(false);
        }
    }

    fn handle_settings_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_settings(),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.cursor =
                        PixelPoint::new(position.x.round() as i32, position.y.round() as i32);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let action = self
                    .settings
                    .as_ref()
                    .and_then(|settings| SettingsPanel::hit_test(settings.cursor));
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
                if self.is_new_capture_key(&event) {
                    self.close_settings();
                    self.request_new_capture(event_loop);
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
                    .and_then(|settings| settings.panel.handle_key(key));
                if let Some(action) = action {
                    self.apply_settings_action(event_loop, action);
                } else if let Some(settings) = self.settings.as_ref() {
                    settings.window.request_redraw();
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
                    settings.width = size.width.max(1);
                    settings.height = size.height.max(1);
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(settings.width),
                        NonZeroU32::new(settings.height),
                    ) {
                        let _ = settings.surface.resize(w, h);
                    }
                    settings.window.request_redraw();
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
                    settings.panel.apply_action(action);
                    settings.window.request_redraw();
                }
            }
            SettingsPanelAction::Cancel => self.close_settings(),
            SettingsPanelAction::Save => self.save_settings(),
        }
    }

    fn save_settings(&mut self) {
        let Some((draft, window)) = self
            .settings
            .as_ref()
            .map(|settings| (settings.panel.draft(), settings.window.clone()))
        else {
            return;
        };
        let save_result = self
            .settings
            .as_ref()
            .map(|settings| settings.store.save(draft));
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
                }
                if let Some(settings) = self.settings.as_mut() {
                    settings.panel.mark_saved();
                }
                println!("pinora: settings saved (theme={:?})", draft.theme);
                window.request_redraw();
            }
            Some(Err(_)) => {
                if let Some(settings) = self.settings.as_mut() {
                    settings.panel.mark_save_failed("settings_save_failed");
                }
                eprintln!("pinora: settings save failed; runtime values unchanged");
                window.request_redraw();
            }
            None => {}
        }
    }

    fn paint_settings(&mut self) -> Result<(), PinoraError> {
        let Some(settings) = self.settings.as_mut() else {
            return Ok(());
        };
        let size = settings.window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        settings.width = width;
        settings.height = height;
        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            settings.surface.resize(w, h).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("settings surface resize: {e}"))
            })?;
        }
        let mut buffer = settings
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings buffer: {e}")))?;
        let width = width as usize;
        let height = height as usize;
        if buffer.len() < width.saturating_mul(height) {
            return Ok(());
        }
        settings_panel::paint(
            &settings.panel,
            &mut buffer[..width * height],
            width,
            height,
        );
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings present: {e}")))?;
        Ok(())
    }

    fn open_history(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        if let Some(history) = self.history.as_ref() {
            history.window.focus_window();
            return Ok(());
        }
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;
        let attrs = Window::default_attributes()
            .with_title("Pinora History")
            .with_inner_size(PhysicalSize::new(
                history_browser::PANEL_WIDTH,
                history_browser::PANEL_HEIGHT,
            ))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history window: {e}")))?;
        let window = Rc::new(window);
        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history surface: {e}")))?;
        if let (Some(w), Some(h)) = (
            NonZeroU32::new(history_browser::PANEL_WIDTH),
            NonZeroU32::new(history_browser::PANEL_HEIGHT),
        ) {
            surface.resize(w, h).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("history resize: {e}"))
            })?;
        }
        let entries = self.history_index.active_entries().cloned().collect();
        self.hide_control();
        self.history = Some(HistoryState {
            window: window.clone(),
            surface,
            panel: HistoryPanel::new(entries),
            cursor: PixelPoint::new(0, 0),
            width: history_browser::PANEL_WIDTH,
            height: history_browser::PANEL_HEIGHT,
            preview: None,
        });
        self.refresh_history_preview();
        window.focus_window();
        window.request_redraw();
        println!("pinora: history opened (Enter pin, Delete remove, Esc close)");
        Ok(())
    }

    fn close_history(&mut self) {
        if let Some(history) = self.history.take() {
            history.window.set_visible(false);
        }
    }

    fn refresh_history_preview(&mut self) {
        let Some(entry) = self
            .history
            .as_ref()
            .and_then(|history| history.panel.selected_entry())
            .cloned()
        else {
            return;
        };
        let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir().clone()) else {
            return;
        };
        match load_history_image(&export_dir, &entry) {
            Ok(image) => {
                if let Some(history) = self.history.as_mut() {
                    history.preview = Some(HistoryPreviewCache {
                        entry_image_id: entry.image_id,
                        pixels_xrgb: rgba_to_xrgb(&image.pixels.bytes),
                        size: image.pixels.size,
                    });
                    history.panel.clear_error();
                    history.window.request_redraw();
                }
            }
            Err(_) => {
                if let Some(history) = self.history.as_mut() {
                    history.preview = None;
                    history.panel.mark_error("history_load_failed");
                    history.window.request_redraw();
                }
            }
        }
    }

    fn handle_history_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_history(),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(history) = self.history.as_mut() {
                    history.cursor =
                        PixelPoint::new(position.x.round() as i32, position.y.round() as i32);
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
                    .and_then(|history| history.panel.hit_test(history.cursor));
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
                if self.is_new_capture_key(&event) {
                    self.close_history();
                    self.request_new_capture(event_loop);
                    return;
                }
                let key = match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => Some(HistoryPanelKey::Up),
                    PhysicalKey::Code(KeyCode::ArrowDown) => Some(HistoryPanelKey::Down),
                    PhysicalKey::Code(KeyCode::Enter) => Some(HistoryPanelKey::Enter),
                    PhysicalKey::Code(KeyCode::Delete) => Some(HistoryPanelKey::Delete),
                    PhysicalKey::Code(KeyCode::Backspace) => Some(HistoryPanelKey::Backspace),
                    PhysicalKey::Code(KeyCode::Escape) => Some(HistoryPanelKey::Escape),
                    _ => None,
                };
                let action = if let Some(key) = key {
                    self.history
                        .as_mut()
                        .and_then(|history| history.panel.handle_key(key))
                } else if !self.modifiers.control_key()
                    && !self.modifiers.alt_key()
                    && !self.modifiers.super_key()
                {
                    let text = match &event.logical_key {
                        Key::Character(text) => Some(text.as_str()),
                        _ => None,
                    };
                    let mut changed = false;
                    if let Some(text) = text
                        && let Some(history) = self.history.as_mut()
                    {
                        for character in text.chars() {
                            changed |= history.panel.input_char(character);
                        }
                    }
                    if changed {
                        self.refresh_history_preview();
                    }
                    None
                } else {
                    None
                };
                if let Some(action) = action {
                    self.apply_history_action(event_loop, action);
                } else {
                    self.refresh_history_preview();
                }
                if let Some(history) = self.history.as_ref() {
                    history.window.request_redraw();
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
                    history.width = size.width.max(1);
                    history.height = size.height.max(1);
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(history.width),
                        NonZeroU32::new(history.height),
                    ) {
                        let _ = history.surface.resize(w, h);
                    }
                    history.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn apply_history_action(&mut self, event_loop: &ActiveEventLoop, action: HistoryPanelAction) {
        match action {
            HistoryPanelAction::Select(index) => {
                if let Some(history) = self.history.as_mut() {
                    history.panel.select(index);
                    history.preview = None;
                }
                self.refresh_history_preview();
            }
            HistoryPanelAction::Close => self.close_history(),
            HistoryPanelAction::Reopen => self.reopen_history_entry(event_loop),
            HistoryPanelAction::Delete => self.delete_selected_history_entry(),
            HistoryPanelAction::RequestClear => {
                if let Some(history) = self.history.as_mut() {
                    let _ = history.panel.request_clear();
                    history.window.request_redraw();
                }
            }
            HistoryPanelAction::CancelClear => {
                if let Some(history) = self.history.as_mut() {
                    history.panel.cancel_clear();
                    history.window.request_redraw();
                }
            }
            HistoryPanelAction::ConfirmClear => self.clear_all_history_entries(),
        }
    }

    fn clear_all_history_entries(&mut self) {
        let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir().clone()) else {
            return;
        };
        let result =
            clear_history_entries(&self.history_store, &export_dir, &mut self.history_index);
        let active = self.history_index.active_entries().cloned().collect();
        if let Some(history) = self.history.as_mut() {
            history.preview = None;
            history.panel.replace_entries(active);
            history.panel.cancel_clear();
            match result {
                Err(error) => {
                    eprintln!("pinora: history clear failed: {error}");
                    history.panel.mark_error("history_clear_failed");
                }
                Ok(cleanup) => {
                    if cleanup.failed_files > 0 || cleanup.protected_files > 0 {
                        history.panel.mark_error("history_clear_partial");
                    } else {
                        history.panel.clear_error();
                    }
                }
            }
            history.window.request_redraw();
        }
    }

    fn reopen_history_entry(&mut self, event_loop: &ActiveEventLoop) {
        let Some(entry) = self
            .history
            .as_ref()
            .and_then(|history| history.panel.selected_entry())
            .cloned()
        else {
            return;
        };
        let Some(export_dir) = self.runtime.as_ref().map(|rt| rt.export_dir().clone()) else {
            return;
        };
        let image = match load_history_image(&export_dir, &entry) {
            Ok(image) => image,
            Err(_) => {
                if let Some(history) = self.history.as_mut() {
                    history.panel.mark_error("history_load_failed");
                    history.preview = None;
                    history.window.request_redraw();
                }
                return;
            }
        };
        match self.open_pin_from_image(event_loop, image, entry.source_rect.origin, false) {
            Ok(()) => self.close_history(),
            Err(error) => {
                eprintln!("pinora: history reopen failed ({})", error.code);
                if let Some(history) = self.history.as_mut() {
                    history.panel.mark_error("history_pin_failed");
                    history.window.request_redraw();
                }
            }
        }
    }

    fn delete_selected_history_entry(&mut self) {
        let Some(image_id) = self
            .history
            .as_ref()
            .and_then(|history| history.panel.selected_entry())
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
                history.preview = None;
                history.panel.replace_entries(remaining);
                history.panel.mark_error("history_delete_failed");
                history.window.request_redraw();
            }
            return;
        }
        if let Some(history) = self.history.as_mut() {
            history.preview = None;
            history
                .panel
                .replace_entries(self.history_index.active_entries().cloned().collect());
            history.window.request_redraw();
        }
        self.refresh_history_preview();
    }

    fn paint_history(&mut self) -> Result<(), PinoraError> {
        let Some(history) = self.history.as_mut() else {
            return Ok(());
        };
        let size = history.window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        history.width = width;
        history.height = height;
        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            history.surface.resize(w, h).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("history surface resize: {e}"))
            })?;
        }
        let mut buffer = history
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history buffer: {e}")))?;
        let width = width as usize;
        let height = height as usize;
        if buffer.len() < width.saturating_mul(height) {
            return Ok(());
        }
        let selected_image_id = history.panel.selected_entry().map(|entry| entry.image_id);
        let preview = history.preview.as_ref().and_then(|preview| {
            (Some(preview.entry_image_id) == selected_image_id).then_some(HistoryPreview {
                pixels_xrgb: &preview.pixels_xrgb,
                size: preview.size,
            })
        });
        history_browser::paint(
            &history.panel,
            preview,
            &mut buffer[..width * height],
            width,
            height,
        );
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history present: {e}")))?;
        Ok(())
    }

    fn hide_control(&mut self) {
        if let Some(control) = self.control.as_ref() {
            control.window.set_visible(false);
        }
    }

    fn handle_control_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.quit = true;
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.is_quit_key(&event) {
                    self.quit = true;
                    event_loop.exit();
                } else if self.is_new_capture_key(&event) {
                    self.request_new_capture(event_loop);
                } else if matches!(event.logical_key, Key::Character(ref c) if c == "s" || c == "S")
                    || (self.modifiers.control_key()
                        && matches!(event.logical_key, Key::Character(ref c) if c == ","))
                {
                    if let Err(error) = self.open_settings(event_loop) {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                } else if matches!(event.logical_key, Key::Character(ref c) if c == "h" || c == "H")
                {
                    if let Err(error) = self.open_history(event_loop) {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                } else if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    // Esc 在控制窗：若有贴图则聚焦贴图，否则退出
                    if let Some(pin) = self.pins.values().next() {
                        pin.window.focus_window();
                    } else {
                        self.quit = true;
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    /// 任意模式触发再截：立刻关 overlay/loading，开新一轮 grab。
    fn request_new_capture(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(ov) = self.overlay.take() {
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            self.export_jobs
                .close_owner(JobOwner::Session(ov.session_id));
            ov.window.set_visible(false);
        }
        self.close_settings();
        self.close_history();
        let _ = self.loading.take();
        self.hide_control();
        self.mode = Mode::StartCapture;
        println!("pinora: new capture requested (F2/Ctrl+N)");
        if let Err(e) = self.begin_screen_grab(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn cancel_loading(&mut self) {
        let _ = self.loading.take();
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        println!("pinora: capture cancelled (F2/Ctrl+N 再截，Ctrl+Q 退出)");
        if let Some(pin) = self.pins.values().next() {
            pin.window.focus_window();
        }
    }

    fn poll_loading_to_overlay(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        let Some(loading) = self.loading.as_ref() else {
            return Ok(());
        };
        let prep = match loading.preview_rx.try_recv() {
            Ok(Ok(p)) => p,
            Ok(Err(err)) => {
                self.cancel_loading();
                return Err(PinoraError::new(
                    ErrorCode::RetryablePlatform,
                    format!("screen capture failed: {err}"),
                ));
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => {
                self.cancel_loading();
                return Err(PinoraError::new(
                    ErrorCode::Internal,
                    "capture thread disconnected",
                ));
            }
        };

        let loading = self.loading.take().unwrap();
        let img_w = prep.image.pixels.size.width.max(1);
        let img_h = prep.image.pixels.size.height.max(1);
        if prep.base.len() != (img_w as usize) * (img_h as usize) {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "capture buffer size mismatch",
            ));
        }

        self.open_overlay_with_preview(
            event_loop,
            prep,
            loading.display_id,
            loading.display_origin,
            img_w,
            img_h,
        )
    }

    fn open_overlay_with_preview(
        &mut self,
        event_loop: &ActiveEventLoop,
        prep: PreparedPreview,
        display_id: DisplayId,
        display_origin: PixelPoint,
        img_w: u32,
        img_h: u32,
    ) -> Result<(), PinoraError> {
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let attrs = Window::default_attributes()
            .with_title("Pinora — 拖选后工具栏 | 双击复制 中键贴图 Enter贴图 Esc取消")
            .with_inner_size(PhysicalSize::new(img_w, img_h))
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_cursor(CursorIcon::Crosshair)
            .with_decorations(false)
            .with_visible(true);

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay window: {e}")))?;
        let window = Rc::new(window);
        window.focus_window();

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
                .with_min_edge(2),
            phase: OverlayPhase::Selecting,
            dragging: false,
            pending_reselect: false,
            drag_anchor: PixelPoint::new(0, 0),
            annotate_dragging: false,
            annotate: AnnotateSession::new(1, 1),
            annotate_cache: None,
            annotate_cache_wh: (0, 0),
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
        });
        self.mode = Mode::Idle;
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
            if self.is_new_capture_key(key) {
                self.request_new_capture(event_loop);
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
                    if is_double {
                        if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Copy)
                        {
                            eprintln!("pinora: double-click copy failed: {e}");
                        }
                        return;
                    }
                    if let Some(local) = overlay_annotate_local(ov, p) {
                        ov.annotate.begin(local);
                        ov.annotate_dragging = ov.annotate.tool != AnnotateTool::Text;
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
                        // 标注坐标系 = 原图选区像素
                        ov.annotate = AnnotateSession::new(src_sel.size.width, src_sel.size.height);
                        ov.annotate.tool = tool;
                        ov.annotate.color = color;
                        ov.annotate.stroke = stroke;
                        ov.annotate_cache = None;
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
            ToolbarAction::Tool(tool) => {
                // 切换工具：零重绘（高亮可延后；点击必须瞬时）
                if let Some(ov) = self.overlay.as_mut() {
                    ov.annotate.tool = tool;
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
                            println!("pinora: copied OCR text for {}", job.asset.image_id);
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
        let display_id = ov.display_id.clone();
        let session_owner = JobOwner::Session(ov.session_id);
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
        // 先裁切（仍持有 overlay），再立刻关窗
        let image = self.crop_overlay_image(true)?;
        let position = PixelPoint::new(global.origin.x, global.origin.y);

        if let Some(ov) = self.overlay.take() {
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            if action == OverlayFinish::Pin {
                self.export_jobs
                    .close_owner(JobOwner::Session(ov.session_id));
            }
            ov.window.set_visible(false);
            drop(ov);
        }
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        println!(
            "pinora: finish {action:?} {}x{} @ ({},{}) display={display_id:?}",
            src_rect.size.width, src_rect.size.height, global.origin.x, global.origin.y
        );

        match action {
            OverlayFinish::Copy => {
                if let Some(rt) = self.runtime.as_mut() {
                    rt.dispatch(Command::create_pin(image.clone(), position))?;
                }
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
                if let Some(rt) = self.runtime.as_mut() {
                    rt.dispatch(Command::create_pin(image.clone(), position))?;
                }
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
        Ok(())
    }

    fn open_pin_from_image(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: CaptureImage,
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
            position,
            1.0,
            self.default_pin_opacity,
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
        if let Some(ov) = self.overlay.take() {
            self.ocr_jobs.close_owner(JobOwner::Session(ov.session_id));
            self.export_jobs
                .close_owner(JobOwner::Session(ov.session_id));
            ov.window.set_visible(false);
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
        position: PixelPoint,
        scale: f64,
        opacity: f64,
    ) -> Result<(), PinoraError> {
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let (w, h) = scaled_window_size(image.size(), scale);
        let pixels_xrgb = rgba_to_xrgb(&image.pixels.bytes);

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

        let window = event_loop
            .create_window(attrs)
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
                image,
                asset,
                pixels_xrgb,
                scale,
                opacity: opacity.clamp(0.15, 1.0),
                locked: false,
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
        window.set_visible(true);
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
                if self.is_new_capture_key(&event) {
                    self.request_new_capture(event_loop);
                    return;
                }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    self.close_pin(window_id);
                    return;
                }
                // L：锁定；[ ]：透明度；O：OCR；T：词框
                if let Key::Character(c) = &event.logical_key {
                    if c == "l" || c == "L" {
                        if let Some(pin) = self.pins.get_mut(&window_id) {
                            pin.locked = !pin.locked;
                            println!(
                                "pinora: pin {} {}",
                                pin.pin_id,
                                if pin.locked { "LOCKED" } else { "unlocked" }
                            );
                            self.sync_pin_transform(window_id);
                        }
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
                    if let Err(e) = pin.window.drag_window() {
                        eprintln!("pinora: drag_window failed: {e:?}");
                        self.drag_pin = Some(window_id);
                    } else {
                        self.drag_pin = None;
                    }
                }
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
        let locked = pin.locked;
        if let Some(rt) = self.runtime.as_mut()
            && let Some(p) = rt.state_mut().pin_mut(pin_id)
        {
            p.transform = transform;
            p.locked = locked;
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
        let bw = bw as usize;
        let bh = bh as usize;
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
        let sw = pin.image.pixels.size.width as usize;
        let sh = pin.image.pixels.size.height as usize;
        let opacity = pin.opacity;
        let locked = pin.locked;
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
        if bw == sw && bh == sh {
            buffer[..bw * bh].copy_from_slice(&pin.pixels_xrgb);
        } else {
            scale_nearest(&pin.pixels_xrgb, sw, sh, &mut buffer[..bw * bh], bw, bh);
        }
        // softbuffer 无真透明：用不透明度压暗近似
        if opacity < 0.999 {
            apply_opacity_darken(&mut buffer[..bw * bh], opacity);
        }
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
        Key::Character(c) if c == "1" || c == "r" || c == "R" => {
            ov.annotate.tool = AnnotateTool::Rect;
            println!("pinora: tool = Rect");
        }
        Key::Character(c) if c == "2" || c == "a" || c == "A" => {
            ov.annotate.tool = AnnotateTool::Arrow;
            println!("pinora: tool = Arrow");
        }
        Key::Character(c) if c == "3" => {
            ov.annotate.tool = AnnotateTool::Pen;
            println!("pinora: tool = Pen");
        }
        Key::Character(c) if c == "4" || c == "e" || c == "E" => {
            ov.annotate.tool = AnnotateTool::Ellipse;
            println!("pinora: tool = Ellipse");
        }
        Key::Character(c) if c == "5" || c == "m" || c == "M" => {
            ov.annotate.tool = AnnotateTool::Mosaic;
            println!("pinora: tool = Mosaic");
        }
        Key::Character(c) if c == "6" || c == "t" || c == "T" => {
            ov.annotate.tool = AnnotateTool::Text;
            println!("pinora: tool = Text");
        }
        _ => {}
    }
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
            ov.annotate_dirty = true;
        }
        if ov.annotate.image_w != src_sel.size.width || ov.annotate.image_h != src_sel.size.height {
            let tool = ov.annotate.tool;
            let color = ov.annotate.color;
            let stroke = ov.annotate.stroke;
            ov.annotate = AnnotateSession::new(src_sel.size.width, src_sel.size.height);
            ov.annotate.tool = tool;
            ov.annotate.color = color;
            ov.annotate.stroke = stroke;
            ov.annotate_cache = None;
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
                paint_toolbar(&mut ov.frame, img_w, img_h, &ov.toolbar, ov.annotate.tool);
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
            paint_toolbar(&mut ov.frame, img_w, img_h, &ov.toolbar, ov.annotate.tool);
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

/// 在选区变化/标注变化时重烤局部缓存（原图烤制 → 缩放到缓冲选区尺寸）。
fn ensure_annotate_cache(ov: &mut OverlayState, disp_rect: PixelRect) {
    let wh = (disp_rect.size.width, disp_rect.size.height);
    if !ov.annotate_dirty && ov.annotate_cache.is_some() && ov.annotate_cache_wh == wh {
        return;
    }
    ov.annotate_cache = None;
    let Some(src_rect) = ov.active_src_rect else {
        return;
    };
    let Ok(crop) = ov.full_image.crop_local(src_rect) else {
        return;
    };
    let rgba = render_preview_rgba(&crop, &ov.annotate);
    let xrgb = rgba_to_xrgb(&rgba);
    let sw = crop.pixels.size.width as usize;
    let sh = crop.pixels.size.height as usize;
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

fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    let (base, _) = rgba_to_xrgb_and_dim(bytes);
    base
}

/// 无窗口透明时，用压暗模拟 opacity（1.0 = 原色，0.15 = 很暗）。
fn apply_opacity_darken(buf: &mut [u32], opacity: f64) {
    let o = opacity.clamp(0.05, 1.0);
    let factor = (o * 256.0) as u32;
    for px in buf.iter_mut() {
        let r = ((*px >> 16) & 0xff) * factor / 256;
        let g = ((*px >> 8) & 0xff) * factor / 256;
        let b = (*px & 0xff) * factor / 256;
        *px = (r << 16) | (g << 8) | b;
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
        Annotation, AnnotationDoc, CaptureMetadata, DEFAULT_STROKE, DEFAULT_WIDTH, JobResultRef,
        RgbaBuffer,
    };

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
    fn settings_opacity_is_converted_to_bounded_runtime_value() {
        assert!((opacity_from_settings_percent(72) - 0.72).abs() < f64::EPSILON);
        assert!((opacity_from_settings_percent(0) - 0.15).abs() < f64::EPSILON);
        assert!((opacity_from_settings_percent(255) - 1.0).abs() < f64::EPSILON);
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
