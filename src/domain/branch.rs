//! 领域模型模块
//!
//! 定义核心业务实体：分支、加载状态等

use std::collections::HashMap;
use crate::domain::loading::{LoadingState, SortField};

/// 远程分支
#[derive(Clone, Debug)]
pub struct RemoteBranch {
    /// 远程分支的完整引用名称，如 "origin/feature/login"
    pub remote_ref: String,
    /// 远程分支的短名称，如 "feature/login"
    pub short_name: String,
    /// 是否存在对应的本地分支
    pub has_local: bool,
    /// 对应的本地分支名称（如果存在）
    pub local_name: Option<String>,
    /// 是否被选中（用于批量创建）
    pub selected: bool,
    /// 领先远程的提交数（仅当 has_local=true 时有效）
    pub ahead: usize,
    /// 落后远程的提交数（仅当 has_local=true 时有效）
    pub behind: usize,
    /// 最后提交时间（相对时间，如 "2 天前"）
    pub last_commit_time: String,
    /// 最后提交作者
    pub last_commit_author: String,
    /// 最后提交消息
    pub last_commit_message: String,
}

impl RemoteBranch {
    /// 创建一个新的 RemoteBranch（用于测试）
    pub fn new(remote_ref: String, short_name: String) -> Self {
        Self {
            remote_ref,
            short_name,
            has_local: false,
            local_name: None,
            selected: false,
            ahead: 0,
            behind: 0,
            last_commit_time: String::from("-"),
            last_commit_author: String::from("-"),
            last_commit_message: String::from("-"),
        }
    }
}

/// 分支列表（单一数据源）
pub struct BranchList {
    /// 所有分支项
    pub items: Vec<RemoteBranch>,
    /// 加载状态
    pub loading_state: LoadingState,
    /// 排序字段
    pub sort_by: SortField,
    /// 用于快速查找的映射（short_name -> index）
    pub index_map: HashMap<String, usize>,
}

impl BranchList {
    /// 创建一个新的空分支列表
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            loading_state: LoadingState::Idle,
            sort_by: SortField::Name,
            index_map: HashMap::new(),
        }
    }

    /// 更新分支列表并重建索引
    pub fn set_items(&mut self, items: Vec<RemoteBranch>) {
        self.index_map.clear();
        for (i, item) in items.iter().enumerate() {
            self.index_map.insert(item.short_name.clone(), i);
        }
        self.items = items;
    }

    /// 根据短名称查找分支的索引
    pub fn index_of(&self, short_name: &str) -> Option<usize> {
        self.index_map.get(short_name).copied()
    }

    /// 更新单个分支的信息
    pub fn update_branch<F>(&mut self, short_name: &str, update_fn: F)
    where
        F: FnOnce(&mut RemoteBranch),
    {
        if let Some(idx) = self.index_of(short_name) {
            update_fn(&mut self.items[idx]);
        }
    }

    /// 获取过滤后的分支迭代器
    pub fn filtered_iter(&self, filter_text: &str) -> Box<dyn Iterator<Item = &RemoteBranch> + '_> {
        if filter_text.is_empty() {
            Box::new(self.items.iter())
        } else {
            let filter_text = filter_text.to_lowercase();
            Box::new(self.items.iter().filter(move |b| {
                b.short_name.to_lowercase().contains(&filter_text)
                    || b.remote_ref.to_lowercase().contains(&filter_text)
            }))
        }
    }
}

impl Default for BranchList {
    fn default() -> Self {
        Self::new()
    }
}
