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
}

/// 应用主状态
pub struct App {
    /// 远程分支列表
    pub remote_branches: Vec<RemoteBranch>,
    /// 当前选中的分支索引
    pub cursor: usize,
    /// 状态消息
    pub status_message: String,
    /// 远程仓库名称（默认 "origin"）
    pub remote_name: String,
    /// 操作历史记录（最多保留 10 条）
    pub operation_log: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        App {
            remote_branches: Vec::new(),
            cursor: 0,
            status_message: String::from("就绪 - 按 'r' 刷新，'q' 退出"),
            remote_name: String::from("origin"),
            operation_log: Vec::new(),
        }
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
        self.remote_branches.clear();

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

            self.remote_branches.push(RemoteBranch {
                remote_ref,
                short_name,
                has_local,
                local_name,
                selected: false,
            });
        }

        // 按名称排序
        self.remote_branches.sort_by(|a, b| {
            a.short_name.to_lowercase().cmp(&b.short_name.to_lowercase())
        });

        if self.cursor >= self.remote_branches.len() {
            self.cursor = self.remote_branches.len().saturating_sub(1);
        }

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

    /// 创建所有选中的分支
    pub fn create_selected_branches(&mut self) -> Result<()> {
        // 先收集需要创建的分支信息（拥有所有权）
        let to_create: Vec<(String, String)> = self.remote_branches
            .iter()
            .filter(|b| b.selected && !b.has_local)
            .map(|b| (b.remote_ref.clone(), b.short_name.clone()))
            .collect();

        if to_create.is_empty() {
            self.status_message = String::from("没有选中的分支需要创建");
            return Ok(());
        }

        self.status_message = format!("正在创建 {} 个分支...", to_create.len());

        let mut success_count = 0;
        let mut failed_branches = Vec::new();

        for (remote_ref, short_name) in to_create {
            match git::create_local_branch(&remote_ref, &short_name) {
                Ok(_) => {
                    success_count += 1;
                    // 更新状态
                    if let Some(b) = self.remote_branches.iter_mut().find(|b| b.short_name == short_name) {
                        b.has_local = true;
                        b.local_name = Some(short_name.clone());
                        b.selected = false;
                    }
                }
                Err(e) => {
                    failed_branches.push(format!("{}: {}", short_name, e));
                }
            }
        }

        self.status_message = if failed_branches.is_empty() {
            let msg = format!("成功创建 {} 个分支", success_count);
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
