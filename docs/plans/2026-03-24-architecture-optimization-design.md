# Git 分支管理 TUI 架构优化设计文档

**创建日期**: 2026-03-24
**作者**: Claude
**状态**: 待审批

---

## 1. 概述

本文档描述 Git 分支管理 TUI 工具的架构优化方案，目标是解决当前代码中存在的状态管理混乱、异步逻辑复杂、代码耦合严重等问题。

### 1.1 优化目标

1. **清晰的数据流**：单一数据源，消息驱动更新
2. **可维护的代码**：职责分离，易于理解和修改
3. **高效的异步处理**：优雅的任务管理，无竞态条件
4. **良好的可扩展性**：添加新功能无需修改大量现有代码

---

## 2. 当前架构问题分析

### 2.1 状态管理混乱

**问题代码**:
```rust
pub struct App {
    pub remote_branches: Vec<RemoteBranch>,  // 显示用的列表（过滤后）
    pub all_branches: Vec<RemoteBranch>,     // 完整列表（用于过滤）
    // ... 20+ 个字段
}
```

**具体问题**:
- 两个列表存储相同类型的数据，职责重叠
- 过滤逻辑需要克隆整个列表，效率低
- 两处数据需要同步维护，容易产生不一致

### 2.2 异步加载逻辑脆弱

**问题代码**:
```rust
pub load_receiver: Option<mpsc::Receiver<Result<Vec<RemoteBranch>>>>,
pub load_ahead_behind_receiver: Option<mpsc::Receiver<Vec<RemoteBranch>>>,
pub debug_log_receiver: Option<mpsc::Receiver<String>>,
```

**具体问题**:
- 多个 channel 接收器需要手动 `take()` 和 `Some()` 恢复
- 每次新请求会覆盖之前的接收器，导致消息丢失
- `poll_loading_complete()` 逻辑复杂，难以维护
- 容易出现"loading 弹窗不消失"等问题

### 2.3 数据流不清晰

**问题代码**:
```rust
fn load_ahead_behind_for_visible(&mut self) {
    let mut branches = std::mem::take(&mut self.all_branches);  // 清空数据！
    std::thread::spawn(move || {
        // 后台修改数据...
        let _ = tx.send(branches);
    });
    self.load_ahead_behind_receiver = Some(rx);
}
```

**具体问题**:
- 后台线程修改数据，主线程被动接收
- 没有清晰的数据流方向
- 容易出现竞态条件和数据不一致

### 2.4 代码耦合严重

**问题代码**:
```rust
fn start_loading_branches(&mut self, fetch_remote: bool) {
    // 混合了：UI 状态、线程创建、Git 调用、日志记录
    self.is_loading = true;
    std::thread::spawn(move || {
        git::list_remote_branches(...);
    });
    self.add_log(...);
}
```

**具体问题**:
- 一个函数承担太多责任
- 难以进行单元测试
- 添加新功能需要修改多处代码

---

## 3. 新架构设计

### 3.1 核心设计原则

1. **单一数据源**: 只有一个分支列表，过滤使用视图/迭代器
2. **消息驱动更新**: 所有状态变更通过消息处理
3. **任务抽象**: 异步操作封装为 Task，完成时发送消息
4. **清晰数据流**: `Action → Reducer → State → View`

### 3.2 架构模式

采用 **Elm 架构**（也称为 Model-View-Update）:

```
┌─────────────────────────────────────────────────────────────┐
│                         Event Loop                          │
├─────────────────────────────────────────────────────────────┤
│  Events  │    Model     │    Update     │      View         │
│  ──────→ │    ──────→   │    ──────→    │    ──────→        │
│  用户输入 │   应用状态    │  状态更新逻辑  │    UI 渲染         │
│  任务完成 │  (单一数据源) │  (纯函数)     │   (声明式)        │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 模块结构

```
src/
├── main.rs              # 程序入口，事件循环
├── app/
│   ├── mod.rs           # 导出 App
│   ├── state.rs         # AppState 定义
│   ├── update.rs        # update() 函数（消息处理）
│   └── commands.rs      # Command 实现
├── domain/
│   ├── mod.rs
│   ├── branch.rs        # RemoteBranch, BranchList
│   └── loading.rs       # LoadingState, SortField
├── ui/
│   ├── mod.rs
│   ├── render.rs        # 主渲染逻辑
│   ├── widgets/         # 可复用组件
│   │   ├── table.rs
│   │   ├── modal.rs
│   │   └── toast.rs
│   └── theme.rs         # 颜色、样式
├── git/
│   ├── mod.rs
│   ├── commands.rs      # Git 命令封装
│   └── types.rs         # Git 相关类型
└── messages.rs          # Message 枚举定义
```

---

## 4. 详细设计

### 4.1 状态定义

```rust
// app/state.rs

