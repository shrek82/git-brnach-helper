//! 加载状态和排序相关定义

use std::time::Instant;

/// 加载状态（替代多个布尔字段）
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingState {
    /// 空闲状态
    Idle,
    /// 加载中
    Loading { progress: u8, message: String },
    /// 加载完成
    Loaded { last_updated: Instant },
    /// 加载错误
    Error { message: String },
}

impl LoadingState {
    /// 是否正在加载
    pub fn is_loading(&self) -> bool {
        matches!(self, LoadingState::Loading { .. })
    }

    /// 是否有错误
    pub fn is_error(&self) -> bool {
        matches!(self, LoadingState::Error { .. })
    }
}

impl Default for LoadingState {
    fn default() -> Self {
        Self::Idle
    }
}

/// 排序字段
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SortField {
    #[default]
    Name,
    LastCommitTime,
    Author,
}
