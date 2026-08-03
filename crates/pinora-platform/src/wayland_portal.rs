//! XDG GlobalShortcuts Portal 适配器。
//!
//! 纯 Wayland 不支持 `global-hotkey` 的 X11 后端。这里仅在检测到 Wayland 会话时
//! 创建一个后台 D-Bus worker：GUI 线程通过无阻塞队列读取动作和状态，绝不等待
//! Portal 的授权对话、`Request::Response` 或长期信号流。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use async_channel::{Receiver, Sender};
use futures_lite::{StreamExt, future};
use pinora_core::{ActionId, HotkeyBinding, HotkeyCode, HotkeyModifiers};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const GLOBAL_SHORTCUTS_INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";
const REGION_SHORTCUT_ID: &str = "capture-region";
const FULL_DISPLAY_SHORTCUT_ID: &str = "capture-full-display";
// CreateSession/BindShortcuts/Activated are consumed according to the v2 contract.
const PORTAL_MIN_VERSION: u32 = 2;
const PORTAL_QUEUE_CAPACITY: usize = 16;

static TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// GUI 线程可消费的 Portal 状态更新；内容固定，不携带 D-Bus 原始错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalAvailability {
    Available,
    Unavailable(PortalFailure),
}

/// 受控的 Portal 失败种类，供日志与能力状态使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalFailure {
    SessionBusUnavailable,
    InterfaceUnavailable,
    VersionUnsupported,
    SessionRejected,
    BindingRejected,
    Disconnected,
}

impl PortalFailure {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::SessionBusUnavailable => "portal_session_bus_unavailable",
            Self::InterfaceUnavailable => "portal_interface_unavailable",
            Self::VersionUnsupported => "portal_version_unsupported",
            Self::SessionRejected => "portal_session_rejected",
            Self::BindingRejected => "portal_binding_rejected",
            Self::Disconnected => "portal_disconnected",
        }
    }
}

