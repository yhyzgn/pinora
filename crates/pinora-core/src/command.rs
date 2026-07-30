use crate::ids::CorrelationId;

/// 用户或系统意图。命令可以失败；成功后应产生对应领域事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 启动应用运行时（首实例）。
    Bootstrap {
        correlation_id: CorrelationId,
    },
    /// 激活已有实例（二次启动转发）。
    Activate {
        correlation_id: CorrelationId,
    },
    /// 请求优雅退出。
    Shutdown {
        correlation_id: CorrelationId,
    },
}

impl Command {
    pub fn correlation_id(&self) -> CorrelationId {
        match self {
            Self::Bootstrap { correlation_id }
            | Self::Activate { correlation_id }
            | Self::Shutdown { correlation_id } => *correlation_id,
        }
    }

    pub fn bootstrap() -> Self {
        Self::Bootstrap {
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn activate() -> Self {
        Self::Activate {
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn shutdown() -> Self {
        Self::Shutdown {
            correlation_id: CorrelationId::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_carry_correlation_ids() {
        let cmd = Command::bootstrap();
        assert!(matches!(cmd, Command::Bootstrap { .. }));
        assert!(cmd.correlation_id().raw() > 0);
    }
}
