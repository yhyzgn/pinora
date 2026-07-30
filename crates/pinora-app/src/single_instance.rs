use std::sync::{Arc, Mutex};

use pinora_core::{Command, PinoraError};

/// 单实例获取结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceAcquisition {
    /// 当前进程成为唯一实例。
    Acquired,
    /// 已有实例；调用方应转发激活命令并退出。
    ExistingInstance,
}

/// 单实例错误（协议层）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleInstanceError {
    Poisoned,
    ForwardFailed(String),
}

impl From<SingleInstanceError> for PinoraError {
    fn from(value: SingleInstanceError) -> Self {
        match value {
            SingleInstanceError::Poisoned => {
                PinoraError::new(pinora_core::ErrorCode::Internal, "single-instance lock poisoned")
            }
            SingleInstanceError::ForwardFailed(msg) => {
                PinoraError::new(pinora_core::ErrorCode::SingleInstanceBusy, msg)
            }
        }
    }
}

/// 单实例锁与跨进程激活转发抽象。
pub trait SingleInstance {
    fn acquire(&self) -> Result<InstanceAcquisition, SingleInstanceError>;
    fn forward(&self, command: Command) -> Result<(), SingleInstanceError>;
    /// 取出主实例待处理的转发命令（如 Activate）。
    fn poll_forwarded(&self) -> Result<Vec<Command>, SingleInstanceError> {
        Ok(Vec::new())
    }
    fn release(&self) -> Result<(), SingleInstanceError>;
}

#[derive(Debug)]
struct InMemoryState {
    held: bool,
    forwarded: Vec<Command>,
}

/// 进程内单实例实现，仅用于测试。
#[derive(Debug, Clone)]
pub struct InMemorySingleInstance {
    inner: Arc<Mutex<InMemoryState>>,
}

impl InMemorySingleInstance {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InMemoryState {
                held: false,
                forwarded: Vec::new(),
            })),
        }
    }

    pub fn forwarded_commands(&self) -> Result<Vec<Command>, SingleInstanceError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        Ok(guard.forwarded.clone())
    }
}

impl Default for InMemorySingleInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleInstance for InMemorySingleInstance {
    fn acquire(&self) -> Result<InstanceAcquisition, SingleInstanceError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        if guard.held {
            Ok(InstanceAcquisition::ExistingInstance)
        } else {
            guard.held = true;
            Ok(InstanceAcquisition::Acquired)
        }
    }

    fn forward(&self, command: Command) -> Result<(), SingleInstanceError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        if !guard.held {
            return Err(SingleInstanceError::ForwardFailed(
                "no primary instance to forward to".into(),
            ));
        }
        guard.forwarded.push(command);
        Ok(())
    }

    fn poll_forwarded(&self) -> Result<Vec<Command>, SingleInstanceError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        Ok(std::mem::take(&mut guard.forwarded))
    }

    fn release(&self) -> Result<(), SingleInstanceError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| SingleInstanceError::Poisoned)?;
        guard.held = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_reports_existing_instance() {
        let lock = InMemorySingleInstance::new();
        assert_eq!(lock.acquire().unwrap(), InstanceAcquisition::Acquired);
        assert_eq!(
            lock.acquire().unwrap(),
            InstanceAcquisition::ExistingInstance
        );
    }

    #[test]
    fn forward_and_poll_activate_command() {
        let lock = InMemorySingleInstance::new();
        lock.acquire().unwrap();
        let cmd = Command::activate();
        lock.forward(cmd.clone()).unwrap();
        let polled = lock.poll_forwarded().unwrap();
        assert_eq!(polled, vec![cmd]);
        assert!(lock.poll_forwarded().unwrap().is_empty());
    }
}
