//! 文件锁 + 本地 IPC 单实例与 Activate 转发。

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::Shutdown;
#[cfg(not(unix))]
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use fs2::FileExt;
use pinora_core::{ActionId, Command};

use crate::single_instance::{InstanceAcquisition, SingleInstance, SingleInstanceError};

const ACTIVATE_FRAME: &[u8] = b"ACTIVATE\n";
const CAPTURE_FRAME: &[u8] = b"CAPTURE\n";
const QUIT_FRAME: &[u8] = b"QUIT\n";

/// Unix 上基于 `instance.lock` + `activate.sock` 的 OS 单实例。
#[cfg(unix)]
pub struct OsSingleInstance {
    dir: PathBuf,
    lock_path: PathBuf,
    sock_path: PathBuf,
    lock_file: Mutex<Option<File>>,
    stop: Arc<AtomicBool>,
    activate_tx: Mutex<Option<Sender<Command>>>,
    activate_rx: Mutex<Option<Receiver<Command>>>,
    listener: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(unix)]
impl OsSingleInstance {
    /// 使用指定运行时目录（测试可传入临时目录）。
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self, SingleInstanceError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("create runtime dir: {e}")))?;
        Ok(Self {
            lock_path: dir.join("instance.lock"),
            sock_path: dir.join("activate.sock"),
            dir,
            lock_file: Mutex::new(None),
            stop: Arc::new(AtomicBool::new(false)),
            activate_tx: Mutex::new(None),
            activate_rx: Mutex::new(None),
            listener: Mutex::new(None),
        })
    }

    /// 默认目录：`$XDG_RUNTIME_DIR/pinora` 或 `/tmp/pinora-$USER`。
    pub fn default_paths() -> Result<Self, SingleInstanceError> {
        let dir = default_runtime_dir();
        Self::with_dir(dir)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn start_listener(&self) -> Result<(), SingleInstanceError> {
        let _ = fs::remove_file(&self.sock_path);
        let listener = UnixListener::bind(&self.sock_path).map_err(|e| {
            SingleInstanceError::ForwardFailed(format!("bind activate socket: {e}"))
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("set nonblocking: {e}")))?;

        let (tx, rx) = mpsc::channel();
        {
            let mut slot = self
                .activate_tx
                .lock()
                .map_err(|_| SingleInstanceError::Poisoned)?;
            *slot = Some(tx.clone());
        }
        {
            let mut slot = self
                .activate_rx
                .lock()
                .map_err(|_| SingleInstanceError::Poisoned)?;
            *slot = Some(rx);
        }

        let stop = Arc::clone(&self.stop);
        let sock_path = self.sock_path.clone();
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 64];
                        let mut collected = Vec::new();
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    collected.extend_from_slice(&buf[..n]);
                                    if let Some(cmd) = parse_ipc_frame(&collected) {
                                        let _ = tx.send(cmd);
                                        break;
                                    }
                                    if collected.len() > 256 {
                                        break;
                                    }
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    thread::sleep(Duration::from_millis(10));
                                    if stop.load(Ordering::SeqCst) {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
            let _ = fs::remove_file(sock_path);
        });

        let mut slot = self
            .listener
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        *slot = Some(handle);
        Ok(())
    }

    fn stop_listener(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // 触碰 socket 以尽快结束 accept 循环
        if let Ok(mut stream) = UnixStream::connect(&self.sock_path) {
            let _ = stream.write_all(b"STOP\n");
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Ok(mut guard) = self.listener.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
        if let Ok(mut guard) = self.activate_tx.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.activate_rx.lock() {
            *guard = None;
        }
        let _ = fs::remove_file(&self.sock_path);
    }
}