#[derive(Debug)]
enum PortalCommand {
    Rebind {
        region_hotkey: HotkeyBinding,
        full_display_hotkey: HotkeyBinding,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
enum PortalEvent {
    Availability(PortalAvailability),
    Action(ActionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PortalPoll {
    pub(crate) availability: Option<PortalAvailability>,
}

/// 一个长生命周期 Portal worker 的 GUI 侧句柄。
pub(crate) struct WaylandPortalHotkeys {
    commands: Sender<PortalCommand>,
    events: Receiver<PortalEvent>,
}

impl WaylandPortalHotkeys {
    /// 只在 Linux Wayland 会话尝试启动。创建 worker 本身不连接 D-Bus，因而不会
    /// 阻塞 tray-only 启动或 `winit` GUI 线程。
    pub(crate) fn start(
        region_hotkey: HotkeyBinding,
        full_display_hotkey: HotkeyBinding,
    ) -> Option<Self> {
        if !is_wayland_session() {
            return None;
        }

        let (command_tx, command_rx) = async_channel::bounded(PORTAL_QUEUE_CAPACITY);
        let (event_tx, event_rx) = async_channel::bounded(PORTAL_QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("pinora-wayland-hotkeys".into())
            .spawn(move || {
                zbus::block_on(run_worker(
                    command_rx,
                    event_tx,
                    region_hotkey,
                    full_display_hotkey,
                ));
            })
            .ok()?;

        Some(Self {
            commands: command_tx,
            events: event_rx,
        })
    }

    /// 异步替换偏好键。发送失败说明 worker 已离线，调用方仍可安全保存设置并
    /// 维持 tray/IPC 入口；后续 `poll` 会把能力标记为受限。
    pub(crate) fn rebind(
        &self,
        region_hotkey: HotkeyBinding,
        full_display_hotkey: HotkeyBinding,
    ) -> bool {
        self.commands
            .try_send(PortalCommand::Rebind {
                region_hotkey,
                full_display_hotkey,
            })
            .is_ok()
    }

    /// 仅在 GUI 线程非阻塞地提取已经完成的 worker 事件。
    pub(crate) fn poll_actions(&self, actions: &mut Vec<ActionId>) -> PortalPoll {
        let mut availability = None;
        while let Ok(event) = self.events.try_recv() {
            match event {
                PortalEvent::Availability(next) => availability = Some(next),
                PortalEvent::Action(action) => actions.push(action),
            }
        }
        PortalPoll { availability }
    }
}

impl Drop for WaylandPortalHotkeys {
    fn drop(&mut self) {
        let _ = self.commands.try_send(PortalCommand::Shutdown);
    }
}

pub(crate) fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .is_ok_and(|session_type| session_type.eq_ignore_ascii_case("wayland"))
}

const fn portal_version_supported(version: u32) -> bool {
    version >= PORTAL_MIN_VERSION
}

async fn run_worker(
    commands: Receiver<PortalCommand>,
    events: Sender<PortalEvent>,
    mut region_hotkey: HotkeyBinding,
    mut full_display_hotkey: HotkeyBinding,
) {
    let result = run_worker_inner(
        &commands,
        &events,
        &mut region_hotkey,
        &mut full_display_hotkey,
    )
    .await;
    if let Err(failure) = result {
        let _ = events
            .send(PortalEvent::Availability(PortalAvailability::Unavailable(
                failure,
            )))
            .await;
    }
}

async fn run_worker_inner(
    commands: &Receiver<PortalCommand>,
    events: &Sender<PortalEvent>,
    region_hotkey: &mut HotkeyBinding,
    full_display_hotkey: &mut HotkeyBinding,
) -> Result<(), PortalFailure> {
    let connection = zbus::Connection::session()
        .await
        .map_err(|_| PortalFailure::SessionBusUnavailable)?;
    let portal = zbus::Proxy::new(
        &connection,
        PORTAL_DESTINATION,
        PORTAL_PATH,
        GLOBAL_SHORTCUTS_INTERFACE,
    )
    .await
    .map_err(|_| PortalFailure::InterfaceUnavailable)?;
    let version: u32 = portal
        .get_property("version")
        .await
        .map_err(|_| PortalFailure::InterfaceUnavailable)?;
    if !portal_version_supported(version) {
        return Err(PortalFailure::VersionUnsupported);
    }

    let mut session =
        bind_session(&connection, &portal, *region_hotkey, *full_display_hotkey).await?;
    let _ = events
        .send(PortalEvent::Availability(PortalAvailability::Available))
        .await;

    loop {
        let signal = async { WorkerInput::Signal(session.signals.next().await) };
        let command = async { WorkerInput::Command(commands.recv().await.ok()) };
        let input = future::race(signal, command).await;
        match input {
            WorkerInput::Command(None) | WorkerInput::Command(Some(PortalCommand::Shutdown)) => {
                close_session(&connection, &session.handle).await;
                return Ok(());
            }
            WorkerInput::Command(Some(PortalCommand::Rebind {
                region_hotkey: next_region,
                full_display_hotkey: next_full,
            })) => {
                close_session(&connection, &session.handle).await;
                match bind_session(&connection, &portal, next_region, next_full).await {
                    Ok(next_session) => {
                        *region_hotkey = next_region;
                        *full_display_hotkey = next_full;
                        session = next_session;
                        let _ = events
                            .send(PortalEvent::Availability(PortalAvailability::Available))
                            .await;
                    }
                    Err(failure) => {
                        let _ = events
                            .send(PortalEvent::Availability(PortalAvailability::Unavailable(
                                failure,
                            )))
                            .await;
                        return Err(failure);
                    }
                }
            }
            WorkerInput::Signal(None) => return Err(PortalFailure::Disconnected),
            WorkerInput::Signal(Some(message)) => {
                if let Some(action) = action_from_signal(&message, &session) {
                    let _ = events.send(PortalEvent::Action(action)).await;
                }
            }
        }
    }
}

enum WorkerInput {
    Command(Option<PortalCommand>),
    Signal(Option<zbus::Message>),
}

struct PortalSession {
    handle: OwnedObjectPath,
    accepted_shortcuts: HashSet<String>,
    signals: zbus::proxy::SignalStream<'static>,
}

async fn bind_session(
    connection: &zbus::Connection,
    portal: &zbus::Proxy<'_>,
    region_hotkey: HotkeyBinding,
    full_display_hotkey: HotkeyBinding,
) -> Result<PortalSession, PortalFailure> {
    let (request_token, session_token) = next_tokens();
    let mut options = HashMap::new();
    options.insert("handle_token", Value::from(request_token.clone()));
    options.insert("session_handle_token", Value::from(session_token));
    let create_request_path =
        request_path(connection, &request_token).ok_or(PortalFailure::SessionRejected)?;
    let mut response_stream = prepare_response_stream(connection, create_request_path.clone())
        .await
        .map_err(|_| PortalFailure::SessionRejected)?;
    let request: OwnedObjectPath = portal
        .call("CreateSession", &options)
        .await
        .map_err(|_| PortalFailure::SessionRejected)?;
    if request != create_request_path {
        return Err(PortalFailure::SessionRejected);
    }
    let session = session_from_response(&mut response_stream).await?;

    // 先订阅再发送 BindShortcuts，避免后端在请求响应后的窗口内触发时丢信号。
    let signals = portal
        .receive_all_signals()
        .await
        .map_err(|_| PortalFailure::Disconnected)?;
    let shortcuts = requested_shortcuts(region_hotkey, full_display_hotkey);
    let mut bind_options = HashMap::new();
    let bind_token = next_token("bind");
    bind_options.insert("handle_token", Value::from(bind_token.clone()));
    let bind_request_path =
        request_path(connection, &bind_token).ok_or(PortalFailure::BindingRejected)?;
    let mut bind_response_stream = prepare_response_stream(connection, bind_request_path.clone())
        .await
        .map_err(|_| PortalFailure::BindingRejected)?;
    let request: OwnedObjectPath = portal
        .call(
            "BindShortcuts",
            &(session.clone(), shortcuts, "", bind_options),
        )
        .await
        .map_err(|_| PortalFailure::BindingRejected)?;
    if request != bind_request_path {
        return Err(PortalFailure::BindingRejected);
    }
    let accepted_shortcuts = shortcuts_from_response(&mut bind_response_stream).await?;
    if accepted_shortcuts.is_empty() {
        close_session(connection, &session).await;
        return Err(PortalFailure::BindingRejected);
    }
    Ok(PortalSession {
        handle: session,
        accepted_shortcuts,
        signals,
    })
}

async fn session_from_response(
    response_stream: &mut zbus::proxy::SignalStream<'static>,
) -> Result<OwnedObjectPath, PortalFailure> {
    let response = read_request_response(response_stream)
        .await
        .map_err(|_| PortalFailure::SessionRejected)?;
    let Some(session_handle) = response.get("session_handle") else {
        return Err(PortalFailure::SessionRejected);
    };
    object_path_from_value(session_handle).ok_or(PortalFailure::SessionRejected)
}

async fn shortcuts_from_response(
    response_stream: &mut zbus::proxy::SignalStream<'static>,
) -> Result<HashSet<String>, PortalFailure> {
    let response = read_request_response(response_stream)
        .await
        .map_err(|_| PortalFailure::BindingRejected)?;
    let Some(shortcuts) = response.get("shortcuts") else {
        return Err(PortalFailure::BindingRejected);
    };
    accepted_shortcut_ids(shortcuts).ok_or(PortalFailure::BindingRejected)
}

async fn prepare_response_stream(
    connection: &zbus::Connection,
    request: OwnedObjectPath,
) -> Result<zbus::proxy::SignalStream<'static>, ()> {
    let proxy = zbus::Proxy::new(connection, PORTAL_DESTINATION, request, REQUEST_INTERFACE)
        .await
        .map_err(|_| ())?;
    proxy.receive_signal("Response").await.map_err(|_| ())
}

async fn read_request_response(
    responses: &mut zbus::proxy::SignalStream<'static>,
) -> Result<HashMap<String, OwnedValue>, ()> {
    let Some(message) = responses.next().await else {
        return Err(());
    };
    let (status, results): (u32, HashMap<String, OwnedValue>) =
        message.body().deserialize().map_err(|_| ())?;
    (status == 0).then_some(results).ok_or(())
}

fn request_path(connection: &zbus::Connection, token: &str) -> Option<OwnedObjectPath> {
    let unique_name = connection.unique_name()?.as_str();
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    OwnedObjectPath::try_from(format!("{PORTAL_PATH}/request/{sender}/{token}")).ok()
}

async fn close_session(connection: &zbus::Connection, session: &OwnedObjectPath) {
    let Ok(proxy) = zbus::Proxy::new(
        connection,
        PORTAL_DESTINATION,
        session.clone(),
        SESSION_INTERFACE,
    )
    .await
    else {
        return;
    };
    let _ = proxy.call::<_, _, ()>("Close", &()).await;
}

fn requested_shortcuts(
    region_hotkey: HotkeyBinding,
    full_display_hotkey: HotkeyBinding,
) -> Vec<(String, HashMap<&'static str, Value<'static>>)> {
    [
        (
            REGION_SHORTCUT_ID,
            "Capture a region and create a pin",
            portal_trigger(region_hotkey),
        ),
        (
            FULL_DISPLAY_SHORTCUT_ID,
            "Capture the full display",
            portal_trigger(full_display_hotkey),
        ),
    ]
    .into_iter()
    .map(|(id, description, trigger)| {
        let mut details = HashMap::new();
        details.insert("description", Value::from(description.to_string()));
        details.insert("preferred_trigger", Value::from(trigger));
        (id.to_string(), details)
    })
    .collect()
}

fn action_from_signal(message: &zbus::Message, session: &PortalSession) -> Option<ActionId> {
    let header = message.header();
    if header.member()?.as_str() != "Activated" {
        return None;
    }
    let (signal_session, shortcut_id, _timestamp, _options): (
        OwnedObjectPath,
        String,
        u64,
        HashMap<String, OwnedValue>,
    ) = message.body().deserialize().ok()?;
    if signal_session != session.handle || !session.accepted_shortcuts.contains(&shortcut_id) {
        return None;
    }
    action_for_shortcut_id(&shortcut_id)
}

fn action_for_shortcut_id(shortcut_id: &str) -> Option<ActionId> {
    match shortcut_id {
        REGION_SHORTCUT_ID => Some(ActionId::CaptureRegionAndPin),
        FULL_DISPLAY_SHORTCUT_ID => Some(ActionId::CaptureFullDisplay),
        _ => None,
    }
}

fn accepted_shortcut_ids(value: &OwnedValue) -> Option<HashSet<String>> {
    let entries: Vec<(String, HashMap<String, OwnedValue>)> =
        value.try_clone().ok()?.try_into().ok()?;
    let accepted = entries
        .iter()
        .map(|(shortcut_id, _)| shortcut_id)
        .filter(|shortcut_id| action_for_shortcut_id(shortcut_id).is_some())
        .cloned()
        .collect();
    Some(accepted)
}

fn object_path_from_value(value: &OwnedValue) -> Option<OwnedObjectPath> {
    if let Ok(path) = OwnedObjectPath::try_from(value.try_clone().ok()?) {
        return Some(path);
    }
    let path: String = value.try_clone().ok()?.try_into().ok()?;
    OwnedObjectPath::try_from(path).ok()
}

fn next_tokens() -> (String, String) {
    (next_token("request"), next_token("session"))
}

fn next_token(prefix: &str) -> String {
    let sequence = TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("pinora_{prefix}_{}_{}", std::process::id(), sequence)
}

fn portal_trigger(binding: HotkeyBinding) -> String {
    let mut parts = Vec::with_capacity(5);
    if binding.modifiers.contains(HotkeyModifiers::CONTROL) {
        parts.push("CTRL".to_string());
    }
    if binding.modifiers.contains(HotkeyModifiers::ALT) {
        parts.push("ALT".to_string());
    }
    if binding.modifiers.contains(HotkeyModifiers::SHIFT) {
        parts.push("SHIFT".to_string());
    }
    if binding.modifiers.contains(HotkeyModifiers::SUPER) {
        parts.push("LOGO".to_string());
    }
    parts.push(portal_key_name(binding.code).to_string());
    parts.join("+")
}

const fn portal_key_name(code: HotkeyCode) -> &'static str {
    match code {
        HotkeyCode::F1 => "F1",
        HotkeyCode::F2 => "F2",
        HotkeyCode::F3 => "F3",
        HotkeyCode::F4 => "F4",
        HotkeyCode::F5 => "F5",
        HotkeyCode::F6 => "F6",
        HotkeyCode::F7 => "F7",
        HotkeyCode::F8 => "F8",
        HotkeyCode::F9 => "F9",
        HotkeyCode::F10 => "F10",
        HotkeyCode::F11 => "F11",
        HotkeyCode::F12 => "F12",
        HotkeyCode::KeyA => "a",
        HotkeyCode::KeyB => "b",
        HotkeyCode::KeyC => "c",
        HotkeyCode::KeyD => "d",
        HotkeyCode::KeyE => "e",
        HotkeyCode::KeyF => "f",
        HotkeyCode::KeyG => "g",
        HotkeyCode::KeyH => "h",
        HotkeyCode::KeyI => "i",
        HotkeyCode::KeyJ => "j",
        HotkeyCode::KeyK => "k",
        HotkeyCode::KeyL => "l",
        HotkeyCode::KeyM => "m",
        HotkeyCode::KeyN => "n",
        HotkeyCode::KeyO => "o",
        HotkeyCode::KeyP => "p",
        HotkeyCode::KeyQ => "q",
        HotkeyCode::KeyR => "r",
        HotkeyCode::KeyS => "s",
        HotkeyCode::KeyT => "t",
        HotkeyCode::KeyU => "u",
        HotkeyCode::KeyV => "v",
        HotkeyCode::KeyW => "w",
        HotkeyCode::KeyX => "x",
        HotkeyCode::KeyY => "y",
        HotkeyCode::KeyZ => "z",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_trigger_uses_the_freedesktop_shortcut_syntax() {
        assert_eq!(
            portal_trigger(HotkeyBinding::new(
                HotkeyModifiers::CONTROL | HotkeyModifiers::SHIFT,
                HotkeyCode::KeyS,
            )),
            "CTRL+SHIFT+s"
        );
        assert_eq!(
            portal_trigger(HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F2)),
            "F2"
        );
    }

    #[test]
    fn only_fixed_shortcut_ids_become_capture_actions() {
        assert_eq!(
            action_for_shortcut_id(REGION_SHORTCUT_ID),
            Some(ActionId::CaptureRegionAndPin)
        );
        assert_eq!(
            action_for_shortcut_id(FULL_DISPLAY_SHORTCUT_ID),
            Some(ActionId::CaptureFullDisplay)
        );
        assert_eq!(action_for_shortcut_id("quit"), None);
    }

    #[test]
    fn requested_shortcuts_have_fixed_descriptions_and_preferred_triggers() {
        let entries = requested_shortcuts(
            HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F2),
            HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4),
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, REGION_SHORTCUT_ID);
        assert_eq!(entries[1].0, FULL_DISPLAY_SHORTCUT_ID);
        assert_eq!(
            entries[1].1["preferred_trigger"].downcast_ref::<&str>(),
            Ok("ALT+F4")
        );
    }

    #[test]
    fn portal_failures_have_stable_non_sensitive_codes() {
        assert_eq!(
            PortalFailure::BindingRejected.code(),
            "portal_binding_rejected"
        );
        assert!(!PortalFailure::Disconnected.code().contains('/'));
    }

    #[test]
    fn portal_requires_the_v2_global_shortcuts_contract() {
        assert!(!portal_version_supported(0));
        assert!(!portal_version_supported(1));
        assert!(portal_version_supported(2));
        assert!(portal_version_supported(99));
    }
}
