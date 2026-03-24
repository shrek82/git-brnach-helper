//! 更新模块
//!
//! 所有状态变更通过 update 函数处理

use crate::app::{AppState, Command, ModalState, Toast};
use crate::messages::Message;
use crossterm::event::KeyCode;

/// 更新函数：处理消息，返回命令
/// 这是一个纯函数（除了日志），易于测试
pub fn update(state: &mut AppState, msg: Message) -> Command<Message> {
    match msg {
        // === 按键处理 ===
        Message::KeyPressed(key_code) => handle_key_press(state, key_code),

        // === 过滤处理 ===
        Message::FilterChanged(text) => {
            state.filter_text = text.clone();
            if text.is_empty() {
                state.toast = Some(Toast::info("已取消过滤"));
            } else {
                state.toast = Some(Toast::info(format!("过滤：{}", text)));
            }
            Command::none()
        }

        // === 分支选择切换 ===
        Message::BranchToggled(filtered_idx) => {
            if let Some(original_idx) = state.filtered_index_to_original(filtered_idx) {
                if let Some(branch) = state.branches.items.get_mut(original_idx) {
                    branch.selected = !branch.selected;
                    let status = if branch.selected { "已选中" } else { "已取消" };
                    state.toast = Some(Toast::info(format!("{}: {}", branch.short_name, status)));
                }
            }
            Command::none()
        }

        // === 全选切换 ===
        Message::SelectAllToggled => {
            if state.branches.items.is_empty() {
                return Command::none();
            }

            // 检查是否全部已选中
            let all_selected = state.branches.items.iter().all(|b| b.selected);
            let new_state = !all_selected;

            for branch in &mut state.branches.items {
                branch.selected = new_state;
            }

            let msg = if all_selected { "已取消全选" } else { "已全选" };
            state.toast = Some(Toast::info(msg));
            state.add_log(msg);
            Command::none()
        }

        // === 异步任务：分支列表加载完成 ===
        Message::BranchesLoaded(result) => {
            match result {
                Ok(branches) => {
                    state.branches.set_items(branches);
                    state.branches.loading_state =
                        crate::domain::LoadingState::Loaded {
                            last_updated: Instant::now(),
                        };

                    let count = state.branches.items.len();
                    state.toast = Some(Toast::success(format!("已加载 {} 个分支", count)));
                    state.add_log(&format!("刷新分支列表，共 {} 个远程分支", count));

                    // 返回加载提交信息的命令
                    load_commit_info_for_visible(state)
                }
                Err(e) => {
                    state.branches.loading_state = crate::domain::LoadingState::Error {
                        message: e.clone(),
                    };
                    state.toast = Some(Toast::error(format!("加载失败：{}", e)));
                    state.add_log(&format!("加载失败：{}", e));
                    Command::none()
                }
            }
        }

        // === 异步任务：提交信息加载完成 ===
        Message::CommitInfoLoaded { branch_name, info } => {
            state.branches.update_branch(&branch_name, |branch| {
                branch.last_commit_time = info.time;
                branch.last_commit_author = info.author;
                branch.last_commit_message = info.message;
            });
            Command::none()
        }

        // === 分支操作完成 ===
        Message::BranchCreated {
            branch_name,
            success,
            message,
        } => {
            if success {
                state.toast = Some(Toast::success(format!("创建分支成功：{}", branch_name)));
                state.branches.update_branch(&branch_name, |branch| {
                    branch.has_local = true;
                    branch.local_name = Some(branch_name.clone());
                    branch.selected = false;
                });
            } else {
                state.toast = Some(Toast::error(format!("创建分支失败：{}", message)));
            }
            state.add_log(&message);
            Command::none()
        }

        Message::BranchSynced {
            branch_name,
            success,
            message,
        } => {
            if success {
                state.toast = Some(Toast::success(format!("同步分支成功：{}", branch_name)));
            } else {
                state.toast = Some(Toast::error(format!("同步分支失败：{}", message)));
            }
            state.add_log(&message);
            Command::none()
        }

        Message::BranchDeleted {
            branch_name,
            success,
            message,
        } => {
            if success {
                state.toast = Some(Toast::success(format!("删除分支成功：{}", branch_name)));
                state.branches.update_branch(&branch_name, |branch| {
                    branch.has_local = false;
                    branch.local_name = None;
                    branch.selected = false;
                });
            } else {
                state.toast = Some(Toast::error(format!("删除分支失败：{}", message)));
            }
            state.add_log(&message);
            Command::none()
        }

        Message::BranchCheckedOut {
            branch_name,
            success,
            message,
        } => {
            if success {
                state.current_branch = branch_name.clone();
                state.toast = Some(Toast::success(format!("已切换到分支：{}", branch_name)));
            } else {
                state.toast = Some(Toast::error(format!("切换分支失败：{}", message)));
            }
            state.add_log(&message);
            Command::none()
        }

        // === 内部事件：处理超时、动画 ===
        Message::Tick => {
            // 清理过期的 toast
            if let Some(toast) = &state.toast {
                if toast.is_expired() {
                    state.toast = None;
                }
            }
            Command::none()
        }
    }
}

