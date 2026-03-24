mod app;
mod git;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;

fn main() -> Result<()> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用状态
    let mut app = App::new();

    // 运行应用
    let res = run_app(&mut terminal, &mut app);

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("错误：{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::prelude::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    // 初始化：直接从本地缓存加载分支列表，不显示 loading
    app.init_branches_from_cache();

    loop {
        // 每帧检查加载是否完成（仅用于 fetch 远程时）
        app.poll_loading_complete()?;

        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                // 如果显示删除确认对话框，优先处理
                if app.show_delete_confirm {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            app.confirm_delete(true, false);
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.confirm_delete(false, false);
                        }
                        _ => {}
                    }
                    continue;
                }

                // 如果显示分支详情弹窗，按任意键关闭
                if app.show_branch_detail {
                    app.close_branch_detail();
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('?') => {
                        // 显示/隐藏帮助 overlay
                        app.toggle_help_overlay();
                    }
                    KeyCode::Char(' ') => {
                        // 勾选/取消勾选当前分支
                        app.toggle_selection();
                    }
                    KeyCode::Char('a') => {
                        // 全选/取消全选
                        app.toggle_select_all();
                    }
                    KeyCode::Char('l') => {
                        // 获取本地分支（不 fetch 远程）
                        app.start_loading_branches(false);
                    }
                    KeyCode::Char('R') | KeyCode::Char('r') => {
                        // 刷新分支列表：先 fetch 远程，再重新加载
                        app.start_loading_branches(true);
                    }
                    KeyCode::Enter => {
                        // 显示分支详情弹窗
                        app.show_branch_detail_popup();
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        // 同步选中的分支
                        app.sync_selected_branches()?;
                    }
                    KeyCode::Char('b') => {
                        // 批量创建选中的远程分支到本地
                        app.execute_selected_branches()?;
                    }
                    KeyCode::Char('c') => {
                        // 切换到当前选中的分支
                        app.checkout_current_selection()?;
                    }
                    KeyCode::Char('d') => {
                        // 删除选中的分支（显示确认对话框）
                        app.request_delete(false);
                    }
                    KeyCode::Char('D') => {
                        // 强制删除选中的分支（显示确认对话框）
                        app.request_delete(true);
                    }
                    KeyCode::Char('/') => {
                        // 进入过滤模式（简单实现：直接设置过滤）
                        app.set_filter("");
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.select_previous();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.select_next();
                    }
                    _ => {
                        // 如果帮助 overlay 显示中，按任意键关闭
                        if app.show_help_overlay {
                            app.toggle_help_overlay();
                        }
                    }
                }
            }
        }
    }
}
