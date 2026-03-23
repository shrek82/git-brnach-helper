use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Cell},
    Frame,
};

use crate::app::{App, RemoteBranch};

/// 绘制主界面
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(3),  // 标题
            Constraint::Min(10),    // 分支列表
            Constraint::Length(5),  // 操作日志
            Constraint::Length(3),  // 操作帮助
        ])
        .split(f.area());

    draw_title(f, chunks[0]);
    draw_branch_table(f, app, chunks[1]);
    draw_operation_log(f, app, chunks[2]);
    draw_help(f, chunks[3]);
}

/// 绘制标题栏
fn draw_title(f: &mut Frame, area: ratatui::layout::Rect) {
    let title = Paragraph::new("Git 分支管理工具")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center);
    f.render_widget(title, area);
}

/// 绘制分支列表（表格形式）
fn draw_branch_table(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut rows = Vec::new();

    for branch in app.remote_branches.iter() {
        let row = render_branch_row(branch);
        rows.push(row);
    }

    let mut table_state = TableState::default();
    table_state.select(Some(app.cursor));

    // 统计信息
    let total = app.remote_branches.len();
    let has_local = app.remote_branches.iter().filter(|b| b.has_local).count();
    let no_local = total - has_local;
    let selected = app.remote_branches.iter().filter(|b| b.selected).count();

    let title = format!(
        " 远程：{} | 共 {} 个 | 已有本地：{} | 待创建：{} | 已选中：{}  ",
        app.remote_name, total, has_local, no_local, selected
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),  // 选择
            Constraint::Length(10), // 状态
            Constraint::Min(20),    // 远程分支
            Constraint::Min(20),    // 本地分支
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(" 选择 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 状态 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 远程分支 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 本地分支 ").style(Style::default().fg(Color::Yellow)),
        ])
        .style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    )
    .row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Yellow),
    )
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut table_state);
}

/// 渲染单个分支行
fn render_branch_row(branch: &RemoteBranch) -> Row<'_> {
    // 选择列 - 使用更鲜艳的颜色
    let select_text = if branch.selected { " [✓] " } else { " [ ] " };
    let select_style = if branch.selected {
        Style::default()
            .fg(Color::Rgb(255, 200, 0))  // 亮黄色
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(100, 100, 100))  // 深灰色
            .add_modifier(Modifier::BOLD)
    };

    // 状态列
    let (status_text, status_style) = if branch.has_local {
        (" ✓ 已存在", Style::default().fg(Color::Rgb(0, 255, 100)).add_modifier(Modifier::BOLD))  // 亮绿色
    } else {
        (" ○ 待创建", Style::default().fg(Color::Rgb(150, 150, 150)))
    };

    // 远程分支列
    let remote_text = format!(" {}", branch.remote_ref);
    let remote_style = if branch.has_local {
        Style::default().fg(Color::Rgb(0, 255, 100))  // 亮绿色
    } else {
        Style::default().fg(Color::White)
    };

    // 本地分支列
    let local_text = if branch.has_local {
        branch.local_name.as_ref().unwrap().clone()
    } else {
        String::from("-")
    };
    let local_style = if branch.has_local {
        Style::default().fg(Color::Rgb(0, 255, 100))  // 亮绿色
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Row::new(vec![
        Cell::from(select_text).style(select_style),
        Cell::from(status_text).style(status_style),
        Cell::from(remote_text).style(remote_style),
        Cell::from(local_text).style(local_style),
    ])
}

/// 绘制操作日志区域
fn draw_operation_log(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // 构建日志文本
    let log_lines: Vec<Line> = app
        .operation_log
        .iter()
        .map(|log| {
            Line::from(Span::styled(
                log.clone(),
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    let placeholder = vec![Line::from(Span::styled(
        " 暂无操作记录",
        Style::default().fg(Color::DarkGray),
    ))];

    let log_content = if log_lines.is_empty() {
        placeholder
    } else {
        log_lines
    };

    let log_widget = Paragraph::new(log_content)
        .block(
            Block::default()
                .title(" 📋 操作日志 (Shift+ 鼠标选择文本可复制) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(log_widget, area);
}

/// 绘制帮助栏
fn draw_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = Line::from(vec![
        Span::styled(" ↑/k ", Style::default().fg(Color::Yellow)),
        Span::raw("上  "),
        Span::styled(" ↓/j ", Style::default().fg(Color::Yellow)),
        Span::raw("下  "),
        Span::styled(" 空格 ", Style::default().fg(Color::Yellow)),
        Span::raw("勾选  "),
        Span::styled(" a ", Style::default().fg(Color::Yellow)),
        Span::raw("全选  "),
        Span::styled(" Enter ", Style::default().fg(Color::Green)),
        Span::raw("创建  "),
        Span::styled(" r ", Style::default().fg(Color::Yellow)),
        Span::raw("刷新  "),
        Span::styled(" q ", Style::default().fg(Color::Red)),
        Span::raw("退出"),
    ]);

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" 快捷键 ")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black))
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White));
    f.render_widget(help, area);
}