use std::time::Instant;

/// 按键处理函数
fn handle_key_press(state: &mut AppState, key: KeyCode) -> Command<Message> {
    // 如果显示弹窗，优先处理弹窗
    if let Some(modal) = state.modal.take() {
        match modal {
            ModalState::DeleteConfirm { branches, force } => {
                match key {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        // 确认删除：执行批量删除
                        return delete_branches_inner(branches, force);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        state.toast = Some(Toast::info("已取消删除操作"));
                        return Command::none();
                    }
                    _ => {
                        // 其他键重新设置弹窗
                        state.modal = Some(ModalState::DeleteConfirm { branches, force });
                        return Command::none();
                    }
                }
            }
            ModalState::BranchDetail { .. } | ModalState::Help => {
                // 按任意键关闭（弹窗已通过 take() 移除）
                return Command::none();
            }
        }
    }

    match key {
        KeyCode::Char('q') => {
            // 退出
            std::process::exit(0);
        }

        KeyCode::Char('l') => {
            // 获取本地分支（不 fetch 远程）
            state.branches.loading_state = crate::domain::LoadingState::Loading {
                progress: 0,
                message: String::from("正在加载分支列表..."),
            };
            let remote_name = state.remote_name.clone();
            Command::perform(
                move || crate::git::list_local_branches_inner(&remote_name),
                |result| Message::BranchesLoaded(result.map_err(|e| e.to_string())),
            )
        }

        KeyCode::Char('r') | KeyCode::Char('R') => {
            // 刷新分支列表（先 fetch 远程）
            state.branches.loading_state = crate::domain::LoadingState::Loading {
                progress: 0,
                message: String::from("正在同步远程仓库..."),
            };

            let remote_name = state.remote_name.clone();
            Command::perform(
                move || {
                    // 先 fetch
                    let _ = std::process::Command::new("git")
                        .args(["fetch", &remote_name, "--quiet"])
                        .output();
                    // 再获取分支列表
                    crate::git::list_local_branches_inner(&remote_name)
                },
                |result| Message::BranchesLoaded(result.map_err(|e| e.to_string())),
            )
        }

        KeyCode::Char(' ') => {
            // 切换当前分支选中状态
            let cursor = state.cursor;
            let filtered_len = state.filtered_indices().len();
            if cursor < filtered_len {
                return Command::perform(
                    move || cursor,
                    Message::BranchToggled,
                );
            }
            Command::none()
        }

        KeyCode::Char('a') => {
            // 全选/取消全选
            Command::perform(|| (), |_| Message::SelectAllToggled)
        }

        KeyCode::Char('s') | KeyCode::Char('S') => {
            // 同步选中的分支
            sync_selected_branches(state)
        }

        KeyCode::Char('b') => {
            // 批量创建选中的分支
            create_selected_branches(state)
        }

        KeyCode::Char('c') => {
            // 切换到当前分支
            checkout_current_branch(state)
        }

        KeyCode::Char('d') => {
            // 删除选中的分支（显示确认对话框）
            request_delete_branches(state, false)
        }

        KeyCode::Char('D') => {
            // 强制删除选中的分支
            request_delete_branches(state, true)
        }

        KeyCode::Enter => {
            // 显示分支详情
            show_branch_detail(state)
        }

        KeyCode::Char('?') => {
            // 显示/隐藏帮助
            state.modal = Some(ModalState::Help);
            Command::none()
        }

        // === 导航 ===
        KeyCode::Up | KeyCode::Char('k') => {
            if state.cursor > 0 {
                state.cursor -= 1;
            }
            Command::none()
        }

        KeyCode::Down | KeyCode::Char('j') => {
            let filtered_len = state.filtered_indices().len();
            if filtered_len > 0 && state.cursor < filtered_len - 1 {
                state.cursor += 1;
            }
            Command::none()
        }

        KeyCode::Char('/') => {
            // 进入过滤模式
            state.filter_text = String::new();
            Command::none()
        }

        _ => Command::none(),
    }
}