#[cfg(unix)]
impl SingleInstance for OsSingleInstance {
    fn acquire(&self) -> Result<InstanceAcquisition, SingleInstanceError> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("open lock: {e}")))?;

        match file.try_lock_exclusive() {
            Ok(()) => {
                self.stop.store(false, Ordering::SeqCst);
                self.start_listener()?;
                let mut guard = self
                    .lock_file
                    .lock()
                    .map_err(|_| SingleInstanceError::Poisoned)?;
                *guard = Some(file);
                Ok(InstanceAcquisition::Acquired)
            }
            Err(_) => Ok(InstanceAcquisition::ExistingInstance),
        }
    }

    fn forward(&self, command: Command) -> Result<(), SingleInstanceError> {
        let frame: &[u8] = match &command {
            Command::Activate { .. } => ACTIVATE_FRAME,
            Command::InvokeAction {
                action: ActionId::CaptureRegionAndPin,
                ..
            } => CAPTURE_FRAME,
            Command::Shutdown { .. } => QUIT_FRAME,
            _ => {
                return Err(SingleInstanceError::ForwardFailed(
                    "OS instance only forwards Activate / Capture / Quit".into(),
                ));
            }
        };
        let mut stream = UnixStream::connect(&self.sock_path).map_err(|e| {
            SingleInstanceError::ForwardFailed(format!("connect activate socket: {e}"))
        })?;
        stream
            .write_all(frame)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("write ipc frame: {e}")))?;
        let _ = stream.shutdown(Shutdown::Both);
        Ok(())
    }

    fn poll_forwarded(&self) -> Result<Vec<Command>, SingleInstanceError> {
        let guard = self
            .activate_rx
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        let Some(rx) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(cmd) => out.push(cmd),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        Ok(out)
    }

    fn release(&self) -> Result<(), SingleInstanceError> {
        self.stop_listener();
        let mut guard = self
            .lock_file
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        if let Some(file) = guard.take() {
            let _ = file.unlock();
            drop(file);
        }
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for OsSingleInstance {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[cfg(unix)]
fn default_runtime_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("pinora");
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    std::env::temp_dir().join(format!("pinora-{user}"))
}

fn parse_ipc_frame(buf: &[u8]) -> Option<Command> {
    if buf.windows(CAPTURE_FRAME.len()).any(|w| w == CAPTURE_FRAME) {
        return Some(Command::invoke_action(ActionId::CaptureRegionAndPin));
    }
    if buf.windows(QUIT_FRAME.len()).any(|w| w == QUIT_FRAME) {
        return Some(Command::shutdown());
    }
    if buf
        .windows(ACTIVATE_FRAME.len())
        .any(|w| w == ACTIVATE_FRAME)
    {
        return Some(Command::activate());
    }
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pinora-test-{nanos}"))
    }

    #[test]
    fn second_instance_forwards_activate() {
        let dir = temp_dir();
        let primary = OsSingleInstance::with_dir(&dir).unwrap();
        assert_eq!(primary.acquire().unwrap(), InstanceAcquisition::Acquired);

        let secondary = OsSingleInstance::with_dir(&dir).unwrap();
        assert_eq!(
            secondary.acquire().unwrap(),
            InstanceAcquisition::ExistingInstance
        );
        secondary.forward(Command::activate()).unwrap();

        // 等待 listener 处理
        let mut got = Vec::new();
        for _ in 0..50 {
            got = primary.poll_forwarded().unwrap();
            if !got.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(got.len(), 1);
        assert!(matches!(got[0], Command::Activate { .. }));

        primary.release().unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn second_instance_forwards_capture() {
        let dir = temp_dir();
        let primary = OsSingleInstance::with_dir(&dir).unwrap();
        assert_eq!(primary.acquire().unwrap(), InstanceAcquisition::Acquired);

        let secondary = OsSingleInstance::with_dir(&dir).unwrap();
        assert_eq!(
            secondary.acquire().unwrap(),
            InstanceAcquisition::ExistingInstance
        );
        secondary
            .forward(Command::invoke_action(ActionId::CaptureRegionAndPin))
            .unwrap();

        let mut got = Vec::new();
        for _ in 0..50 {
            got = primary.poll_forwarded().unwrap();
            if !got.is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(got.len(), 1);
        assert!(matches!(
            got[0],
            Command::InvokeAction {
                action: ActionId::CaptureRegionAndPin,
                ..
            }
        ));

        primary.release().unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parse_ipc_frames() {
        assert!(matches!(
            parse_ipc_frame(b"CAPTURE\n"),
            Some(Command::InvokeAction {
                action: ActionId::CaptureRegionAndPin,
                ..
            })
        ));
        assert!(matches!(
            parse_ipc_frame(b"QUIT\n"),
            Some(Command::Shutdown { .. })
        ));
        assert!(matches!(
            parse_ipc_frame(b"ACTIVATE\n"),
            Some(Command::Activate { .. })
        ));
    }
}

/// Windows/macOS 等非 Unix 平台的本地单实例实现。
///
/// `fs2` 负责跨平台文件锁；实例监听在 loopback TCP 随机端口，端口号写入
/// runtime 目录的 `activate.port`。该 IPC 只绑定 127.0.0.1，不暴露局域网。
#[cfg(not(unix))]
pub struct OsSingleInstance {
    dir: PathBuf,
    lock_path: PathBuf,
    port_path: PathBuf,
    lock_file: Mutex<Option<File>>,
    stop: Arc<AtomicBool>,
    activate_tx: Mutex<Option<Sender<Command>>>,
    activate_rx: Mutex<Option<Receiver<Command>>>,
    listener: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(not(unix))]
impl OsSingleInstance {
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self, SingleInstanceError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("create runtime dir: {e}")))?;
        Ok(Self {
            lock_path: dir.join("instance.lock"),
            port_path: dir.join("activate.port"),
            dir,
            lock_file: Mutex::new(None),
            stop: Arc::new(AtomicBool::new(false)),
            activate_tx: Mutex::new(None),
            activate_rx: Mutex::new(None),
            listener: Mutex::new(None),
        })
    }

    pub fn default_paths() -> Result<Self, SingleInstanceError> {
        Self::with_dir(default_runtime_dir())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn start_listener(&self) -> Result<(), SingleInstanceError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("bind IPC listener: {e}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("set nonblocking: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("read IPC port: {e}")))?
            .port();
        write_port_file(&self.port_path, port)?;

        let (tx, rx) = mpsc::channel();
        *self
            .activate_tx
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)? = Some(tx.clone());
        *self
            .activate_rx
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)? = Some(rx);

        let stop = Arc::clone(&self.stop);
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut collected = Vec::new();
                        let mut buf = [0u8; 64];
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    collected.extend_from_slice(&buf[..n]);
                                    if let Some(cmd) = parse_ipc_frame(&collected) {
                                        let _ = tx.send(cmd);
                                        break;
                                    }
                                    if collected.len() > 256 {
                                        break;
                                    }
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    thread::sleep(Duration::from_millis(10));
                                }
                                Err(_) => break,
                            }
                        }
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) if stop.load(Ordering::SeqCst) => break,
                    Err(_) => thread::sleep(Duration::from_millis(50)),
                }
            }
        });
        *self
            .listener
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)? = Some(handle);
        Ok(())
    }

    fn stop_listener(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(port) = read_port_file(&self.port_path)
            && let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port))
        {
            let _ = stream.write_all(b"STOP\n");
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Ok(mut guard) = self.listener.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
        if let Ok(mut guard) = self.activate_tx.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.activate_rx.lock() {
            *guard = None;
        }
        let _ = fs::remove_file(&self.port_path);
    }
}

