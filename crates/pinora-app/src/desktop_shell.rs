//! 统一桌面事件循环：选区 Overlay + 贴图窗口（同一 EventLoop，适配 Wayland）。
//!
//! 选区策略：遮罩立即弹出；全屏预览在后台线程用当前 CaptureProvider 抓取
//! （KDE 优先 spectacle/KWin，避免 xcap→portal）；用户拖选期间通常已完成，
//! Enter 时优先内存裁剪，避免再等 Portal。

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use pinora_core::{
    bake_annotations, render_preview_rgba, ActionId, AnnotateSession, AnnotateTool, CaptureImage,
    CaptureProvider, CaptureRequest, Command, DisplayId, DomainEventKind, ErrorCode, ImageSink,
    OcrResult, PinId, PinTransform, PinoraError, PixelPoint, PixelRect, SelectionSession,
};
use crate::frame_cache::{rgba_to_xrgb_and_dim, FrameCache};
use crate::hotkey::GlobalHotkeyHub;
use crate::image_sink::copy_text_to_system_clipboard;
use crate::ocr::{recognize_image, tesseract_available};
use crate::overlay_toolbar::{
    hit_test as toolbar_hit, layout_toolbar, paint_toolbar, toolbar_bounds, ToolbarAction,
    ToolbarButton,
};
use crate::tray::{AppTray, TrayAction};
use softbuffer::{Context, Surface};
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