// === 辅助函数：加载提交信息 ===

fn load_commit_info_for_visible(state: &AppState) -> Command<Message> {
    let branches_to_load: Vec<(String, bool, String)> = state
        .branches
        .items
        .iter()
        .take(50) // 只加载前 50 个
        .map(|b| (b.short_name.clone(), b.has_local, b.remote_ref.clone()))
        .collect();

    // remote_name 用于远程分支信息加载（暂时未使用）

    Command::batch(
        branches_to_load
            .into_iter()
            .map(move |(short_name, has_local, remote_ref)| {
                let short_name_for_closure = short_name.clone();
                let short_name_for_result = short_name.clone();
                Command::perform(
                    move || {
                        if has_local {
                            crate::git::get_last_commit_info_inner(&short_name_for_closure)
                                .unwrap_or_else(|_| (String::from("-"), String::from("-"), String::from("-")))
                        } else {
                            crate::git::get_remote_last_commit_info_inner(&remote_ref)
                                .unwrap_or_else(|_| (String::from("-"), String::from("-"), String::from("-")))
                        }
                    },
                    move |info| Message::CommitInfoLoaded {
                        branch_name: short_name_for_result,
                        info: crate::messages::CommitInfo::from(info),
                    },
                )
            })
            .collect(),
    )
}

// === 辅助函数：分支操作 ===

fn sync_selected_branches(state: &AppState) -> Command<Message> {
    let to_sync: Vec<String> = state
        .branches
        .items
        .iter()
        .filter(|b| b.selected && b.has_local)
        .map(|b| b.short_name.clone())
        .collect();

    if to_sync.is_empty() {
        return Command::none();
    }

    Command::batch(
        to_sync
            .into_iter()
            .map(|branch_name| {
                let name = branch_name.clone();
                let name_for_result = name.clone();
                Command::perform(
                    move || crate::git::sync_local_branch_inner(&name),
                    move |result| {
                        let (success, message) = match result {
                            Ok(_) => (true, format!("同步分支成功：{}", name_for_result)),
                            Err(e) => (false, format!("同步分支失败：{}: {}", name_for_result, e)),
                        };
                        Message::BranchSynced {
                            branch_name: name_for_result,
                            success,
                            message,
                        }
                    },
                )
            })
            .collect(),
    )
}

fn create_selected_branches(state: &AppState) -> Command<Message> {
    let to_create: Vec<(String, String)> = state
        .branches
        .items
        .iter()
        .filter(|b| b.selected && !b.has_local)
        .map(|b| (b.remote_ref.clone(), b.short_name.clone()))
        .collect();

    if to_create.is_empty() {
        return Command::none();
    }

    Command::batch(
        to_create
            .into_iter()
            .map(|(remote_ref, branch_name)| {
                let name = branch_name.clone();
                let ref_name = remote_ref.clone();
                let name_for_result = name.clone();
                Command::perform(
                    move || crate::git::create_local_branch_inner(&ref_name, &name),
                    move |result| {
                        let (success, message) = match result {
                            Ok(_) => (true, format!("创建分支成功：{}", name_for_result)),
                            Err(e) => (false, format!("创建分支失败：{}: {}", name_for_result, e)),
                        };
                        Message::BranchCreated {
                            branch_name: name_for_result,
                            success,
                            message,
                        }
                    },
                )
            })
            .collect(),
    )
}

