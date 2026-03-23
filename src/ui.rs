use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{App, RemoteBranch};

/// 绘制主界面
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // 标题
            Constraint::Min(10),    // 分支列表
            Constraint::Length(3),  // 操作提示
            Constraint::Length(1),  // 状态消息
        ])
        .split(f.area());

    draw_title(f, chunks[0]);
    draw_branch_list(f, app, chunks[1]);
    draw_help(f, chunks[2]);
    draw_status(f, app, chunks[3]);
}

/// 绘制标题栏
fn draw_title(f: &mut Frame, area: ratatui::layout::Rect) {
    let title = Paragraph::new("Git 分支管理工具")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

/// 绘制分支列表
fn draw_branch_list(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut items = Vec::new();

    for branch in app.remote_branches.iter() {
        let item = render_branch_item(branch);
        items.push(item);
    }

    let mut list_state = ListState::default();
    list_state.select(Some(app.cursor));

    let branches_list = List::new(items)
        .block(
            Block::default()
                .title(format!("远程分支 (远程：{})", app.remote_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(Color::Yellow),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(branches_list, area, &mut list_state);
}

/// 渲染单个分支项
fn render_branch_item(branch: &RemoteBranch) -> ListItem<'_> {
    let (status_symbol, status_style) = if branch.has_local {
        ("✓", Style::default().fg(Color::Green))
    } else {
        ("○", Style::default().fg(Color::DarkGray))
    };

    let select_symbol = if branch.selected { "[●]" } else { "[ ]" };

    let mut spans = vec![
        Span::styled(
            select_symbol,
            if branch.selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::raw(" "),
        Span::styled(status_symbol, status_style),
        Span::raw(" "),
        Span::styled(
            &branch.short_name,
            if branch.has_local {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            },
        ),
    ];

    if branch.has_local {
        spans.push(Span::styled(
            format!(" (← {})", branch.local_name.as_ref().unwrap()),
            Style::default().fg(Color::DarkGray),
        ));
    }

    ListItem::new(Line::from(spans))
}

/// 绘制帮助信息
fn draw_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = vec![
        Span::styled("↑/k", Style::default().fg(Color::Yellow)),
        Span::raw(":上 "),
        Span::styled("↓/j", Style::default().fg(Color::Yellow)),
        Span::raw(":下 "),
        Span::styled("空格", Style::default().fg(Color::Yellow)),
        Span::raw(":勾选 "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(":全选 "),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::raw(":创建 "),
        Span::styled("r", Style::default().fg(Color::Yellow)),
        Span::raw(":刷新 "),
        Span::styled("q", Style::default().fg(Color::Red)),
        Span::raw(":退出"),
    ];

    let help = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL).title("操作帮助"));
    f.render_widget(help, area);
}

/// 绘制状态消息
fn draw_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::White));
    f.render_widget(status, area);
}