/// 运行统一桌面 shell（阻塞直到退出）。
pub fn run_desktop_shell<L, P, C, S>(
    runtime: AppRuntime<L, P, C, S>,
) -> Result<(), PinoraError>
where
    L: SingleInstance + 'static,
    P: CapabilityProbe + 'static,
    C: CaptureProvider + Clone + Send + 'static,
    S: ImageSink + 'static,
{
    let event_loop = EventLoop::new().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("desktop event loop: {e}"))
    })?;

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

    let mut app = DesktopApp {
        runtime: Some(runtime),
        context: None,
        // 先 Idle，等缓存出第一帧再自动截；若用户立刻 F2 也会走缓存/等待
        mode: Mode::StartCapture,
        loading: None,
        overlay: None,
        control: None,
        pins: HashMap::new(),
        drag_pin: None,
        modifiers: ModifiersState::empty(),
        error: None,
        quit: false,
        pending_messages: Vec::new(),
        hotkeys,
        frame_cache: Some(frame_cache),
        start_capture_wait: None,
        tray,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop loop: {e}")))?;

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

struct OverlayState {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    /// 选区外：真实桌面暗化。
    dimmed: Vec<u32>,
    /// 选区内：真实桌面原图（视觉上“透明看到桌面”）。
    base: Vec<u32>,
    frame: Vec<u32>,
    session: SelectionSession,
    phase: OverlayPhase,
    dragging: bool,
    /// 在选区内画标注。
    annotate_dragging: bool,
    annotate: AnnotateSession,
    toolbar: Vec<ToolbarButton>,
    last_cursor: PixelPoint,
    needs_redraw: bool,
    last_drawn_rect: Option<PixelRect>,
    last_present: Instant,
    /// 双击检测。
    last_click_at: Option<Instant>,
    last_click_pos: PixelPoint,
    img_w: u32,
    img_h: u32,
    win_w: u32,
    win_h: u32,
    display_id: DisplayId,
    display_origin: PixelPoint,
    full_image: CaptureImage,
}

struct PinWin {
    pin_id: PinId,
    image: CaptureImage,
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
}


struct DesktopApp<L, P, C, S> {
    runtime: Option<AppRuntime<L, P, C, S>>,
    context: Option<Context<Rc<Window>>>,
    mode: Mode,
    loading: Option<LoadingState>,
    overlay: Option<OverlayState>,
    /// 无选区/加载时的常驻控制窗（收 F2 / Ctrl+N / Ctrl+Q）。
    control: Option<ControlState>,
    pins: HashMap<WindowId, PinWin>,
    drag_pin: Option<WindowId>,
    modifiers: ModifiersState,
    error: Option<PinoraError>,
    quit: bool,
    pending_messages: Vec<String>,
    hotkeys: GlobalHotkeyHub,
    frame_cache: Option<FrameCache>,
    /// 等待 frame-cache 首帧的起始时间；超时走 cold path。
    start_capture_wait: Option<Instant>,
    tray: Option<AppTray>,
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
        if let Some(ov) = self.overlay.as_mut() {
            if ov.needs_redraw {
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
        if matches!(self.mode, Mode::Idle)
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
        if let Some(control) = self.control.as_ref() {
            if control.window.id() == window_id {
                self.handle_control_event(event_loop, event);
                return;
            }
        }
        if let Some(ov) = self.overlay.as_ref() {
            if ov.window.id() == window_id {
                self.handle_overlay_event(event_loop, event);
                return;
            }
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
                    if let Some(rt) = self.runtime.as_mut() {
                        if let Ok(_) = rt.dispatch(command) {
                            self.pending_messages.push(format!(
                                "pinora: activated (count={})",
                                rt.state().activation_count
                            ));
                        }
                    }
                    need_capture = true;
                }
                other => {
                    if let Some(rt) = self.runtime.as_mut() {
                        if let Err(e) = rt.dispatch(other) {
                            eprintln!("pinora: forwarded command failed: {e}");
                        }
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
        let cache_ready = self
            .frame_cache
            .as_ref()
            .and_then(|c| c.peek())
            .is_some();
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
        if let Some(cache) = &self.frame_cache {
            if let Some(frame) = cache
                .take_if_fresh(Duration::from_secs(2))
                .or_else(|| cache.take_any())
            {
                let age_ms = frame.age().as_secs_f64() * 1000.0;
                println!(
                    "pinora: overlay INSTANT from cache (age {:.0}ms, {}x{})",
                    age_ms,
                    frame.image.pixels.size.width,
                    frame.image.pixels.size.height
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
            let _ = control.window.focus_window();
            return Ok(());
        }
        let attrs = Window::default_attributes()
            .with_title("Pinora — F2 截图 · Ctrl+N 截图 · Ctrl+Q 退出")
            .with_inner_size(PhysicalSize::new(420, 64))
            .with_decorations(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);
        let window = event_loop.create_window(attrs).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("control window: {e}"))
        })?;
        let window = Rc::new(window);
        let _ = window.focus_window();
        self.control = Some(ControlState { window });
        println!("pinora: idle — focus control window, then F2/Ctrl+N to capture");
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
                } else if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    // Esc 在控制窗：若有贴图则聚焦贴图，否则退出
                    if let Some(pin) = self.pins.values().next() {
                        let _ = pin.window.focus_window();
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
            ov.window.set_visible(false);
        }
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
            let _ = pin.window.focus_window();
        }
    }

    fn poll_loading_to_overlay(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), PinoraError> {
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
        let _ = window.focus_window();

        let mut surface = Surface::new(context, window.clone()).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("overlay surface: {e}"))
        })?;
        let size = window.inner_size();
        let win_w = size.width.max(1);
        let win_h = size.height.max(1);
        if let (Some(w), Some(h)) = (NonZeroU32::new(win_w), NonZeroU32::new(win_h)) {
            let _ = surface.resize(w, h);
        }

        let frame = prep.dimmed.clone();
        println!(
            "pinora: overlay ready {}x{} display={} — 拖选后出工具栏；双击复制 · 中键贴图 · Enter贴图",
            img_w, img_h, display_id
        );
        window.set_ime_allowed(true);

        self.overlay = Some(OverlayState {
            window: window.clone(),
            surface,
            dimmed: prep.dimmed,
            base: prep.base,
            frame,
            session: SelectionSession::new()
                .with_bounds(PixelRect::new(0, 0, img_w, img_h))
                .with_min_edge(2),
            phase: OverlayPhase::Selecting,
            dragging: false,
            annotate_dragging: false,
            annotate: AnnotateSession::new(1, 1),
            toolbar: Vec::new(),
            last_cursor: PixelPoint::new(0, 0),
            needs_redraw: true,
            last_drawn_rect: None,
            last_present: Instant::now()
                .checked_sub(MIN_FRAME_INTERVAL * 2)
                .unwrap_or_else(Instant::now),
            last_click_at: None,
            last_click_pos: PixelPoint::new(0, 0),
            img_w,
            img_h,
            win_w,
            win_h,
            display_id,
            display_origin,
            full_image: prep.image,
        });
        self.mode = Mode::Idle;
        window.request_redraw();
        Ok(())
    }

    fn handle_overlay_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        // 先处理会拿走整个 overlay 的全局键，避免与 ov 可变借用冲突
        if let WindowEvent::KeyboardInput { event: ref key, .. } = event {
            if key.state.is_pressed() {
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
                    if let Some(ov) = self.overlay.as_mut() {
                        if ov.annotate.draft.is_some() {
                            ov.annotate.cancel_draft();
                            ov.annotate_dragging = false;
                            ov.needs_redraw = true;
                            return;
                        }
                    }
                    self.cancel_overlay();
                    return;
                }
                if matches!(key.logical_key, Key::Named(NamedKey::Enter)) {
                    // 文本草稿：先提交文字；Ctrl+Enter 同样。裸 Enter 贴图。
                    if let Some(ov) = self.overlay.as_mut() {
                        if ov.annotate.is_text_editing() {
                            ov.annotate.commit();
                            ov.needs_redraw = true;
                            println!("pinora: text committed on overlay");
                            return;
                        }
                    }
                    if let Err(e) = self.finish_overlay_action(event_loop, OverlayFinish::Pin) {
                        eprintln!("pinora: pin failed: {e}");
                    }
                    return;
                }
                if matches!(key.logical_key, Key::Named(NamedKey::Space)) {
                    if let Some(ov) = self.overlay.as_mut() {
                        if ov.annotate.is_text_editing() {
                            ov.annotate.text_push(" ");
                            ov.needs_redraw = true;
                            return;
                        }
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
            WindowEvent::Ime(Ime::Commit(text)) => {
                if ov.phase == OverlayPhase::Ready
                    && ov.annotate.is_text_editing()
                    && !text.is_empty()
                {
                    ov.annotate.text_push(&text);
                    ov.needs_redraw = true;
                }
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
                    position.x,
                    position.y,
                    ov.win_w,
                    ov.win_h,
                    ov.img_w,
                    ov.img_h,
                );
                if ov.dragging {
                    ov.session.update_cursor(ov.last_cursor);
                    ov.phase = OverlayPhase::Selecting;
                    ov.toolbar.clear();
                    ov.needs_redraw = true;
                } else if ov.annotate_dragging {
                    if let Ok(sel) = ov.session.try_confirm() {
                        let local = PixelPoint::new(
                            ov.last_cursor.x - sel.origin.x,
                            ov.last_cursor.y - sel.origin.y,
                        );
                        ov.annotate.drag(local);
                        ov.needs_redraw = true;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = paint_overlay(ov) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                ov.win_w = size.width.max(1);
                ov.win_h = size.height.max(1);
                if let (Some(w), Some(h)) = (NonZeroU32::new(ov.win_w), NonZeroU32::new(ov.win_h))
                {
                    let _ = ov.surface.resize(w, h);
                }
                ov.last_drawn_rect = None;
                if !ov.dimmed.is_empty() {
                    ov.frame = ov.dimmed.clone();
                }
                if ov.phase == OverlayPhase::Ready {
                    if let Ok(sel) = ov.session.try_confirm() {
                        ov.toolbar = layout_toolbar(sel, ov.img_w, ov.img_h);
                    }
                }
                ov.needs_redraw = true;
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

                // 工具栏点击
                if ov.phase == OverlayPhase::Ready {
                    if let Some(action) = toolbar_hit(&ov.toolbar, p) {
                        self.apply_toolbar_action(event_loop, action);
                        return;
                    }
                }

                // 双击选区 → 复制
                if ov.phase == OverlayPhase::Ready {
                    if let Ok(sel) = ov.session.try_confirm() {
                        if sel.contains_point(p) {
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
                                if let Err(e) =
                                    self.finish_overlay_action(event_loop, OverlayFinish::Copy)
                                {
                                    eprintln!("pinora: double-click copy failed: {e}");
                                }
                                return;
                            }
                            // 选区内：开始标注
                            let local = PixelPoint::new(p.x - sel.origin.x, p.y - sel.origin.y);
                            ov.annotate.begin(local);
                            ov.annotate_dragging = ov.annotate.tool != AnnotateTool::Text;
                            ov.needs_redraw = true;
                            return;
                        }
                    }
                }

                // 选区外 / 工具栏外：新选区
                if let Some(bounds) = toolbar_bounds(&ov.toolbar) {
                    if bounds.contains_point(p) {
                        return;
                    }
                }
                ov.phase = OverlayPhase::Selecting;
                ov.toolbar.clear();
                ov.annotate = AnnotateSession::new(1, 1);
                ov.annotate_dragging = false;
                ov.dragging = true;
                ov.session.begin_drag(p);
                ov.needs_redraw = true;
            }
            ElementState::Released => {
                let Some(ov) = self.overlay.as_mut() else {
                    return;
                };
                if ov.dragging {
                    ov.dragging = false;
                    if let Ok(sel) = ov.session.try_confirm() {
                        ov.phase = OverlayPhase::Ready;
                        ov.toolbar = layout_toolbar(sel, ov.img_w, ov.img_h);
                        let tool = ov.annotate.tool;
                        let color = ov.annotate.color;
                        let stroke = ov.annotate.stroke;
                        ov.annotate = AnnotateSession::new(sel.size.width, sel.size.height);
                        ov.annotate.tool = tool;
                        ov.annotate.color = color;
                        ov.annotate.stroke = stroke;
                        println!(
                            "pinora: selection {}x{} — 工具栏就绪 | 双击复制 中键/Enter贴图",
                            sel.size.width, sel.size.height
                        );
                    } else {
                        ov.phase = OverlayPhase::Selecting;
                        ov.toolbar.clear();
                    }
                    ov.needs_redraw = true;
                } else if ov.annotate_dragging {
                    ov.annotate.commit();
                    ov.annotate_dragging = false;
                    ov.needs_redraw = true;
                } else if ov.annotate.is_text_editing() {
                    // 文本：松手不提交，等再次点击或继续键入
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
                if let Some(ov) = self.overlay.as_mut() {
                    ov.annotate.tool = tool;
                    ov.needs_redraw = true;
                    println!("pinora: tool = {tool:?}");
                }
            }
        }
    }

    fn overlay_ocr(&mut self) {
        let Some(ov) = self.overlay.as_ref() else {
            return;
        };
        let Ok(sel) = ov.session.try_confirm() else {
            eprintln!("pinora: OCR 需要有效选区");
            return;
        };
        let image = match self.crop_overlay_image(true) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("pinora: OCR crop: {e}");
                return;
            }
        };
        println!(
            "pinora: OCR on selection {}x{}…",
            sel.size.width, sel.size.height
        );
        match recognize_image(&image) {
            Ok(result) => {
                let preview: String = result.full_text.chars().take(240).collect();
                println!(
                    "pinora: OCR ok — {} words\n---\n{}\n---",
                    result.word_count(),
                    if preview.is_empty() {
                        "(empty)"
                    } else {
                        &preview
                    }
                );
                if !result.full_text.trim().is_empty() {
                    match copy_text_to_system_clipboard(&result.full_text) {
                        Ok(b) => println!("pinora: system clipboard ← text via {b}"),
                        Err(e) => eprintln!("pinora: text clipboard: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("pinora: OCR failed: {e}"),
        }
    }

    /// 从当前 overlay 选区裁剪图像，可选烧录标注。
    fn crop_overlay_image(&self, bake: bool) -> Result<CaptureImage, PinoraError> {
        let ov = self.overlay.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "overlay missing")
        })?;
        let local = ov.session.try_confirm()?;
        let crop = ov.full_image.crop_local(local)?;
        if bake && !ov.annotate.doc.is_empty() {
            Ok(bake_annotations(&crop, &ov.annotate.doc))
        } else if bake {
            // 仍可能有进行中的草稿
            if ov.annotate.draft.is_some() {
                let rgba = render_preview_rgba(&crop, &ov.annotate);
                // 把预览写回 CaptureImage
                let mut img = crop;
                if rgba.len() == img.pixels.bytes.len() {
                    img.pixels.bytes = rgba;
                }
                Ok(img)
            } else {
                Ok(crop)
            }
        } else {
            Ok(crop)
        }
    }

    fn finish_overlay_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        action: OverlayFinish,
    ) -> Result<(), PinoraError> {
        let ov = self.overlay.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "overlay missing")
        })?;
        let local = match ov.session.try_confirm() {
            Ok(r) => r,
            Err(_) => {
                println!("pinora: 尚无有效选区");
                return Ok(());
            }
        };
        let global = PixelRect::new(
            ov.display_origin.x.saturating_add(local.origin.x),
            ov.display_origin.y.saturating_add(local.origin.y),
            local.size.width,
            local.size.height,
        );
        let image = self.crop_overlay_image(true)?;
        let position = PixelPoint::new(global.origin.x, global.origin.y);

        // 关闭 overlay
        if let Some(ov) = self.overlay.take() {
            ov.window.set_visible(false);
        }
        self.mode = Mode::Idle;
        self.resume_frame_cache();

        match action {
            OverlayFinish::Copy => {
                if let Some(rt) = self.runtime.as_mut() {
                    // 临时创建 pin 状态以便 CopyLast？直接 sink
                    let _ = rt.dispatch(Command::create_pin(image.clone(), position));
                    let _ = rt.dispatch(Command::invoke_action(ActionId::CopyLastCapture));
                }
                println!(
                    "pinora: copied {}x{} (双击/工具栏复制)",
                    image.pixels.size.width, image.pixels.size.height
                );
            }
            OverlayFinish::Save => {
                if let Some(rt) = self.runtime.as_mut() {
                    let _ = rt.dispatch(Command::create_pin(image.clone(), position));
                    if let Ok(save) = rt.dispatch(Command::invoke_action(ActionId::SaveLastCapture))
                    {
                        for event in &save.events {
                            if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind
                            {
                                println!("pinora: saved {image_id} -> {}", path.display());
                            }
                        }
                    }
                }
            }
            OverlayFinish::Pin => {
                self.open_pin_from_image(event_loop, image, position)?;
            }
        }
        Ok(())
    }

    fn open_pin_from_image(
        &mut self,
        event_loop: &ActiveEventLoop,
        image: CaptureImage,
        position: PixelPoint,
    ) -> Result<(), PinoraError> {
        let rt = self.runtime.as_mut().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "runtime missing")
        })?;
        let pin = rt.dispatch(Command::create_pin(image.clone(), position))?;
        let pin_id = pin
            .events
            .iter()
            .find_map(|e| match e.event.kind {
                DomainEventKind::PinCreated { pin_id, .. } => Some(pin_id),
                _ => None,
            })
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "missing PinCreated"))?;

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

        if let Ok(save) = rt.dispatch(Command::invoke_action(ActionId::SaveLastCapture)) {
            for event in &save.events {
                if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind {
                    println!("pinora: saved {image_id} -> {}", path.display());
                }
            }
        }
        let _ = rt.dispatch(Command::invoke_action(ActionId::CopyLastCapture));

        self.spawn_pin(event_loop, pin_id, image, position, 1.0)?;
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        Ok(())
    }

    fn cancel_overlay(&mut self) {
        if let Some(ov) = self.overlay.take() {
            ov.window.set_visible(false);
        }
        // Esc 只取消选区，绝不自动再截；再截仅 F2 / Ctrl+N
        self.mode = Mode::Idle;
        self.resume_frame_cache();
        println!("pinora: selection cancelled (F2/Ctrl+N 再截，Ctrl+Q 退出)");
        if let Some(pin) = self.pins.values().next() {
            let _ = pin.window.focus_window();
        }
    }

    fn spawn_pin(
        &mut self,
        event_loop: &ActiveEventLoop,
        pin_id: PinId,
        image: CaptureImage,
        position: PixelPoint,
        scale: f64,
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
        let _ = window.set_outer_position(PhysicalPosition::new(position.x, position.y));

        let mut surface = Surface::new(context, window.clone()).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("pin surface: {e}"))
        })?;
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            surface.resize(nw, nh).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("pin surface resize: {e}"))
            })?;
        }

        let id = window.id();
        self.pins.insert(
            id,
            PinWin {
                pin_id,
                image,
                pixels_xrgb,
                scale,
                opacity: 1.0,
                locked: false,
                window: window.clone(),
                surface,
                ocr: None,
                ocr_show_boxes: true,
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
            let _ = window.set_outer_position(PhysicalPosition::new(position.x, position.y));
        }

        // 钉位后再置顶，准备在 overlay 撤掉后露出来
        window.set_window_level(WindowLevel::AlwaysOnTop);
        let _ = window.focus_window();
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
                    if c == "t" || c == "T" {
                        if let Some(pin) = self.pins.get_mut(&window_id) {
                            pin.ocr_show_boxes = !pin.ocr_show_boxes;
                            println!(
                                "pinora: pin {} OCR boxes {}",
                                pin.pin_id,
                                if pin.ocr_show_boxes { "ON" } else { "OFF" }
                            );
                            pin.window.request_redraw();
                        }
                        return;
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(pin) = self.pins.get(&window_id) {
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
                self.drag_pin = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
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
                    let _ = pin.window.set_outer_position(PhysicalPosition::new(
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
        println!("pinora: pin {pin_id} OCR…");
        let image = pin.image.clone();
        match recognize_image(&image) {
            Ok(result) => {
                let preview: String = result
                    .full_text
                    .chars()
                    .take(200)
                    .collect();
                println!(
                    "pinora: pin {pin_id} OCR ok — {} words, {} lines, langs={:?}\n---\n{}\n---",
                    result.word_count(),
                    result.lines.len(),
                    result.languages,
                    if preview.is_empty() {
                        "(empty)"
                    } else {
                        &preview
                    }
                );
                if !result.full_text.trim().is_empty() {
                    match copy_text_to_system_clipboard(&result.full_text) {
                        Ok(backend) => {
                            println!("pinora: system clipboard ← text via {backend}");
                        }
                        Err(e) => {
                            eprintln!("pinora: text clipboard skipped: {e}");
                        }
                    }
                }
                if let Some(pin) = self.pins.get_mut(&window_id) {
                    pin.ocr = Some(result);
                    pin.ocr_show_boxes = true;
                    pin.window.request_redraw();
                }
            }
            Err(e) => {
                eprintln!("pinora: pin {pin_id} OCR failed: {e}");
            }
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
        if let Some(rt) = self.runtime.as_mut() {
            if let Some(p) = rt.state_mut().pin_mut(pin_id) {
                p.transform = transform;
                p.locked = locked;
            }
        }
    }

    fn close_pin(&mut self, window_id: WindowId) {
        if let Some(pin) = self.pins.remove(&window_id) {
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
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(bw), NonZeroU32::new(bh)) {
            if let Err(e) = pin.surface.resize(nw, nh) {
                return Err(PinoraError::new(
                    ErrorCode::Internal,
                    format!("pin surface resize: {e}"),
                ));
            }
        }
        let bw = bw as usize;
        let bh = bh as usize;
        let mut buffer = pin.surface.buffer_mut().map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("pin buffer: {e}"))
        })?;
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
        let ocr_boxes: Vec<PixelRect> = if show_ocr {
            pin.ocr
                .as_ref()
                .map(|r| r.lines.iter().flat_map(|l| l.words.iter().map(|w| w.bbox)).collect())
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
            for rect in ocr_boxes {
                let x0 = (rect.origin.x as f64 * sx).round() as i32;
                let y0 = (rect.origin.y as f64 * sy).round() as i32;
                let x1 = (rect.right() as f64 * sx).round() as i32;
                let y1 = (rect.bottom() as f64 * sy).round() as i32;
                draw_rect_outline_xrgb(
                    &mut buffer[..bw * bh],
                    bw,
                    bh,
                    x0,
                    y0,
                    x1.max(x0 + 1),
                    y1.max(y0 + 1),
                    0x00_22_EE_66,
                );
            }
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
            && matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::KeyQ)
            )
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
        Key::Character(c) if (c == "c" || c == "C") && !modifiers.control_key() => {
            if ov.phase == OverlayPhase::Ready {
                ov.annotate.cycle_color();
                ov.needs_redraw = true;
                println!("pinora: stroke color rgba{:?}", ov.annotate.color);
            }
        }
        Key::Character(c) if c == "+" || c == "=" => {
            if ov.phase == OverlayPhase::Ready {
                ov.annotate.stroke_up();
                ov.needs_redraw = true;
            }
        }
        Key::Character(c) if c == "-" || c == "_" => {
            if ov.phase == OverlayPhase::Ready {
                ov.annotate.stroke_down();
                ov.needs_redraw = true;
            }
        }
        Key::Character(c) if (c == "z" || c == "Z") && modifiers.control_key() => {
            if ov.phase == OverlayPhase::Ready {
                ov.annotate.doc.undo();
                ov.needs_redraw = true;
            }
        }
        Key::Character(c) if c == "1" || c == "r" || c == "R" => {
            ov.annotate.tool = AnnotateTool::Rect;
            println!("pinora: tool = Rect");
            ov.needs_redraw = true;
        }
        Key::Character(c) if c == "2" || c == "a" || c == "A" => {
            ov.annotate.tool = AnnotateTool::Arrow;
            println!("pinora: tool = Arrow");
            ov.needs_redraw = true;
        }
        Key::Character(c) if c == "3" => {
            ov.annotate.tool = AnnotateTool::Pen;
            println!("pinora: tool = Pen");
            ov.needs_redraw = true;
        }
        Key::Character(c) if c == "4" || c == "e" || c == "E" => {
            ov.annotate.tool = AnnotateTool::Ellipse;
            println!("pinora: tool = Ellipse");
            ov.needs_redraw = true;
        }
        Key::Character(c) if c == "5" || c == "m" || c == "M" => {
            ov.annotate.tool = AnnotateTool::Mosaic;
            println!("pinora: tool = Mosaic");
            ov.needs_redraw = true;
        }
        Key::Character(c) if c == "6" || c == "t" || c == "T" => {
            ov.annotate.tool = AnnotateTool::Text;
            println!("pinora: tool = Text");
            ov.needs_redraw = true;
        }
        _ => {}
    }
}

