use crate::git;
use anyhow::Result;
use chrono::Local;

/// 表示一个远程分支及其对应的本地分支状态
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
}

/// 应用主状态
pub struct App {
    /// 远程分支列表
    pub remote_branches: Vec<RemoteBranch>,
    /// 所有分支的完整列表（用于过滤）
    pub all_branches: Vec<RemoteBranch>,
    /// 当前选中的分支索引
    pub cursor: usize,
    /// 状态消息
    pub status_message: String,
    /// 远程仓库名称（默认 "origin"）
    pub remote_name: String,
    /// 操作历史记录（最多保留 10 条）
    pub operation_log: Vec<String>,
    /// 搜索过滤文本
    pub filter_text: String,
    /// 是否处于搜索模式
    pub is_filtering: bool,
    /// 是否显示帮助 overlay
    pub show_help_overlay: bool,
    /// 是否显示删除确认对话框
    pub show_delete_confirm: bool,
    /// 等待删除的分支数量
    pub pending_delete_count: usize,
    /// 是否强制删除
    pub pending_force_delete: bool,
    /// 受保护的分支名称列表
    pub protected_branches: Vec<String>,
    /// 是否正在执行操作
    pub is_operating: bool,
    /// 当前操作进度（当前完成数）
    pub progress_current: usize,
    /// 当前操作进度（总数）
    pub progress_total: usize,
    /// 是否显示分支详情弹窗
    pub show_branch_detail: bool,
    /// 当前详情弹窗中显示的分支信息
    pub detail_branch_name: String,
    /// 最近提交记录
    pub recent_commits: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        App {
            remote_branches: Vec::new(),
            all_branches: Vec::new(),
            cursor: 0,
            status_message: String::from("就绪 - 按 'r' 刷新，'q' 退出"),
            remote_name: String::from("origin"),
            operation_log: Vec::new(),
            filter_text: String::new(),
            is_filtering: false,
            show_help_overlay: false,
            show_delete_confirm: false,
            pending_delete_count: 0,
            pending_force_delete: false,
            protected_branches: vec![
                String::from("main"),
                String::from("master"),
                String::from("develop"),
                String::from("dev"),
            ],
            is_operating: false,
            progress_current: 0,
            progress_total: 0,
            show_branch_detail: false,
            detail_branch_name: String::new(),
            recent_commits: Vec::new(),
        }
    }

    /// 检查分支是否是受保护的
    pub fn is_protected_branch(&self, branch_name: &str) -> bool {
        self.protected_branches.iter().any(|p| p == branch_name)
    }

    /// 应用过滤条件到分支列表
    fn apply_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.remote_branches = self.all_branches.clone();
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.remote_branches = self.all_branches
                .iter()
                .filter(|b| b.short_name.to_lowercase().contains(&filter_lower) || b.remote_ref.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();
        }
        // 重置 cursor 到有效范围
        if self.cursor >= self.remote_branches.len() {
            self.cursor = self.remote_branches.len().saturating_sub(1);
        }
    }

    /// 设置过滤文本并更新显示列表
    pub fn set_filter(&mut self, text: &str) {
        self.filter_text = text.to_string();
        self.is_filtering = !text.is_empty();
        self.apply_filter();
        if text.is_empty() {
            self.status_message = String::from("已取消过滤");
        } else {
            self.status_message = format!("过滤中：\"{}\" - 显示 {} 个分支", text, self.remote_branches.len());
        }
    }

    /// 切换帮助 overlay 显示状态
    pub fn toggle_help_overlay(&mut self) {
        self.show_help_overlay = !self.show_help_overlay;
    }

    /// 请求删除选中的分支（显示确认对话框）
    pub fn request_delete(&mut self, force: bool) {
        // 检查是否有受保护的分支被选中
        let protected_selected: Vec<String> = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local && self.is_protected_branch(&b.short_name))
            .map(|b| b.short_name.clone())
            .collect();

        if !protected_selected.is_empty() {
            self.status_message = format!(
                "无法删除受保护分支：{}",
                protected_selected.join(", ")
            );
            return;
        }

        let to_delete_count = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local)
            .count();

        if to_delete_count == 0 {
            self.status_message = String::from("没有选中的本地分支可删除");
            return;
        }

        self.pending_delete_count = to_delete_count;
        self.pending_force_delete = force;
        self.show_delete_confirm = true;
        self.status_message = format!("确认删除 {} 个分支？按 'y' 确认，'n' 取消", to_delete_count);
    }

    /// 确认删除操作
    pub fn confirm_delete(&mut self, confirm: bool, _force: bool) {
        self.show_delete_confirm = false;

        if confirm {
            let force = self.pending_force_delete;
            let _ = self.delete_selected_branches(force);
        } else {
            self.status_message = String::from("已取消删除操作");
        }
    }

    /// 显示分支详情弹窗
    pub fn show_branch_detail_popup(&mut self) {
        if self.remote_branches.is_empty() {
            self.status_message = String::from("没有可选的分支");
            return;
        }

        let branch = &self.remote_branches[self.cursor];
        self.detail_branch_name = branch.short_name.clone();

        // 获取最近提交记录
        if branch.has_local {
            match git::get_recent_commits(&branch.short_name) {
                Ok(commits) => {
                    self.recent_commits = commits;
                }
                Err(e) => {
                    self.recent_commits = vec![format!("获取失败：{}", e)];
                }
            }
        } else {
            self.recent_commits = vec!["[本地分支不存在，无法查看提交记录]".to_string()];
        }

        self.show_branch_detail = true;
    }

    /// 关闭分支详情弹窗
    pub fn close_branch_detail(&mut self) {
        self.show_branch_detail = false;
        self.recent_commits.clear();
    }

    /// 同步选中的分支（按 s 键）
    pub fn sync_selected_branches(&mut self) -> Result<()> {
        // 收集需要同步的分支（只同步已存在的本地分支）
        let to_sync: Vec<String> = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local)
            .map(|b| b.short_name.clone())
            .collect();

        // 如果没有选中的分支，则同步当前光标所在的分支（如果存在本地）
        let to_sync = if to_sync.is_empty() && !self.remote_branches.is_empty() {
            let branch = &self.remote_branches[self.cursor];
            if branch.has_local {
                vec![branch.short_name.clone()]
            } else {
                self.status_message = format!("分支 '{}' 尚未创建到本地", branch.short_name);
                return Ok(());
            }
        } else {
            to_sync
        };

        if to_sync.is_empty() {
            self.status_message = String::from("没有可同步的分支");
            return Ok(());
        }

        self.is_operating = true;
        self.progress_total = to_sync.len();
        self.progress_current = 0;
        self.status_message = format!("正在同步 {} 个分支... [0/{}]", to_sync.len(), to_sync.len());

        let mut success_count = 0;
        let mut failed_branches = Vec::new();

        for short_name in to_sync {
            match git::sync_local_branch(&short_name) {
                Ok(_) => {
                    success_count += 1;
                    self.progress_current += 1;
                    self.add_log(&format!("同步分支：{}", short_name));
                }
                Err(e) => {
                    failed_branches.push(format!("{}: {}", short_name, e));
                    self.progress_current += 1;
                }
            }
        }

        self.is_operating = false;

        self.status_message = if failed_branches.is_empty() {
            let msg = format!("成功同步 {} 个分支", success_count);
            self.add_log(&msg);
            msg
        } else {
            let msg = format!(
                "成功 {} 个，失败 {} 个：{}",
                success_count,
                failed_branches.len(),
                failed_branches.join(", ")
            );
            self.add_log(&msg);
            msg
        };

        Ok(())
    }

    /// 添加操作日志
    fn add_log(&mut self, message: &str) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.operation_log.insert(0, format!("[{}] {}", timestamp, message));
        // 保留最多 10 条记录
        if self.operation_log.len() > 10 {
            self.operation_log.truncate(10);
        }
    }

    /// 刷新分支列表
    pub fn refresh_branches(&mut self) -> Result<()> {
        self.status_message = String::from("正在刷新分支列表...");

        // 获取所有远程引用
        let remote_refs = git::list_remote_branches(&self.remote_name)?;

        // 获取所有本地分支名称
        let local_branches = git::list_local_branches()?;

        // 构建 RemoteBranch 列表
        self.all_branches.clear();

        for remote_ref in remote_refs {
            // 从 "origin/feature/login" 提取 "feature/login"
            let short_name = remote_ref
                .strip_prefix(&format!("{}/", self.remote_name))
                .unwrap_or(&remote_ref)
                .to_string();

            // 检查是否存在对应的本地分支
            let has_local = local_branches.iter().any(|l| l == &short_name);
            let local_name = if has_local {
                Some(short_name.clone())
            } else {
                None
            };

            // 计算 ahead/behind（仅当本地分支存在时）
            let (ahead, behind) = if has_local {
                git::get_branch_ahead_behind(&short_name).unwrap_or((0, 0))
            } else {
                (0, 0)
            };

            self.all_branches.push(RemoteBranch {
                remote_ref,
                short_name,
                has_local,
                local_name,
                selected: false,
                ahead,
                behind,
            });
        }

        // 按名称排序
        self.all_branches.sort_by(|a, b| {
            a.short_name.to_lowercase().cmp(&b.short_name.to_lowercase())
        });

        // 应用过滤
        self.apply_filter();

        let count = self.remote_branches.len();
        self.status_message = format!("共 {} 个远程分支", count);
        self.add_log(&format!("刷新分支列表，共 {} 个远程分支", count));

        Ok(())
    }

    /// 选中下一个分支
    pub fn select_next(&mut self) {
        if self.remote_branches.is_empty() {
            return;
        }
        if self.cursor < self.remote_branches.len() - 1 {
            self.cursor += 1;
        }
    }

    /// 选中上一个分支
    pub fn select_previous(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// 切换当前分支的选中状态
    pub fn toggle_selection(&mut self) {
        if !self.remote_branches.is_empty() {
            let branch = &mut self.remote_branches[self.cursor];
            branch.selected = !branch.selected;

            let status = if branch.selected { "已选中" } else { "已取消" };
            let short_name = branch.short_name.clone();
            self.status_message = format!("{}: {}", short_name, status);
            self.add_log(&format!("{}分支：{}", short_name, status));
        }
    }

    /// 切换全选状态
    pub fn toggle_select_all(&mut self) {
        if self.remote_branches.is_empty() {
            return;
        }

        // 检查是否全部已选中
        let all_selected = self.remote_branches.iter().all(|b| b.selected);

        for branch in &mut self.remote_branches {
            branch.selected = !all_selected;
        }

        self.status_message = if all_selected {
            String::from("已取消全选")
        } else {
            String::from("已全选")
        };
        let log_msg = self.status_message.clone();
        self.add_log(&log_msg);
    }

    /// 执行选中的分支操作（创建或同步）
    pub fn execute_selected_branches(&mut self) -> Result<()> {
        // 收集需要创建的分支
        let to_create: Vec<(String, String)> = self.remote_branches
            .iter()
            .filter(|b| b.selected && !b.has_local)
            .map(|b| (b.remote_ref.clone(), b.short_name.clone()))
            .collect();

        // 收集需要同步的分支
        let to_sync: Vec<String> = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local)
            .map(|b| b.short_name.clone())
            .collect();

        if to_create.is_empty() && to_sync.is_empty() {
            self.status_message = String::from("没有选中的分支需要操作");
            return Ok(());
        }

        let total_ops = to_create.len() + to_sync.len();
        self.is_operating = true;
        self.progress_total = total_ops;
        self.progress_current = 0;
        self.status_message = format!("正在执行 {} 个操作... [0/{}]", total_ops, total_ops);

        let mut success_count = 0;
        let mut failed_branches = Vec::new();

        // 执行创建操作
        for (remote_ref, short_name) in to_create {
            match git::create_local_branch(&remote_ref, &short_name) {
                Ok(_) => {
                    success_count += 1;
                    self.progress_current += 1;
                    // 更新状态
                    if let Some(b) = self.remote_branches.iter_mut().find(|b| b.short_name == short_name) {
                        b.has_local = true;
                        b.local_name = Some(short_name.clone());
                        b.selected = false;
                    }
                    self.add_log(&format!("创建分支：{}", short_name));
                }
                Err(e) => {
                    failed_branches.push(format!("{}: {}", short_name, e));
                    self.progress_current += 1;
                }
            }
        }

        // 执行同步操作
        for short_name in to_sync {
            match git::sync_local_branch(&short_name) {
                Ok(_) => {
                    success_count += 1;
                    self.progress_current += 1;
                    // 取消选中
                    if let Some(b) = self.remote_branches.iter_mut().find(|b| b.short_name == short_name) {
                        b.selected = false;
                    }
                    self.add_log(&format!("同步分支：{}", short_name));
                }
                Err(e) => {
                    failed_branches.push(format!("{}: {}", short_name, e));
                    self.progress_current += 1;
                }
            }
        }

        self.is_operating = false;

        self.status_message = if failed_branches.is_empty() {
            let msg = format!("成功完成 {} 个操作", success_count);
            self.add_log(&msg);
            msg
        } else {
            let msg = format!(
                "成功 {} 个，失败 {} 个：{}",
                success_count,
                failed_branches.len(),
                failed_branches.join(", ")
            );
            self.add_log(&msg);
            msg
        };

        Ok(())
    }

    /// 切换到当前选中的分支
    pub fn checkout_current_selection(&mut self) -> Result<()> {
        if self.remote_branches.is_empty() {
            self.status_message = String::from("没有可选的分支");
            return Ok(());
        }

        let branch = &self.remote_branches[self.cursor];

        if !branch.has_local {
            self.status_message = format!("分支 '{}' 尚未创建到本地", branch.short_name);
            return Ok(());
        }

        match git::checkout_branch(&branch.short_name) {
            Ok(_) => {
                let msg = format!("已切换到分支：{}", branch.short_name);
                self.status_message = msg.clone();
                self.add_log(&msg);
            }
            Err(e) => {
                let msg = format!("切换失败：{}", e);
                self.status_message = msg.clone();
                self.add_log(&msg);
            }
        }

        Ok(())
    }

    /// 删除选中的本地分支
    pub fn delete_selected_branches(&mut self, force: bool) -> Result<()> {
        // 再次检查受保护分支
        let protected_selected: Vec<String> = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local && self.is_protected_branch(&b.short_name))
            .map(|b| b.short_name.clone())
            .collect();

        if !protected_selected.is_empty() {
            self.status_message = format!(
                "拒绝删除受保护分支：{}",
                protected_selected.join(", ")
            );
            self.add_log(&format!("拒绝删除受保护分支：{}", protected_selected.join(", ")));
            return Ok(());
        }

        // 收集需要删除的分支
        let to_delete: Vec<String> = self.remote_branches
            .iter()
            .filter(|b| b.selected && b.has_local)
            .map(|b| b.short_name.clone())
            .collect();

        if to_delete.is_empty() {
            self.status_message = String::from("没有选中的本地分支可删除");
            return Ok(());
        }

        self.is_operating = true;
        self.progress_total = to_delete.len();
        self.progress_current = 0;
        self.status_message = format!("正在删除 {} 个分支... [0/{}]", to_delete.len(), to_delete.len());

        let mut success_count = 0;
        let mut failed_branches = Vec::new();

        for short_name in to_delete {
            match git::delete_local_branch(&short_name, force) {
                Ok(_) => {
                    success_count += 1;
                    self.progress_current += 1;
                    // 更新状态
                    if let Some(b) = self.remote_branches.iter_mut().find(|b| b.short_name == short_name) {
                        b.has_local = false;
                        b.local_name = None;
                        b.selected = false;
                    }
                    self.add_log(&format!("删除分支：{}", short_name));
                }
                Err(e) => {
                    failed_branches.push(format!("{}: {}", short_name, e));
                    self.progress_current += 1;
                }
            }
        }

        self.is_operating = false;

        self.status_message = if failed_branches.is_empty() {
            let msg = format!("成功删除 {} 个分支", success_count);
            self.add_log(&msg);
            msg
        } else {
            let msg = format!(
                "成功 {} 个，失败 {} 个：{}",
                success_count,
                failed_branches.len(),
                failed_branches.join(", ")
            );
            self.add_log(&msg);
            msg
        };

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