fn checkout_current_branch(state: &AppState) -> Command<Message> {
    if state.branches.items.is_empty() {
        return Command::none();
    }

    // 获取当前光标所在的分支
    let filtered_indices = state.filtered_indices();
    if state.cursor >= filtered_indices.len() {
        return Command::none();
    }

    let original_idx = filtered_indices[state.cursor];
    let branch = &state.branches.items[original_idx];

    if !branch.has_local {
        return Command::none(); // 没有本地分支，无法切换
    }

    let branch_name = branch.short_name.clone();
    let branch_name_for_closure = branch_name.clone();

    // 检查是否是当前分支
    if branch_name == state.current_branch {
        return Command::none(); // 已经是当前分支
    }

    Command::perform(
        move || {
            // 检查未提交修改
            if crate::git::has_uncommitted_changes_inner()? {
                return Err(anyhow::anyhow!("当前工作树有未提交的修改，无法切换分支"));
            }
            crate::git::checkout_branch_inner(&branch_name_for_closure)
        },
        move |result| {
            let (success, message) = match result {
                Ok(_) => (true, format!("已切换到分支：{}", branch_name)),
                Err(e) => (false, format!("切换分支失败：{}", e)),
            };
            Message::BranchCheckedOut {
                branch_name,
                success,
                message,
            }
        },
    )
}

fn request_delete_branches(state: &mut AppState, force: bool) -> Command<Message> {
    let to_delete: Vec<String> = state
        .branches
        .items
        .iter()
        .filter(|b| b.selected && b.has_local && !state.is_protected_branch(&b.short_name))
        .map(|b| b.short_name.clone())
        .collect();

    if to_delete.is_empty() {
        state.toast = Some(Toast::warning("没有选中的本地分支可删除"));
        return Command::none();
    }

    // 显示确认对话框
    state.modal = Some(ModalState::DeleteConfirm {
        branches: to_delete,
        force,
    });

    Command::none()
}

fn show_branch_detail(state: &mut AppState) -> Command<Message> {
    if state.branches.items.is_empty() {
        state.toast = Some(Toast::warning("没有可选的分支"));
        return Command::none();
    }

    let filtered_indices = state.filtered_indices();
    if state.cursor >= filtered_indices.len() {
        return Command::none();
    }

    let original_idx = filtered_indices[state.cursor];
    let branch = &state.branches.items[original_idx];
    let branch_name = branch.short_name.clone();
    let branch_name_for_closure = branch_name.clone();

    if !branch.has_local {
        state.toast = Some(Toast::warning(format!(
            "分支 '{}' 尚未创建到本地",
            branch_name
        )));
        return Command::none();
    }

    // 获取最近提交记录
    Command::perform(
        move || crate::git::get_recent_commits_inner(&branch_name_for_closure),
        move |result| {
            let commits = result.unwrap_or_else(|_| vec![String::from("获取提交记录失败")]);
            // 这里需要更新状态来显示弹窗，暂时简化处理
            Message::CommitInfoLoaded {
                branch_name,
                info: crate::messages::CommitInfo {
                    time: String::from("-"),
                    author: String::from("-"),
                    message: commits.join("; "),
                },
            }
        },
    )
}

/// 删除分支的内部实现
fn delete_branches_inner(branches: Vec<String>, force: bool) -> Command<Message> {
    if branches.is_empty() {
        return Command::none();
    }

    Command::batch(
        branches
            .into_iter()
            .map(|branch_name| {
                let name = branch_name.clone();
                let name_for_result = name.clone();
                Command::perform(
                    move || crate::git::delete_local_branch_inner(&name, force),
                    move |result| {
                        let (success, message) = match result {
                            Ok(_) => (true, format!("删除分支成功：{}", name_for_result)),
                            Err(e) => (false, format!("删除分支失败：{}: {}", name_for_result, e)),
                        };
                        Message::BranchDeleted {
                            branch_name: name_for_result,
                            success,
                            message,
                        }
                    },
                )
            })
            .collect(),
    )
}

impl AppState {
    /// 检查分支是否是受保护的
    fn is_protected_branch(&self, branch_name: &str) -> bool {
        const PROTECTED: [&str; 4] = ["main", "master", "develop", "dev"];
        PROTECTED.iter().any(|p| p == &branch_name)
    }
}