/// 应用主状态
pub struct AppState {
    pub branches: BranchList,
    pub cursor: usize,
    pub filter_text: String,
    pub current_branch: String,
    pub modal: Option<ModalState>,
    pub toast: Option<Toast>,
}

/// 分支列表（单一数据源）
pub struct BranchList {
    pub items: Vec<RemoteBranch>,
    pub loading_state: LoadingState,
    pub sort_by: SortField,
}

/// 加载状态（替代多个布尔字段）
pub enum LoadingState {
    Idle,
    Loading { progress: u8, message: String },
    Loaded { last_updated: Instant },
    Error { message: String },
}

/// 排序字段
pub enum SortField {
    Name,
    LastCommitTime,
    Author,
}

/// 弹窗状态
pub enum ModalState {
    DeleteConfirm { branches: Vec<String>, force: bool },
    BranchDetail { branch_name: String, commits: Vec<String> },
    Help,
}

/// 提示信息
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
}

pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}
```

### 4.2 消息定义

```rust
// messages.rs

/// 所有消息的枚举（状态变更的唯一入口）
pub enum Message {
    // === 用户输入 ===
    KeyPressed(KeyCode),
    FilterChanged(String),
    BranchToggled(usize),
    SelectAllToggled,

    // === 异步任务完成 ===
    BranchesLoaded(Result<Vec<RemoteBranch>, Error>),
    CommitInfoLoaded { branch_name: String, info: CommitInfo },
    AheadBehindLoaded { branch_name: String, ahead: usize, behind: usize },
    BranchCreated { branch_name: String, success: bool },
    BranchSynced { branch_name: String, success: bool },
    BranchDeleted { branch_name: String, success: bool },

    // === 内部事件 ===
    Tick,  // 每帧调用，用于动画、超时等
}

/// 用户事件（可序列化为日志）
pub enum UserEvent {
    KeyPressed(KeyCode),
    FilterChanged(String),
    BranchAction { name: String, action: BranchActionType },
}

pub enum BranchActionType {
    Toggle,
    Create,
    Sync,
    Delete,
    Checkout,
}
```

### 4.3 更新函数（核心）

```rust
// app/update.rs

use crate::messages::{Message, UserEvent};
use crate::app::commands::Command;
use crate::git;

/// 更新函数：处理消息，返回命令
/// 这是一个纯函数（除了日志），易于测试
pub fn update(state: &mut AppState, msg: Message) -> Command<Message> {
    match msg {
        // === 按键处理 ===
        Message::KeyPressed(key_code) => handle_key_press(state, key_code),

        // === 过滤处理 ===
        Message::FilterChanged(text) => {
            state.filter_text = text;
            state.toast = Some(Toast::info(format!("过滤：{}", text)));
            Command::none()
        }

        // === 分支选择切换 ===
        Message::BranchToggled(index) => {
            if let Some(branch) = state.branches.items.get_mut(index) {
                branch.selected = !branch.selected;
            }
            Command::none()
        }

        // === 异步任务：分支列表加载完成 ===
        Message::BranchesLoaded(result) => {
            match result {
                Ok(branches) => {
                    state.branches.items = branches;
                    state.branches.loading_state = LoadingState::Loaded {
                        last_updated: Instant::now(),
                    };
                    state.toast = Some(Toast::success(format!(
                        "已加载 {} 个分支",
                        state.branches.items.len()
                    )));

                    // 返回加载提交信息的命令
                    load_commit_info_for_visible(&state.branches)
                }
                Err(e) => {
                    state.branches.loading_state = LoadingState::Error {
                        message: e.to_string(),
                    };
                    state.toast = Some(Toast::error(format!("加载失败：{}", e)));
                    Command::none()
                }
            }
        }

        // === 异步任务：提交信息加载完成 ===
        Message::CommitInfoLoaded { branch_name, info } => {
            if let Some(branch) = state.branches.items.iter_mut()
                .find(|b| b.short_name == branch_name)
            {
                branch.last_commit_time = info.time;
                branch.last_commit_author = info.author;
                branch.last_commit_message = info.message;
            }
            Command::none()
        }

        // === 内部事件：处理超时、动画 ===
        Message::Tick => {
            // 清理过期的 toast
            if let Some(toast) = &state.toast {
                if toast.created_at.elapsed() > Duration::from_secs(3) {
                    state.toast = None;
                }
            }
            Command::none()
        }
    }
}

