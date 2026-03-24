# Git 分支管理 TUI 重构总结

**重构日期**: 2026-03-24
**状态**: 已完成

---

## 1. 重构概述

本次重构按照 `docs/plans/2026-03-24-architecture-optimization-design.md` 设计文档，将原有的混乱架构重构为清晰的 Elm 架构（Model-View-Update）。

---

## 2. 重构前的主要问题

### 2.1 状态管理混乱
- `App` 结构体有 20+ 个字段
- `remote_branches` 和 `all_branches` 存储相同类型数据，职责重叠
- 过滤逻辑需要克隆整个列表，效率低

### 2.2 异步加载逻辑脆弱
- 3 个独立的 channel 接收器需要手动管理
- 每次新请求会覆盖之前的接收器，导致消息丢失
- `poll_loading_complete()` 逻辑复杂，难以维护

### 2.3 数据流不清晰
- 后台线程修改数据，主线程被动接收
- 没有清晰的数据流方向
- 容易出现竞态条件和数据不一致

### 2.4 代码耦合严重
- 一个函数承担太多责任
- 难以进行单元测试
- 添加新功能需要修改多处代码

---

## 3. 重构后的架构

### 3.1 新的模块结构

```
src/
├── main.rs              # 程序入口，事件循环
├── app/
│   ├── mod.rs           # 模块导出
│   ├── state.rs         # AppState 定义（Model）
│   ├── update.rs        # update() 函数（消息处理）
│   └── commands.rs      # Command 实现（异步抽象）
├── domain/
│   ├── mod.rs
│   ├── branch.rs        # RemoteBranch, BranchList
│   └── loading.rs       # LoadingState, SortField
├── ui/
│   └── ui.rs            # 渲染逻辑（View）
├── git/
│   └── git.rs           # Git 命令封装
└── messages.rs          # Message 枚举定义
```

### 3.2 核心设计原则

1. **单一数据源**: 只有一个 `BranchList` 存储分支数据
2. **消息驱动更新**: 所有状态变更通过 `Message` 枚举处理
3. **任务抽象**: 异步操作封装为 `Command`，完成时发送消息
4. **清晰数据流**: `Event → Update → State → View`

### 3.3 架构模式

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

---

## 4. 关键类型定义

### 4.1 应用状态

```rust
pub struct AppState {
    pub branches: BranchList,      // 分支列表（单一数据源）
    pub cursor: usize,              // 光标位置
    pub filter_text: String,        // 过滤文本
    pub current_branch: String,     // 当前分支
    pub remote_name: String,        // 远程名称
    pub modal: Option<ModalState>,  // 弹窗状态
    pub toast: Option<Toast>,       // 提示信息
    pub operation_log: Vec<String>, // 操作日志
}
```

### 4.2 分支列表

```rust
pub struct BranchList {
    pub items: Vec<RemoteBranch>,   // 所有分支
    pub loading_state: LoadingState,// 加载状态
    pub sort_by: SortField,         // 排序字段
    pub index_map: HashMap<String, usize>, // 快速查找索引
}
```

### 4.3 消息定义

```rust
pub enum Message {
    // 用户输入
    KeyPressed(KeyCode),
    BranchToggled(usize),
    SelectAllToggled,

    // 异步任务完成
    BranchesLoaded(Result<Vec<RemoteBranch>, String>),
    CommitInfoLoaded { branch_name: String, info: CommitInfo },
    BranchCreated { branch_name: String, success: bool, message: String },
    BranchSynced { branch_name: String, success: bool, message: String },
    BranchDeleted { branch_name: String, success: bool, message: String },
    BranchCheckedOut { branch_name: String, success: bool, message: String },

    // 内部事件
    Tick,
}
```

### 4.4 命令抽象

```rust
pub struct Command<Msg> {
    inner: Option<Box<dyn FnOnce(mpsc::Sender<Msg>) + Send>>,
}

impl<Msg> Command<Msg> {
    pub fn perform<T, F, M>(task: F, mapper: M) -> Self;
    pub fn batch(commands: Vec<Self>) -> Self;
    pub fn none() -> Self;
    pub fn execute(self, tx: mpsc::Sender<Msg>);
}
```

---

## 5. 数据流示例

### 5.1 用户点击 'l' 键加载分支列表

```
1. 用户按下 'l' 键
   ↓
2. event::read() 返回 KeyCode::Char('l')
   ↓
3. update() 处理 KeyPressed('l')
   - 设置 loading_state = Loading
   - 返回 Command::perform(load_branches, Message::BranchesLoaded)
   ↓
4. Command.execute() 在后台线程执行
   ↓
5. 任务完成，通过 channel 发送 Message::BranchesLoaded
   ↓
6. 主循环接收消息，调用 update()
   - 更新 branches.items
   - 设置 loading_state = Loaded
   - 显示 Toast 提示
   ↓
7. 渲染新状态
```

### 5.2 删除分支流程

```
1. 用户按下 'd' 键
   ↓
2. update() 处理
   - 收集选中的分支
   - 设置 modal = Some(DeleteConfirm { branches, force })
   ↓
3. 用户按下 'y' 键确认
   ↓
4. update() 处理
   - 清除 modal
   - 返回 Command::batch(删除所有选中的分支)
   ↓
5. 每个删除命令执行
   - 调用 git::delete_local_branch_inner()
   - 发送 Message::BranchDeleted
   ↓
6. update() 处理 BranchDeleted
   - 更新分支状态（has_local = false）
   - 显示结果 Toast
```

---

## 6. 重构完成的功能

### 6.1 已完成

- ✅  Elm 架构实现
- ✅  单一数据源（BranchList）
- ✅  统一的 LoadingState
- ✅  Command 异步抽象
- ✅  消息驱动的更新
- ✅  Toast 提示系统
- ✅  分支列表加载
- ✅  分支批量创建
- ✅  分支批量同步
- ✅  分支批量删除
- ✅  分支切换
- ✅  弹窗系统（删除确认、帮助）
- ✅  过滤/搜索功能

### 6.2 待优化

- ⏳  FilterChanged 消息的完整实现（需要输入框支持）
- ⏳  BranchDetail 弹窗的完整实现
- ⏳  单元测试添加
- ⏳  未使用的代码清理

---

## 7. 代码质量对比

| 指标 | 重构前 | 重构后 |
|-----|------|------|
| App 结构体字段数 | 20+ | ~10 |
| 异步消息接收器 | 3 个 | 1 个（统一） |
| 状态变更入口 | 多处 | 唯一（update） |
| 代码行数 | ~1000 | ~900 |
| 编译警告 | 多个 | 10 个（均为 dead_code） |

---

## 8. 后续工作

1. **添加单元测试**: 针对 `update()` 函数编写测试
2. **完善输入功能**: 实现过滤输入框
3. **清理死代码**: 移除未使用的字段和方法
4. **添加文档**: 为公共 API 添加 Rustdoc 注释
5. **性能优化**: 优化分支列表的过滤和排序

---

## 9. 编译与运行

```bash
# 开发模式
cargo build
cargo run

# 发布模式
cargo build --release
```

---

## 10. 总结

本次重构成功将混乱的代码架构改造为清晰的 Elm 架构，主要收益：

- ✅ **可维护性**: 新增功能只需修改 `update()` 函数
- ✅ **可测试性**: `update()` 是纯函数，易于单元测试
- ✅ **可靠性**: 统一的消息处理，减少状态不一致
- ✅ **扩展性**: 添加新消息类型不影响现有代码

重构后的代码更易理解、易维护、易扩展，为后续功能开发奠定了良好基础。
