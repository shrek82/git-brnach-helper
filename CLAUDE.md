# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

基于 Rust + ratatui 的 TUI Git 分支管理工具，支持批量创建、同步、删除远程分支。

## 构建与运行

```bash
# 开发构建
cargo build

# 运行
cargo run

# 释放模式
cargo run --release
```

## 架构结构

```
src/
├── main.rs      # 程序入口，终端设置，事件循环
├── app.rs       # 应用状态管理（RemoteBranch 结构体、App 核心逻辑）
├── git.rs       # Git 命令封装（分支创建、同步、删除、查询）
└── ui.rs        # ratatui 界面渲染
```

## 核心模块职责

**app.rs**
- `RemoteBranch`：远程分支数据结构（含本地分支状态、ahead/behind、提交信息）
- `App`：主状态机（分支列表、光标位置、过滤、操作日志、异步加载）
- 批量操作：`execute_selected_branches`、`sync_selected_branches`、`delete_selected_branches`
- 懒加载：分支列表异步加载，ahead/behind 分批计算

**git.rs**
- `list_remote_branches(remote_name)`：获取指定远程的所有分支
- `create_local_branch(remote_ref, branch_name)`：基于远程创建本地分支
- `sync_local_branch(branch_name)`：同步本地分支到远程（git pull）
- `delete_local_branch(branch_name, force)`：删除本地分支
- `get_branch_ahead_behind(branch_name)`：计算与远程的分歧提交数
- `get_recent_commits(branch_name)`：获取最近 5 条提交记录

**ui.rs**
- `draw()`：主渲染函数，包含分支表格、操作日志、帮助栏
- 弹窗：帮助 overlay、删除确认对话框、分支详情弹窗、进度条、loading 提示

## 快捷键

| 按键 | 功能 |
|------|------|
| `j/↓` | 下移光标 |
| `k/↑` | 上移光标 |
| `空格` | 勾选/取消勾选当前分支 |
| `a` | 全选/取消全选 |
| `Enter` | 显示分支详情弹窗 |
| `b` | 批量创建选中的远程分支 |
| `s` | 同步选中的分支 |
| `c` | 切换到当前分支 |
| `d` | 删除选中的本地分支（确认） |
| `D` | 强制删除选中的分支 |
| `/` | 过滤分支 |
| `r` | 刷新分支列表 |
| `?` | 显示/隐藏帮助 |
| `q` | 退出 |

## 开发注意事项

1. **受保护分支**：main、master、develop、dev 默认不允许删除（`app.protected_branches`）
2. **远程仓库名称**：通过 `app.remote_name` 配置（默认 `origin`）
3. **异步加载**：启动时不阻塞 UI，通过 `mpsc::channel` 后台获取分支数据
4. **操作日志**：保留最近 10 条记录，显示在界面底部

## 架构模式

**Elm 架构（Model-View-Update）**：
- `Model` = `AppState`（单一数据源）
- `Msg` = `Message` 枚举（所有状态变更入口）
- `Update` = `update()` 函数（纯函数，处理消息返回 `Command`）
- `View` = `draw()` 函数（声明式渲染）
- `Command` = 异步操作封装（通过 channel 发送消息）

**数据流**：`用户输入/异步完成` → `Message` → `update()` → `Command` → `Message` → ...

## 模块依赖关系

```
main.rs (入口)
  ├── app (状态管理 + 更新逻辑)
  │   ├── state.rs (AppState)
  │   ├── commands.rs (Command 异步抽象)
  │   └── update.rs (消息处理)
  ├── domain (领域模型)
  │   ├── branch.rs (RemoteBranch, BranchList)
  │   └── loading.rs (LoadingState, SortField)
  ├── git (Git 命令封装)
  ├── messages (Message 定义)
  └── ui (渲染)
```