/// 按键处理函数
fn handle_key_press(state: &mut AppState, key: KeyCode) -> Command<Message> {
    match key {
        KeyCode::Char('q') => std::process::exit(0),

        KeyCode::Char('l') => {
            state.branches.loading_state = LoadingState::Loading {
                progress: 0,
                message: String::from("正在加载分支列表..."),
            };
            Command::perform(
                load_branches_from_cache(),
                Message::BranchesLoaded,
            )
        }

        KeyCode::Char('r') => {
            state.branches.loading_state = LoadingState::Loading {
                progress: 0,
                message: String::from("正在同步远程仓库..."),
            };
            Command::perform(
                async {
                    git::fetch_remote("origin").await?;
                    git::list_remote_branches("origin").await
                },
                Message::BranchesLoaded,
            )
        }

        KeyCode::Char('s') => sync_selected_branches(state),
        KeyCode::Char('b') => create_selected_branches(state),
        KeyCode::Char('c') => checkout_current_branch(state),
        KeyCode::Char('d') => request_delete_branches(state, false),
        KeyCode::Char('D') => request_delete_branches(state, true),
        KeyCode::Char('?') => {
            state.modal = Some(ModalState::Help);
            Command::none()
        }

        // === 弹窗处理 ===
        KeyCode::Char('y') | KeyCode::Char('n') | KeyCode::Esc => {
            if state.modal.is_some() {
                state.modal = None;
            }
            Command::none()
        }

        _ => Command::none(),
    }
}
```

### 4.4 命令抽象

```rust
// app/commands.rs

use std::future::Future;
use std::sync::mpsc;
use std::thread;

/// 命令：封装异步操作
/// 完成时通过 channel 发送消息
pub struct Command<Msg> {
    inner: CommandInner<Msg>,
}

enum CommandInner<Msg> {
    /// 无操作
    None,

    /// 执行异步任务
    Perform {
        future: Box<dyn FnOnce() -> Box<dyn Future<Output = Msg> + Send> + Send>,
    },

    /// 批量执行多个命令
    Batch(Vec<Command<Msg>>),
}

impl<Msg> Command<Msg>
where
    Msg: Send + 'static,
{
    /// 创建执行异步任务的命令
    pub fn perform<Fut, F>(future_fn: F, mapper: fn(Fut::Output) -> Msg) -> Self
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
    {
        Command {
            inner: CommandInner::Perform {
                future: Box::new(move || {
                    Box::new(async move {
                        let future = future_fn();
                        mapper(future.await).await
                    })
                }),
            },
        }
    }

    /// 批量执行多个命令
    pub fn batch(commands: Vec<Self>) -> Self {
        Command {
            inner: CommandInner::Batch(commands),
        }
    }

    /// 无操作
    pub fn none() -> Self {
        Command {
            inner: CommandInner::None,
        }
    }

    /// 执行命令，将结果发送到 channel
    pub fn execute(self, tx: mpsc::Sender<Msg>) {
        match self.inner {
            CommandInner::None => {}

            CommandInner::Perform { future } => {
                let tx = tx.clone();
                thread::spawn(move || {
                    // 对于简单任务，可以直接在 thread 中执行
                    // 对于 async 任务，需要运行时
                });
            }

            CommandInner::Batch(commands) => {
                for cmd in commands {
                    cmd.execute(tx.clone());
                }
            }
        }
    }
}
```

### 4.5 简化版命令（使用线程）

考虑到项目的简单性，可以使用更简单的线程方案：

```rust
// app/commands.rs (简化版)

pub struct Command<Msg> {
    inner: Option<Box<dyn FnOnce(mpsc::Sender<Msg>) + Send>>,
}

impl<Msg: Send + 'static> Command<Msg> {
    /// 创建执行任务的命令
    pub fn perform<T, F>(task: F, mapper: fn(T) -> Msg) -> Self
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        Command {
            inner: Some(Box::new(move |tx| {
                let result = task();
                let _ = tx.send(mapper(result));
            })),
        }
    }

    /// 批量执行
    pub fn batch(commands: Vec<Self>) -> Self {
        Command {
            inner: Some(Box::new(move |tx| {
                for cmd in commands {
                    if let Some(inner) = cmd.inner {
                        inner(tx.clone());
                    }
                }
            })),
        }
    }

    /// 无操作
    pub fn none() -> Self {
        Command { inner: None }
    }

    /// 执行命令
    pub fn execute(self, tx: mpsc::Sender<Msg>) {
        if let Some(inner) = self.inner {
            std::thread::spawn(move || {
                inner(tx);
            });
        }
    }
}
```

### 4.6 主循环

```rust
// main.rs

