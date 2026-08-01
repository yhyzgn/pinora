use std::fmt;

/// 稳定错误码，供日志与诊断使用；不得包含像素或敏感路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    AlreadyRunning,
    NotRunning,
    SingleInstanceBusy,
    InvalidState,
    NotFound,
    CapabilityUnavailable,
    PermissionDenied,
    RetryablePlatform,
    CommandRejected,
    Cancelled,
    TimedOut,
    ResourceLimitExceeded,
    ClipboardFailed,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyRunning => "already_running",
            Self::NotRunning => "not_running",
            Self::SingleInstanceBusy => "single_instance_busy",
            Self::InvalidState => "invalid_state",
            Self::NotFound => "not_found",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::PermissionDenied => "permission_denied",
            Self::RetryablePlatform => "retryable_platform",
            Self::CommandRejected => "command_rejected",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::ResourceLimitExceeded => "resource_limit_exceeded",
            Self::ClipboardFailed => "clipboard_failed",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 领域/应用层可传播错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinoraError {
    pub code: ErrorCode,
    pub message: String,
}

impl PinoraError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for PinoraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for PinoraError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_stable_strings() {
        assert_eq!(ErrorCode::AlreadyRunning.as_str(), "already_running");
        assert_eq!(ErrorCode::PermissionDenied.as_str(), "permission_denied");
        assert_eq!(ErrorCode::Cancelled.as_str(), "cancelled");
        assert_eq!(ErrorCode::TimedOut.as_str(), "timed_out");
        assert_eq!(
            ErrorCode::ResourceLimitExceeded.as_str(),
            "resource_limit_exceeded"
        );
        assert_eq!(ErrorCode::ClipboardFailed.as_str(), "clipboard_failed");
    }
}
