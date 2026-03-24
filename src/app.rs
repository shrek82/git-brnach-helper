use crate::git;
use anyhow::Result;
use chrono::Local;
use std::sync::mpsc;

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
    /// 最后提交时间（相对时间，如 "2 天前"）
    pub last_commit_time: String,
    /// 最后提交作者
    pub last_commit_author: String,
    /// 最后提交消息
    pub last_commit_message: String,
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
    // === 懒加载状态 ===
    /// 是否正在加载分支列表
    pub is_loading: bool,
    /// 加载状态消息
    pub loading_message: String,
    /// 后台加载任务的接收器
    pub load_receiver: Option<mpsc::Receiver<Result<Vec<RemoteBranch>>>>,
    /// 后台 ahead/behind 和提交信息更新任务的接收器
    pub load_ahead_behind_receiver: Option<mpsc::Receiver<Vec<RemoteBranch>>>,
    /// 是否已从远程加载（false=使用本地缓存，true=已 fetch 远程）
    pub fetched_from_remote: bool,
    /// 当前所在的分支名称
    pub current_branch: String,
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
            is_loading: false,
            loading_message: String::new(),
            load_receiver: None,
            load_ahead_behind_receiver: None,
            fetched_from_remote: false,
            current_branch: String::from("unknown"),
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
        self.loading_message = format!("正在同步 {} 个分支...", to_sync.len());

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
        self.loading_message.clear();

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

    /// 刷新分支列表（保留用于兼容）
    #[allow(dead_code)]
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

            // 获取远程分支的最后提交信息
            let (commit_time, commit_author, commit_message) =
                git::get_remote_last_commit_info(&remote_ref)
                    .unwrap_or((String::from("-"), String::from("-"), String::from("-")));

            self.all_branches.push(RemoteBranch {
                remote_ref,
                short_name,
                has_local,
                local_name,
                selected: false,
                ahead,
                behind,
                last_commit_time: commit_time,
                last_commit_author: commit_author,
                last_commit_message: commit_message,
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

        let branch_name = self.remote_branches[self.cursor].short_name.clone();
        let has_local = self.remote_branches[self.cursor].has_local;

        if !has_local {
            self.status_message = format!("分支 '{}' 尚未创建到本地", branch_name);
            return Ok(());
        }

        // 检查是否有未提交的修改
        if git::has_uncommitted_changes()? {
            self.status_message = String::from("当前工作树有未提交的修改，无法切换分支");
            self.add_log("切换分支失败：存在未提交的修改");
            return Ok(());
        }

        // 如果是当前所在的分支，不需要切换
        if branch_name == self.current_branch {
            self.status_message = format!("已经在分支 '{}' 上", branch_name);
            return Ok(());
        }

        match git::checkout_branch(&branch_name) {
            Ok(_) => {
                let msg = format!("已切换到分支：{}", branch_name);
                self.status_message = msg.clone();
                self.add_log(&msg);
                // 更新当前分支名称
                self.current_branch = branch_name;
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

// === 懒加载相关方法 ===

impl App {
    /// 开始异步加载分支列表
    /// `fetch_remote` 控制是否先 fetch 远程（true=先请求远程，false=使用本地缓存）
    pub fn start_loading_branches(&mut self, fetch_remote: bool) {
        self.is_loading = true;
        self.loading_message = String::from("正在加载分支列表...");
        self.remote_branches.clear();
        self.all_branches.clear();

        let remote_name = self.remote_name.clone();
        let (tx, rx) = mpsc::channel();
        self.load_receiver = Some(rx);

        // 如果需要 fetch，在后台异步执行，不阻塞 UI
        if fetch_remote {
            let fetch_remote_name = remote_name.clone();
            std::thread::spawn(move || {
                // 后台 fetch，不影响 UI 显示
                let _ = std::process::Command::new("git")
                    .args(["fetch", &fetch_remote_name, "--quiet"])
                    .output();
                // fetch 完成后不通知 UI，用户下次刷新时自然能看到更新
            });
        }

        std::thread::spawn(move || {
            // 不管是否 fetch，先快速读取本地缓存的分支列表
            let remote_refs = match git::list_remote_branches(&remote_name) {
                Ok(refs) => refs,
                Err(e) => return tx.send(Err(anyhow::anyhow!("获取远程分支失败：{}", e))),
            };

            let local_branches = match git::list_local_branches() {
                Ok(branches) => branches,
                Err(e) => return tx.send(Err(anyhow::anyhow!("获取本地分支失败：{}", e))),
            };

            use std::collections::HashSet;
            let local_set: HashSet<&String> = local_branches.iter().collect();

            let mut branches = Vec::with_capacity(remote_refs.len());
            for remote_ref in remote_refs {
                let short_name = remote_ref
                    .strip_prefix(&format!("{}/", remote_name))
                    .unwrap_or(&remote_ref)
                    .to_string();

                let has_local = local_set.contains(&short_name);
                let local_name = if has_local { Some(short_name.clone()) } else { None };

                branches.push(RemoteBranch {
                    remote_ref,
                    short_name,
                    has_local,
                    local_name,
                    selected: false,
                    ahead: 0,
                    behind: 0,
                    last_commit_time: String::from("-"),
                    last_commit_author: String::from("-"),
                    last_commit_message: String::from("-"),
                });
            }

            branches.sort_by(|a, b| a.short_name.to_lowercase().cmp(&b.short_name.to_lowercase()));

            // 先发送分支列表给用户显示
            let _ = tx.send(Ok(branches));

            Ok(())
        });
    }

    /// 初始化加载：直接从本地缓存读取，不显示 loading
    pub fn init_branches_from_cache(&mut self) {
        let remote_name = self.remote_name.clone();

        // 获取当前分支
        self.current_branch = git::get_current_branch().unwrap_or_else(|_| String::from("unknown"));

        // 同步加载（不显示 loading）
        match git::list_remote_branches(&remote_name) {
            Ok(remote_refs) => {
                // 获取本地分支
                match git::list_local_branches() {
                    Ok(local_branches) => {
                        use std::collections::HashSet;
                        let local_set: HashSet<&String> = local_branches.iter().collect();

                        let mut branches = Vec::new();
                        for remote_ref in remote_refs {
                            let short_name = remote_ref
                                .strip_prefix(&format!("{}/", remote_name))
                                .unwrap_or(&remote_ref)
                                .to_string();

                            let has_local = local_set.contains(&short_name);
                            let local_name = if has_local { Some(short_name.clone()) } else { None };

                            branches.push(RemoteBranch {
                                remote_ref,
                                short_name,
                                has_local,
                                local_name,
                                selected: false,
                                ahead: 0,
                                behind: 0,
                                last_commit_time: String::from("-"),
                                last_commit_author: String::from("-"),
                                last_commit_message: String::from("-"),
                            });
                        }

                        branches.sort_by(|a, b| a.short_name.to_lowercase().cmp(&b.short_name.to_lowercase()));
                        self.all_branches = branches.clone();
                        self.remote_branches = branches;
                        self.fetched_from_remote = false;
                        self.status_message = format!("已加载 {} 个分支（本地缓存）", self.remote_branches.len());

                        // 后台加载提交信息和 ahead/behind
                        self.load_ahead_behind_for_visible();
                    }
                    Err(e) => {
                        self.status_message = format!("加载本地分支失败：{}", e);
                    }
                }
            }
            Err(e) => {
                self.status_message = format!("加载远程分支失败：{}", e);
            }
        }
    }

    /// 检查异步加载是否完成
    pub fn poll_loading_complete(&mut self) -> Result<bool> {
        let mut changed = false;

        // 检查主加载任务
        if self.is_loading {
            if let Some(rx) = self.load_receiver.take() {
                match rx.try_recv() {
                    Ok(Ok(branches)) => {
                        self.all_branches = branches;
                        self.apply_filter();
                        self.is_loading = false;
                        self.loading_message.clear();
                        self.fetched_from_remote = true;
                        self.load_ahead_behind_for_visible();
                        self.status_message = format!("已加载 {} 个分支（已同步远程）", self.remote_branches.len());
                        changed = true;
                    }
                    Ok(Err(e)) => {
                        self.is_loading = false;
                        self.loading_message.clear();
                        self.status_message = format!("加载失败：{}", e);
                        changed = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        self.load_receiver = Some(rx);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.is_loading = false;
                        self.status_message = String::from("加载线程已断开");
                        changed = true;
                    }
                }
            }
        }

        // 检查 ahead/behind 和提交信息更新
        if let Some(rx) = self.load_ahead_behind_receiver.take() {
            match rx.try_recv() {
                Ok(branches) => {
                    // 更新分支数据
                    self.all_branches = branches.clone();
                    self.apply_filter();
                    // 保持接收器以便后续更新（如果需要）
                    // self.load_ahead_behind_receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.load_ahead_behind_receiver = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // 线程结束，不再接收
                }
            }
        }

        Ok(changed)
    }

    /// 懒加载：只为可见区域的分支计算 ahead/behind 和提交信息
    fn load_ahead_behind_for_visible(&mut self) {
        // 全部移到后台线程执行，避免阻塞 UI
        let mut branches = std::mem::take(&mut self.all_branches);

        let (tx, rx) = mpsc::channel::<Vec<RemoteBranch>>();

        std::thread::spawn(move || {
            // 优先级 1: 先加载前 30 个分支的 ahead/behind（可见区域）
            let visible_count = std::cmp::min(30, branches.len());
            for i in 0..visible_count {
                if branches[i].has_local {
                    if let Ok((ahead, behind)) = git::get_branch_ahead_behind(&branches[i].short_name) {
                        branches[i].ahead = ahead;
                        branches[i].behind = behind;
                    }
                }
            }

            // 优先级 2: 加载前 50 个分支的提交信息
            let batch_size = std::cmp::min(50, branches.len());
            for i in 0..batch_size {
                let commit_info = if branches[i].has_local {
                    git::get_last_commit_info(&branches[i].short_name)
                        .unwrap_or((String::from("-"), String::from("-"), String::from("-")))
                } else {
                    git::get_remote_last_commit_info(&branches[i].remote_ref)
                        .unwrap_or((String::from("-"), String::from("-"), String::from("-")))
                };
                branches[i].last_commit_time = commit_info.0;
                branches[i].last_commit_author = commit_info.1;
                branches[i].last_commit_message = commit_info.2;
            }

            // 优先级 3: 加载剩余分支的 ahead/behind
            for i in visible_count..branches.len() {
                if branches[i].has_local {
                    if let Ok((ahead, behind)) = git::get_branch_ahead_behind(&branches[i].short_name) {
                        branches[i].ahead = ahead;
                        branches[i].behind = behind;
                    }
                }
            }

            // 优先级 4: 加载剩余分支的提交信息
            for i in batch_size..branches.len() {
                let commit_info = if branches[i].has_local {
                    git::get_last_commit_info(&branches[i].short_name)
                        .unwrap_or((String::from("-"), String::from("-"), String::from("-")))
                } else {
                    git::get_remote_last_commit_info(&branches[i].remote_ref)
                        .unwrap_or((String::from("-"), String::from("-"), String::from("-")))
                };
                branches[i].last_commit_time = commit_info.0;
                branches[i].last_commit_author = commit_info.1;
                branches[i].last_commit_message = commit_info.2;
            }

            // 发送回主线程
            let _ = tx.send(branches);
        });

        // 后台接收更新（非阻塞）
        self.load_ahead_behind_receiver = Some(rx);
    }
}