fn refresh_overlay_ready(ov: &mut OverlayState) {
    if let Ok(sel) = ov.session.try_confirm() {
        if ov.phase == OverlayPhase::Ready {
            ov.toolbar = layout_toolbar(sel, ov.img_w, ov.img_h);
            if ov.annotate.image_w != sel.size.width || ov.annotate.image_h != sel.size.height {
                let tool = ov.annotate.tool;
                let color = ov.annotate.color;
                let stroke = ov.annotate.stroke;
                ov.annotate = AnnotateSession::new(sel.size.width, sel.size.height);
                ov.annotate.tool = tool;
                ov.annotate.color = color;
                ov.annotate.stroke = stroke;
            }
        }
    }
    ov.needs_redraw = true;
}

fn paint_overlay(ov: &mut OverlayState) -> Result<(), PinoraError> {
    let img_w = ov.img_w as usize;
    let img_h = ov.img_h as usize;
    let new_rect = ov.session.preview_rect();

    // 每帧从 dimmed 重建：选区洞 + 标注 + 工具栏（标注会变，脏矩形不够）
    if ov.frame.len() == ov.dimmed.len() {
        ov.frame.copy_from_slice(&ov.dimmed);
    } else {
        ov.frame = ov.dimmed.clone();
    }

    if let Some(rect) = new_rect {
        // 选区内：原图或带标注预览
        let use_annotate = ov.phase == OverlayPhase::Ready
            && (ov.annotate.doc.len() > 0 || ov.annotate.draft.is_some())
            && ov.annotate.image_w == rect.size.width
            && ov.annotate.image_h == rect.size.height;
        if use_annotate {
            if let Ok(crop) = ov.full_image.crop_local(rect) {
                let rgba = render_preview_rgba(&crop, &ov.annotate);
                blit_rgba_into_xrgb(&mut ov.frame, img_w, img_h, rect, &rgba);
            } else {
                blit_rect(&mut ov.frame, &ov.base, img_w, img_h, rect);
            }
        } else {
            blit_rect(&mut ov.frame, &ov.base, img_w, img_h, rect);
        }
        let x0 = rect.origin.x.max(0) as usize;
        let y0 = rect.origin.y.max(0) as usize;
        let x1 = (rect.right() as usize).min(img_w);
        let y1 = (rect.bottom() as usize).min(img_h);
        draw_rect_border(&mut ov.frame, img_w, img_h, x0, y0, x1, y1, 0x00_FF_CC_33);

        // 尺寸角标
        // （省略文字；工具栏已足够）
    }
    ov.last_drawn_rect = new_rect;

    if ov.phase == OverlayPhase::Ready && !ov.toolbar.is_empty() {
        paint_toolbar(
            &mut ov.frame,
            img_w,
            img_h,
            &ov.toolbar,
            ov.annotate.tool,
        );
    }

    let mut buffer = ov.surface.buffer_mut().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("overlay buffer: {e}"))
    })?;
    let bw = ov.win_w as usize;
    let bh = ov.win_h as usize;
    let needed = bw * bh;
    if buffer.len() < needed {
        return Err(PinoraError::new(
            ErrorCode::Internal,
            "overlay buffer size mismatch",
        ));
    }
    if bw == img_w && bh == img_h {
        buffer[..needed].copy_from_slice(&ov.frame);
    } else {
        scale_nearest(&ov.frame, img_w, img_h, &mut buffer[..needed], bw, bh);
    }
    buffer
        .present()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay present: {e}")))?;
    ov.last_present = Instant::now();
    Ok(())
}

