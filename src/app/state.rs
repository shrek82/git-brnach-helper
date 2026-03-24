//! 应用状态模块
//!
//! 定义应用主状态 AppState

use std::time::Instant;
use crate::domain::{BranchList, RemoteBranch};

/// 应用主状态（单一数据源）
pub struct AppState {
    /// 分支列表
    pub branches: BranchList,
    /// 当前选中的分支索引（在过滤后的列表中的索引）
    pub cursor: usize,
    /// 过滤文本
    pub filter_text: String,
    /// 当前所在的分支名称
    pub current_branch: String,
    /// 远程仓库名称
    pub remote_name: String,
    /// 模态框/弹窗状态
    pub modal: Option<ModalState>,
    /// 提示信息（Toast）
    pub toast: Option<Toast>,
    /// 操作日志
    pub operation_log: Vec<String>,
}

/// 弹窗状态
#[derive(Debug, Clone)]
pub enum ModalState {
    /// 删除确认
    DeleteConfirm {
        branches: Vec<String>,
        force: bool,
    },
    /// 分支详情
    BranchDetail {
        branch_name: String,
        commits: Vec<String>,
    },
    /// 帮助
    Help,
}

/// 提示信息（Toast）
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
}

/// Toast 级别
#[derive(Debug, Clone, Copy)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl Toast {
    /// 创建 Info 级别的 Toast
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Info,
            created_at: Instant::now(),
        }
    }

    /// 创建 Success 级别的 Toast
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Success,
            created_at: Instant::now(),
        }
    }

    /// 创建 Warning 级别的 Toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Warning,
            created_at: Instant::now(),
        }
    }

    /// 创建 Error 级别的 Toast
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Error,
            created_at: Instant::now(),
        }
    }

    /// 检查 Toast 是否已过期（超过 3 秒）
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(3)
    }
}

use std::time::Duration;

impl AppState {
    /// 创建新的应用状态
    pub fn new() -> Self {
        Self {
            branches: BranchList::new(),
            cursor: 0,
            filter_text: String::new(),
            current_branch: String::from("unknown"),
            remote_name: String::from("origin"),
            modal: None,
            toast: None,
            operation_log: Vec::new(),
        }
    }

    /// 添加操作日志
    pub fn add_log(&mut self, message: &str) {
        use chrono::Local;
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.operation_log.insert(0, format!("[{}] {}", timestamp, message));
        // 保留最多 10 条记录
        if self.operation_log.len() > 10 {
            self.operation_log.truncate(10);
        }
    }

    /// 获取过滤后的分支引用列表
    pub fn filtered_branches(&self) -> impl Iterator<Item = &RemoteBranch> {
        self.branches.filtered_iter(&self.filter_text)
    }

    /// 获取过滤后的分支索引到原始索引的映射
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter_text.is_empty() {
            return (0..self.branches.items.len()).collect();
        }

        let filter_lower = self.filter_text.to_lowercase();
        self.branches
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                if b.short_name.to_lowercase().contains(&filter_lower)
                    || b.remote_ref.to_lowercase().contains(&filter_lower)
                {
                    Some(i)
                }
                else {
                    None
                }
            })
            .collect()
    }

    /// 根据过滤后的索引获取原始索引
    pub fn filtered_index_to_original(&self, filtered_idx: usize) -> Option<usize> {
        self.filtered_indices().get(filtered_idx).copied()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
