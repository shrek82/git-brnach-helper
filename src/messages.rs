//! 消息定义模块
//!
//! 所有状态变更通过 Message 枚举进行处理，确保数据流清晰可追踪

use crossterm::event::KeyCode;
use crate::domain::RemoteBranch;

/// 所有消息的枚举（状态变更的唯一入口）
#[derive(Debug, Clone)]
pub enum Message {
    // === 用户输入 ===
    /// 按键按下
    KeyPressed(KeyCode),
    /// 过滤文本变化
    FilterChanged(String),
    /// 切换分支选中状态（过滤后的索引）
    BranchToggled(usize),
    /// 切换全选状态
    SelectAllToggled,

    // === 异步任务完成 ===
    /// 分支列表加载完成
    BranchesLoaded(Result<Vec<RemoteBranch>, String>),
    /// 提交信息加载完成
    CommitInfoLoaded {
        branch_name: String,
        info: CommitInfo,
    },
    /// 分支创建完成
    BranchCreated {
        branch_name: String,
        success: bool,
        message: String,
    },
    /// 分支同步完成
    BranchSynced {
        branch_name: String,
        success: bool,
        message: String,
    },
    /// 分支删除完成
    BranchDeleted {
        branch_name: String,
        success: bool,
        message: String,
    },
    /// 分支切换完成
    BranchCheckedOut {
        branch_name: String,
        success: bool,
        message: String,
    },

    // === 内部事件 ===
    /// 每帧调用的 tick 事件（用于动画、超时等）
    Tick,
    /// 退出应用
    Quit,
}

/// 提交信息
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub time: String,
    pub author: String,
    pub message: String,
}

impl From<(String, String, String)> for CommitInfo {
    fn from((time, author, message): (String, String, String)) -> Self {
        Self { time, author, message }
    }
}