/// 将 RGBA 块写入 XRGB frame 的 rect 区域。
fn blit_rgba_into_xrgb(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    rgba: &[u8],
) {
    let rw = rect.size.width as usize;
    let rh = rect.size.height as usize;
    if rw == 0 || rh == 0 || rgba.len() < rw * rh * 4 {
        return;
    }
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    for row in 0..rh {
        let dy = y0 + row;
        if dy >= height {
            break;
        }
        for col in 0..rw {
            let dx = x0 + col;
            if dx >= stride {
                break;
            }
            let si = (row * rw + col) * 4;
            let r = rgba[si] as u32;
            let g = rgba[si + 1] as u32;
            let b = rgba[si + 2] as u32;
            frame[dy * stride + dx] = (r << 16) | (g << 8) | b;
        }
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

fn draw_rect_border(
    buf: &mut [u32],
    stride: usize,
    height: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    color: u32,
) {
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
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: u32,
) {
    if stride == 0 || height == 0 {
        return;
    }
    let x0 = x0.clamp(0, stride as i32 - 1) as usize;
    let x1 = x1.clamp(0, stride as i32 - 1) as usize;
    let y0 = y0.clamp(0, height as i32 - 1) as usize;
    let y1 = y1.clamp(0, height as i32 - 1) as usize;
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