use app::{App, Command};
use messages::Message;

fn main() -> Result<()> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用
    let mut app = App::new();

    // 创建消息通道
    let (msg_tx, msg_rx) = mpsc::channel::<Message>();

    // 初始化：加载分支列表
    let init_cmd = load_branches_from_cache();
    init_cmd.execute(msg_tx.clone());

    // 主循环
    let mut last_tick = Instant::now();
    loop {
        // 处理消息
        while let Ok(msg) = msg_rx.try_recv() {
            let cmd = update(&mut app.state, msg);
            cmd.execute(msg_tx.clone());
        }

        // Tick 事件（用于超时、动画）
        if last_tick.elapsed() >= Duration::from_millis(100) {
            let cmd = update(&mut app.state, Message::Tick);
            cmd.execute(msg_tx.clone());
            last_tick = Instant::now();
        }

        // 渲染
        terminal.draw(|f| ui::render(f, &app.state))?;

        // 处理输入
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let cmd = update(&mut app.state, Message::KeyPressed(key.code));
                    cmd.execute(msg_tx.clone());
                }
            }
        }
    }
}
```

---

## 5. 重构步骤

### 第 1 阶段：基础设施（1 天）

1. **创建新模块结构**
   - 创建 `app/`, `domain/`, `messages.rs`
   - 移动现有代码到新位置

2. **定义新类型**
   - `AppState`, `BranchList`, `LoadingState`
   - `Message` 枚举

3. **实现 Command**
   - 简化版线程方案
   - 基本测试

### 第 2 阶段：核心逻辑（1 天）

1. **实现 update 函数**
   - 按键处理
   - 分支列表加载

2. **迁移异步逻辑**
   - `load_branches()` 改为返回 `Command<Message>`
   - 使用消息处理完成事件

3. **更新主循环**
   - 使用消息通道
   - 移除 `poll_loading_complete()`

### 第 3 阶段：UI 迁移（1 天）

1. **更新渲染逻辑**
   - 使用新的状态结构
   - 移除 `app.remote_branches` / `app.all_branches`

2. **实现 Toast**
   - 替代 `status_message`
   - 自动消失

3. **实现 Modal**
   - 替代 `show_delete_confirm` 等布尔字段
   - 统一的弹窗处理

### 第 4 阶段：清理优化（0.5 天）

1. **移除旧代码**
   - 删除废弃字段
   - 清理导入

2. **添加测试**
   - `update()` 函数测试
   - 边界条件测试

3. **文档更新**
   - README 更新
   - 代码注释

---

## 6. 预期收益

| 指标 | 当前 | 优化后 |
|-----|------|-------|
| App 结构体字段数 | 20+ | ~10 |
| 异步消息接收器 | 3 个 | 1 个（统一） |
| 状态变更入口 | 多处 | 唯一（update） |
| 单元测试覆盖 | 0% | 80%+ |
| 代码行数 | ~1000 | ~900（更清晰） |

### 质量提升

- ✅ **可维护性**: 新增功能只需修改 `update()` 函数
- ✅ **可测试性**: `update()` 是纯函数，易于单元测试
- ✅ **可靠性**: 统一的消息处理，减少状态不一致
- ✅ **扩展性**: 添加新消息类型不影响现有代码

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|-----|------|---------|
| 重构引入 bug | 高 | 分阶段进行，每阶段可编译测试 |
| 性能下降 | 中 | 使用简化版 Command，避免 async 运行时开销 |
| 学习曲线 | 低 | Elm 架构简单，代码注释完善 |

---

## 8. 附录

### 8.1 参考项目

- [ratatui](https://github.com/ratatui-org/ratatui) - TUI 框架
- [elm-architecture](https://guide.elm-lang.org/architecture/) - Elm 架构指南
- [tui-react](https://github.com/fdehau/tui-rs) - React 风格的 TUI

### 8.2 术语表

- **Elm 架构**: Model-View-Update 模式，由 Elm 语言推广
- **Command**: 封装副作用的抽象，类似于 Elm 的 `Cmd`
- **Toast**: 短暂显示的提示信息，自动消失

---

## 审批

- [ ] 用户审批
- [ ] 开始实施
