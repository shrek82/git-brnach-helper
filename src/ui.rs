use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table, TableState, Cell},
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

    // 绘制帮助 overlay
    if app.show_help_overlay {
        draw_help_overlay(f);
    }

    // 绘制删除确认对话框
    if app.show_delete_confirm {
        draw_delete_confirm(f, app);
    }

    // 绘制分支详情弹窗
    if app.show_branch_detail {
        draw_branch_detail(f, app);
    }

    // 绘制进度条
    if app.is_operating && app.progress_total > 0 {
        draw_progress(f, app);
    }

    // 绘制 loading 提示（加载或同步时显示）
    if app.is_loading || app.is_operating {
        draw_loading(f, app);
    }
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

    let filter_info = if !app.filter_text.is_empty() {
        format!("过滤：{} | ", app.filter_text)
    } else {
        String::new()
    };

    let title = format!(
        " {}远程：{} | 共 {} 个 | 已有本地：{} | 待创建：{} | 已选中：{}  ",
        filter_info, app.remote_name, total, has_local, no_local, selected
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),   // 选择
            Constraint::Length(8),   // 状态
            Constraint::Min(25),     // 远程分支
            Constraint::Min(25),     // 本地分支
            Constraint::Length(12),  // 最后提交
            Constraint::Length(12),  // 作者
            Constraint::Min(30),     // 提交消息
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(" 选择 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 状态 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 远程分支 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 本地分支 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 最后提交 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 作者 ").style(Style::default().fg(Color::Yellow)),
            Cell::from(" 提交消息 ").style(Style::default().fg(Color::Yellow)),
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

    // 状态列 - 显示 ahead/behind 信息
    let status_text = if branch.has_local {
        if branch.ahead > 0 && branch.behind > 0 {
            format!(" ↑{}↓{}", branch.ahead, branch.behind)
        } else if branch.ahead > 0 {
            format!(" ↑{}", branch.ahead)
        } else if branch.behind > 0 {
            format!(" ↓{}", branch.behind)
        } else {
            " ✓ 同步".to_string()
        }
    } else {
        String::from(" ○ 待创建")
    };

    let status_style = if branch.has_local {
        if branch.ahead > 0 || branch.behind > 0 {
            Style::default().fg(Color::Rgb(255, 200, 0)).add_modifier(Modifier::BOLD)  // 黄色警告
        } else {
            Style::default().fg(Color::Rgb(0, 255, 100)).add_modifier(Modifier::BOLD)  // 绿色正常
        }
    } else {
        Style::default().fg(Color::Rgb(150, 150, 150))
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

    // 最后提交时间列
    let time_text = branch.last_commit_time.clone();
    let time_style = Style::default().fg(Color::Rgb(200, 200, 200));

    // 作者列
    let author_text = branch.last_commit_author.clone();
    let author_style = Style::default().fg(Color::Rgb(180, 180, 255));

    // 提交消息列
    let message_text = branch.last_commit_message.clone();
    let message_style = Style::default().fg(Color::White);

    Row::new(vec![
        Cell::from(select_text).style(select_style),
        Cell::from(status_text).style(status_style),
        Cell::from(remote_text).style(remote_style),
        Cell::from(local_text).style(local_style),
        Cell::from(time_text).style(time_style),
        Cell::from(author_text).style(author_style),
        Cell::from(message_text).style(message_style),
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
        Span::raw("详情  "),
        Span::styled(" s ", Style::default().fg(Color::Cyan)),
        Span::raw("同步  "),
        Span::styled(" b ", Style::default().fg(Color::Green)),
        Span::raw("创建  "),
        Span::styled(" c ", Style::default().fg(Color::Cyan)),
        Span::raw("切换  "),
        Span::styled(" d ", Style::default().fg(Color::Red)),
        Span::raw("删除  "),
        Span::styled(" / ", Style::default().fg(Color::Yellow)),
        Span::raw("过滤  "),
        Span::styled(" ? ", Style::default().fg(Color::Magenta)),
        Span::raw("帮助  "),
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

/// 绘制帮助 overlay（全屏弹窗）
fn draw_help_overlay(f: &mut Frame) {
    let area = f.area();

    // 计算居中弹窗大小
    let popup_width = 70;
    let popup_height = 20;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    let help_lines = vec![
        Line::from(vec![
            Span::styled("═══ 快捷键帮助 ═══", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑/k      ", Style::default().fg(Color::Yellow)),
            Span::raw("向上移动"),
        ]),
        Line::from(vec![
            Span::styled("  ↓/j      ", Style::default().fg(Color::Yellow)),
            Span::raw("向下移动"),
        ]),
        Line::from(vec![
            Span::styled("  空格     ", Style::default().fg(Color::Yellow)),
            Span::raw("勾选/取消勾选当前分支"),
        ]),
        Line::from(vec![
            Span::styled("  a        ", Style::default().fg(Color::Yellow)),
            Span::raw("全选/取消全选"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(Color::Green)),
            Span::raw("执行操作（创建/同步）"),
        ]),
        Line::from(vec![
            Span::styled("  c        ", Style::default().fg(Color::Cyan)),
            Span::raw("切换到当前分支"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(Color::Red)),
            Span::raw("删除选中的本地分支"),
        ]),
        Line::from(vec![
            Span::styled("  D        ", Style::default().fg(Color::Red)),
            Span::raw("强制删除选中的分支"),
        ]),
        Line::from(vec![
            Span::styled("  r        ", Style::default().fg(Color::Yellow)),
            Span::raw("刷新分支列表"),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(Color::Yellow)),
            Span::raw("过滤/搜索分支"),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(Color::Magenta)),
            Span::raw("显示/隐藏帮助"),
        ]),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(Color::Red)),
            Span::raw("退出"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  按任意键关闭帮助", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let help_overlay = Paragraph::new(help_lines)
        .block(
            Block::default()
                .title(" 帮助 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left);

    f.render_widget(help_overlay, popup_area);
}

/// 绘制删除确认对话框
fn draw_delete_confirm(f: &mut Frame, app: &App) {
    let area = f.area();

    // 计算居中弹窗大小
    let popup_width = 50;
    let popup_height = 8;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    let confirm_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("⚠️  确认删除 ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(format!("{} 个分支？", app.pending_delete_count)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y] 确认删除  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" [n] 取消  ", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    let confirm_dialog = Paragraph::new(confirm_lines)
        .block(
            Block::default()
                .title(" 删除确认 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .style(Style::default().bg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    f.render_widget(confirm_dialog, popup_area);
}

/// 绘制进度条
fn draw_progress(f: &mut Frame, app: &App) {
    let area = f.area();

    // 在底部绘制进度条
    let progress_height = 3;
    let progress_area = ratatui::layout::Rect {
        x: 0,
        y: area.height.saturating_sub(progress_height),
        width: area.width,
        height: progress_height,
    };

    let progress_ratio = if app.progress_total > 0 {
        app.progress_current as f64 / app.progress_total as f64
    } else {
        0.0
    };

    let progress_label = format!("{} / {} ({:.0}%)",
        app.progress_current,
        app.progress_total,
        progress_ratio * 100.0
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent((progress_ratio * 100.0) as u16)
        .label(progress_label)
        .block(
            Block::default()
                .title(" 执行中 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(gauge, progress_area);
}

/// 绘制分支详情弹窗
fn draw_branch_detail(f: &mut Frame, app: &App) {
    let area = f.area();

    // 计算居中弹窗大小
    let popup_width = 70;
    let popup_height = 20;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    // 获取当前分支信息
    let branch = app.remote_branches.iter().find(|b| b.short_name == app.detail_branch_name);

    let mut detail_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("📋 分支详情", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("分支名称：", Style::default().fg(Color::Yellow)),
            Span::raw(app.detail_branch_name.clone()),
        ]),
    ];

    if let Some(b) = branch {
        let status = if b.has_local {
            format!("已存在本地分支 (ahead: {}, behind: {})", b.ahead, b.behind)
        } else {
            String::from("待创建")
        };
        detail_lines.push(Line::from(vec![
            Span::styled("状态：", Style::default().fg(Color::Yellow)),
            Span::raw(status),
        ]));

        if b.has_local {
            detail_lines.push(Line::from(vec![
                Span::styled("远程引用：", Style::default().fg(Color::Yellow)),
                Span::raw(b.remote_ref.clone()),
            ]));
        }
    }

    detail_lines.push(Line::from(""));
    detail_lines.push(Line::from(vec![
        Span::styled("最近提交记录：", Style::default().fg(Color::Yellow)),
    ]));

    if app.recent_commits.is_empty() {
        detail_lines.push(Line::from(vec![
            Span::styled("  暂无提交记录", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        for commit in &app.recent_commits {
            detail_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::raw(commit.clone()),
            ]));
        }
    }

    detail_lines.push(Line::from(""));
    detail_lines.push(Line::from(vec![
        Span::styled("  按任意键关闭", Style::default().fg(Color::DarkGray)),
    ]));

    let detail_dialog = Paragraph::new(detail_lines)
        .block(
            Block::default()
                .title(" 分支详情 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left);

    f.render_widget(detail_dialog, popup_area);
}

/// 绘制 loading 提示（简单紧凑文字版）
fn draw_loading(f: &mut Frame, app: &App) {
    let area = f.area();

    // 计算垂直居中的弹窗位置
    let popup_width = 40;
    let popup_height = 5;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = ratatui::layout::Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    let loading_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                &app.loading_message,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "请稍候...",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
    ];

    let loading_widget = Paragraph::new(loading_lines)
        .block(
            Block::default()
                .title(" 加载中 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    f.render_widget(loading_widget, popup_area);
}
