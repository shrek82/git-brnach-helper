//! 更新模块
//!
//! 所有状态变更通过 update 函数处理

use crate::app::{AppState, Command, ModalState};
use crate::messages::Message;
use crossterm::event::KeyCode;

/// 更新函数：处理消息，返回命令
/// 这是一个纯函数（除了日志），易于测试
pub fn update(state: &mut AppState, msg: Message) -> Command<Message> {
    match msg {
        // === 按键处理 ===
        Message::KeyPressed(key_code) => handle_key_press(state, key_code),

        // === 分支选择切换 ===
        Message::BranchToggled(filtered_idx) => {
            if let Some(original_idx) = state.filtered_index_to_original(filtered_idx) {
                if let Some(branch) = state.branches.items.get_mut(original_idx) {
                    branch.selected = !branch.selected;
                    let status = if branch.selected { "已选中" } else { "已取消" };
                    let branch_name = branch.short_name.clone();
                    let _ = branch; // 忽略借用
                    state.add_log(&format!("{}: {}", branch_name, status));
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
                    state.add_log(&format!("刷新分支列表，共 {} 个远程分支", count));

                    // 返回加载提交信息的命令
                    load_commit_info_for_visible(state)
                }
                Err(e) => {
                    state.branches.loading_state = crate::domain::LoadingState::Error {
                        message: e.clone(),
                    };
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
            progress,
        } => {
            if success {
                state.branches.update_branch(&branch_name, |branch| {
                    branch.has_local = true;
                    branch.local_name = Some(branch_name.clone());
                    branch.selected = false;
                });
            }
            state.add_log(&message);
            // 更新进度状态
            if let Some((current, total)) = progress {
                if current >= total {
                    // 完成，清除加载状态
                    state.branches.loading_state = crate::domain::LoadingState::Loaded {
                        last_updated: Instant::now(),
                    };
                } else {
                    // 更新进度
                    state.branches.loading_state = crate::domain::LoadingState::Loading {
                        progress: (current as u8 * 100 / total as u8).min(100),
                        message: format!("正在创建分支... {}/{}", current, total),
                    };
                }
            }
            Command::none()
        }

        Message::BranchSynced {
            branch_name,
            success,
            message,
            progress,
        } => {
            if success {
                state.add_log(&format!("同步分支成功：{}", branch_name));
            } else {
                state.add_log(&message);
            }
            // 更新进度状态
            if let Some((current, total)) = progress {
                if current >= total {
                    // 完成，清除加载状态
                    state.branches.loading_state = crate::domain::LoadingState::Loaded {
                        last_updated: Instant::now(),
                    };
                } else {
                    // 更新进度
                    state.branches.loading_state = crate::domain::LoadingState::Loading {
                        progress: (current as u8 * 100 / total as u8).min(100),
                        message: format!("正在同步分支... {}/{}", current, total),
                    };
                }
            }
            Command::none()
        }

        Message::BranchDeleted {
            branch_name,
            success,
            message,
            progress,
        } => {
            if success {
                state.add_log(&message);
                // 只有删除远程分支成功后，才从列表中移除
                // 因为远程分支删除意味着这个分支引用完全不存在了
                if message.contains("删除远程") && message.contains("成功") {
                    state.branches.items.retain(|b| b.short_name != branch_name);
                } else if message.contains("删除本地") && message.contains("成功") {
                    // 只删除本地分支，更新状态
                    state.branches.update_branch(&branch_name, |branch| {
                        branch.has_local = false;
                        branch.local_name = None;
                        branch.selected = false;
                    });
                }
            } else {
                state.add_log(&message);
            }
            // 更新进度状态
            if let Some((current, total)) = progress {
                if current >= total {
                    // 完成，清除加载状态
                    state.branches.loading_state = crate::domain::LoadingState::Loaded {
                        last_updated: Instant::now(),
                    };
                } else {
                    // 更新进度
                    state.branches.loading_state = crate::domain::LoadingState::Loading {
                        progress: (current as u8 * 100 / total as u8).min(100),
                        message: format!("正在删除分支... {}/{}", current, total),
                    };
                }
            }
            Command::none()
        }

        Message::BranchCheckedOut {
            branch_name,
            success,
            message,
        } => {
            if success {
                state.current_branch = branch_name.clone();
                state.add_log(&format!("已切换到分支：{}", branch_name));
            } else {
                state.add_log(&message);
            }
            Command::none()
        }

        // === 分支详情准备好显示 ===
        Message::BranchDetailReady { branch_name, commits } => {
            state.modal = Some(ModalState::BranchDetail { branch_name, commits });
            Command::none()
        }

        // === 内部事件：处理超时、动画 ===
        Message::Tick => {
            Command::none()
        }

        // === 退出应用 ===
        Message::Quit => {
            // 返回一个特殊命令，主循环会处理
            Command::none()
        }

        // === 其他消息（预留） ===
        Message::FilterChanged(_) => {
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
            ModalState::DeleteConfirm { branches, delete_remote } => {
                match key {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        // 确认删除：先设置加载状态，然后执行批量删除
                        let total = branches.len();
                        let delete_type = if delete_remote { "删除远程" } else { "删除本地" };
                        state.branches.loading_state = crate::domain::LoadingState::Loading {
                            progress: 0,
                            message: format!("正在{} {} 个分支...", delete_type, total),
                        };
                        return delete_branches_inner(branches, delete_remote);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        state.add_log("已取消删除操作");
                        return Command::none();
                    }
                    _ => {
                        // 其他键重新设置弹窗
                        state.modal = Some(ModalState::DeleteConfirm { branches, delete_remote });
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
            return Command::perform(|| (), |_| Message::Quit);
        }

        KeyCode::Char('f') => {
            // fetch 所有分支
            state.branches.loading_state = crate::domain::LoadingState::Loading {
                progress: 0,
                message: String::from("正在 fetch 所有分支..."),
            };
            let remote_name = state.remote_name.clone();
            let remote_name_for_task = remote_name.clone();
            Command::perform(
                move || crate::git::fetch_all_branches(&remote_name),
                move |_| {
                    let remote_name_for_list = remote_name_for_task.clone();
                    match crate::git::list_local_branches_inner(&remote_name_for_list) {
                        Ok(branches) => Message::BranchesLoaded(Ok(branches)),
                        Err(e) => Message::BranchesLoaded(Err(e.to_string())),
                    }
                },
            )
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
                    let fetch_output = std::process::Command::new("git")
                        .args(["fetch", &remote_name, "--quiet"])
                        .output();

                    // 忽略 fetch 错误，继续获取分支列表
                    if let Err(e) = fetch_output {
                        eprintln!("fetch 远程失败：{}", e);
                    }

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
            // 删除选中的本地分支
            request_delete_branches(state, false)
        }

        KeyCode::Char('D') => {
            // 删除选中的本地分支 + 远程分支
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

fn sync_selected_branches(state: &mut AppState) -> Command<Message> {
    let to_sync: Vec<String> = state
        .branches
        .items
        .iter()
        .filter(|b| b.selected && b.has_local)
        .map(|b| b.short_name.clone())
        .collect();

    let to_create_and_sync: Vec<(String, String)> = state
        .branches
        .items
        .iter()
        .filter(|b| b.selected && !b.has_local)
        .map(|b| (b.remote_ref.clone(), b.short_name.clone()))
        .collect();

    if to_sync.is_empty() && to_create_and_sync.is_empty() {
        return Command::none();
    }

    let total = to_sync.len() + to_create_and_sync.len();
    let to_sync_len = to_sync.len();
    // 设置加载状态，显示进度
    state.branches.loading_state = crate::domain::LoadingState::Loading {
        progress: 0,
        message: format!("正在同步/创建 {} 个分支...", total),
    };

    let mut all_commands = Vec::new();

    // 同步已有本地分支的命令
    for (i, branch_name) in to_sync.into_iter().enumerate() {
        let name = branch_name.clone();
        let name_for_result = name.clone();
        let progress = i + 1;
        all_commands.push(Command::perform(
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
                    progress: Some((progress, total)),
                }
            },
        ));
    }

    // 创建新分支的命令
    for (i, (remote_ref, branch_name)) in to_create_and_sync.into_iter().enumerate() {
        let name = branch_name.clone();
        let ref_name = remote_ref.clone();
        let name_for_result = name.clone();
        let progress = to_sync_len + i + 1;
        all_commands.push(Command::perform(
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
                    progress: Some((progress, total)),
                }
            },
        ));
    }

    Command::batch(all_commands)
}

fn create_selected_branches(state: &mut AppState) -> Command<Message> {
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

    let total = to_create.len();
    // 设置加载状态，显示进度
    state.branches.loading_state = crate::domain::LoadingState::Loading {
        progress: 0,
        message: format!("正在创建 {} 个分支...", total),
    };

    Command::batch(
        to_create
            .into_iter()
            .enumerate()
            .map(move |(i, (remote_ref, branch_name))| {
                let name = branch_name.clone();
                let ref_name = remote_ref.clone();
                let name_for_result = name.clone();
                let progress = i + 1;
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
                            progress: Some((progress, total)),
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
        move || crate::git::checkout_branch_inner(&branch_name_for_closure),
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

fn request_delete_branches(state: &mut AppState, delete_remote: bool) -> Command<Message> {
    let to_delete: Vec<String> = if delete_remote {
        // D 删除：可以删除纯远程分支（无需本地分支），但受保护分支除外
        state.branches.items
            .iter()
            .filter(|b| b.selected && !state.is_protected_branch(&b.short_name))
            .map(|b| b.short_name.clone())
            .collect()
    } else {
        // d 删除：只删除有本地分支的
        state.branches.items
            .iter()
            .filter(|b| b.selected && b.has_local && !state.is_protected_branch(&b.short_name))
            .map(|b| b.short_name.clone())
            .collect()
    };

    if to_delete.is_empty() {
        if delete_remote {
            state.add_log("没有选中的分支可删除");
        } else {
            state.add_log("没有选中的本地分支可删除");
        }
        return Command::none();
    }

    // 显示确认对话框
    state.modal = Some(ModalState::DeleteConfirm {
        branches: to_delete,
        delete_remote,
    });

    Command::none()
}

fn show_branch_detail(state: &mut AppState) -> Command<Message> {
    if state.branches.items.is_empty() {
        state.add_log("没有可选的分支");
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
        state.add_log(&format!("分支 '{}' 尚未创建到本地", branch_name));
        return Command::none();
    }

    // 获取最近提交记录并显示详情弹窗
    Command::perform(
        move || crate::git::get_recent_commits_inner(&branch_name_for_closure),
        move |result| {
            let commits = result.unwrap_or_else(|_| vec![]);
            Message::BranchDetailReady { branch_name, commits }
        },
    )
}

/// 删除分支的内部实现 - 返回删除命令和相关信息
fn delete_branches_inner(branches: Vec<String>, delete_remote: bool) -> Command<Message> {
    if branches.is_empty() {
        return Command::none();
    }

    let remote_name = String::from("origin");
    let total = branches.len();

    // 创建一个命令来设置初始加载状态，然后执行删除
    // 由于 Command::batch 会并行执行所有命令，我们需要使用顺序执行的方式
    // 这里我们直接在每个命令的消息处理中更新进度，第一个命令会设置初始状态
    
    Command::batch(
        branches
            .into_iter()
            .enumerate()
            .map(move |(i, branch_name)| {
                let name = branch_name.clone();
                let name_for_result = name.clone();
                let remote_name_for_delete = remote_name.clone();
                let progress = i + 1;

                if delete_remote {
                    let name_for_remote = branch_name.clone();
                    let name_for_remote_result = branch_name;
                    Command::perform(
                        move || crate::git::delete_remote_branch_inner(&name_for_remote, &remote_name_for_delete),
                        move |result| {
                            let (success, message) = match result {
                                Ok(_) => (true, format!("删除远程分支成功：{}", name_for_remote_result)),
                                Err(e) => (false, format!("删除远程分支失败：{}: {}", name_for_remote_result, e)),
                            };
                            Message::BranchDeleted {
                                branch_name: name_for_remote_result,
                                success,
                                message,
                                progress: Some((progress, total)),
                            }
                        },
                    )
                } else {
                    Command::perform(
                        move || crate::git::delete_local_branch_inner(&name, false),
                        move |result| {
                            let (success, message) = match result {
                                Ok(_) => (true, format!("删除本地分支成功：{}", name_for_result)),
                                Err(e) => (false, format!("删除本地分支失败：{}: {}", name_for_result, e)),
                            };
                            Message::BranchDeleted {
                                branch_name: name_for_result,
                                success,
                                message,
                                progress: Some((progress, total)),
                            }
                        },
                    )
                }
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
