//! Git 分支管理 TUI 工具
//!
//! 基于 Elm 架构（Model-View-Update）实现

mod app;
mod domain;
mod git;
mod messages;
mod ui;

use anyhow::Result;
use app::{update, AppState, Command};
use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use messages::Message;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 恢复终端设置
fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn main() -> Result<()> {
    // 清空之前的 debug.log 文件
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .truncate(true)
        .open("debug.log")
    {
        let start_msg = format!("[{}] === 程序启动 ===\n", Local::now().format("%Y-%m-%d %H:%M:%S"));
        let _ = file.write_all(start_msg.as_bytes());
    }

    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用状态
    let mut state = AppState::new();

    // 获取当前分支
    state.current_branch = git::get_current_branch().unwrap_or_else(|_| String::from("unknown"));

    // 创建消息通道
    let (msg_tx, msg_rx) = mpsc::channel::<Message>();

    // 初始化：加载分支列表
    let remote_name = state.remote_name.clone();
    let init_cmd = Command::perform(
        move || git::list_local_branches_inner(&remote_name),
        |result| Message::BranchesLoaded(result.map_err(|e| e.to_string())),
    );
    init_cmd.execute(msg_tx.clone());

    // 主循环
    let mut last_tick = Instant::now();
    let mut should_quit = false;

    while !should_quit {
        // 处理消息
        while let Ok(msg) = msg_rx.try_recv() {
            // 检查是否是 Quit 消息
            if matches!(msg, Message::Quit) {
                should_quit = true;
                break;
            }

            let cmd = update(&mut state, msg);
            cmd.execute(msg_tx.clone());
        }

        if should_quit {
            break;
        }

        // Tick 事件（用于超时、动画）
        if last_tick.elapsed() >= Duration::from_millis(100) {
            let cmd = update(&mut state, Message::Tick);
            cmd.execute_sync(msg_tx.clone());
            last_tick = Instant::now();
        }

        // 渲染
        terminal.draw(|f| ui::draw(f, &state))?;

        // 处理输入
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let cmd = update(&mut state, Message::KeyPressed(key.code));
                    cmd.execute(msg_tx.clone());
                }
            }
        }
    }

    // 退出前恢复终端
    restore_terminal()?;
    Ok(())
}