#[cfg(not(unix))]
impl SingleInstance for OsSingleInstance {
    fn acquire(&self) -> Result<InstanceAcquisition, SingleInstanceError> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("open lock: {e}")))?;
        match file.try_lock_exclusive() {
            Ok(()) => {
                self.stop.store(false, Ordering::SeqCst);
                self.start_listener()?;
                *self
                    .lock_file
                    .lock()
                    .map_err(|_| SingleInstanceError::Poisoned)? = Some(file);
                Ok(InstanceAcquisition::Acquired)
            }
            Err(_) => Ok(InstanceAcquisition::ExistingInstance),
        }
    }

    fn forward(&self, command: Command) -> Result<(), SingleInstanceError> {
        let frame = command_frame(&command).ok_or_else(|| {
            SingleInstanceError::ForwardFailed(
                "OS instance only forwards Activate / Capture / Quit".into(),
            )
        })?;
        let port = read_port_file(&self.port_path)?;
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("connect IPC: {e}")))?;
        stream
            .write_all(frame)
            .map_err(|e| SingleInstanceError::ForwardFailed(format!("write IPC frame: {e}")))?;
        let _ = stream.shutdown(Shutdown::Both);
        Ok(())
    }

    fn poll_forwarded(&self) -> Result<Vec<Command>, SingleInstanceError> {
        let guard = self
            .activate_rx
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        let Some(rx) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(cmd) => out.push(cmd),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        Ok(out)
    }

    fn release(&self) -> Result<(), SingleInstanceError> {
        self.stop_listener();
        let mut guard = self
            .lock_file
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        if let Some(file) = guard.take() {
            let _ = file.unlock();
            drop(file);
        }
        Ok(())
    }
}

#[cfg(not(unix))]
impl Drop for OsSingleInstance {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[cfg(not(unix))]
fn default_runtime_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("Pinora/runtime");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/Pinora/runtime");
    }
    std::env::temp_dir().join("pinora-runtime")
}

#[cfg(not(unix))]
fn write_port_file(path: &Path, port: u16) -> Result<(), SingleInstanceError> {
    let temporary = path.with_extension("port.tmp");
    fs::write(&temporary, port.to_string())
        .map_err(|e| SingleInstanceError::ForwardFailed(format!("write IPC port: {e}")))?;
    fs::rename(&temporary, path)
        .map_err(|e| SingleInstanceError::ForwardFailed(format!("publish IPC port: {e}")))
}

#[cfg(not(unix))]
fn read_port_file(path: &Path) -> Result<u16, SingleInstanceError> {
    let text = fs::read_to_string(path)
        .map_err(|e| SingleInstanceError::ForwardFailed(format!("read IPC port: {e}")))?;
    text.trim()
        .parse()
        .map_err(|e| SingleInstanceError::ForwardFailed(format!("parse IPC port: {e}")))
}

#[cfg(not(unix))]
fn command_frame(command: &Command) -> Option<&'static [u8]> {
    match command {
        Command::Activate { .. } => Some(ACTIVATE_FRAME),
        Command::InvokeAction {
            action: ActionId::CaptureRegionAndPin,
            ..
        } => Some(CAPTURE_FRAME),
        Command::Shutdown { .. } => Some(QUIT_FRAME),
        _ => None,
    }
}

/// CLI 使用的本地 IPC 转发入口。
pub fn forward_ipc_frame(frame: &[u8]) -> bool {
    #[cfg(unix)]
    {
        let path = default_runtime_dir().join("activate.sock");
        let Ok(mut stream) = UnixStream::connect(path) else {
            return false;
        };
        stream.write_all(frame).is_ok()
    }

    #[cfg(not(unix))]
    {
        let path = default_runtime_dir().join("activate.port");
        let Ok(port) = read_port_file(&path) else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
            return false;
        };
        stream.write_all(frame).is_ok()
    }
}
