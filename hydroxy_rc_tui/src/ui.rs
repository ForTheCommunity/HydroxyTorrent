use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap},
};

use crate::{
    app::{App, ConnectionState, DetailTab, FocusPane, LoginField, ModalState, SortColumn},
    models::{
        LogLevel, TorrentDetail, TorrentItem, format_bytes, format_duration, format_speed,
        sanitize_relative_path,
    },
    theme::ThemePalette,
};

pub fn draw(f: &mut Frame, app: &App) {
    let p = app.theme.palette();
    let area = f.area();

    // Base background fill
    f.render_widget(Block::default().style(Style::default().bg(p.bg)), area);

    if app.modal == ModalState::Login {
        draw_login_screen(f, app, &p, area);
        return;
    }

    let root_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top bar
            Constraint::Min(8),    // Main content
            Constraint::Length(1), // Bottom status bar
        ])
        .split(area);

    draw_top_bar(f, app, &p, root_chunks[0]);
    draw_main_body(f, app, &p, root_chunks[1]);
    draw_status_bar(f, app, &p, root_chunks[2]);

    match &app.modal {
        ModalState::None | ModalState::Login => {}
        ModalState::SearchInput => draw_search_modal(f, app, &p, area),
        ModalState::RemoveConfirm {
            delete_files,
            targets,
        } => draw_remove_modal(f, &p, area, *delete_files, targets),
        ModalState::ShutdownConfirm => draw_shutdown_modal(f, &p, area),
        ModalState::Logs => draw_logs_modal(f, app, &p, area),
        ModalState::Help => draw_help_modal(f, &p, area),
    }
}

fn draw_top_bar(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.border))
        .style(Style::default().bg(p.header_bg));

    let search_badge = if app.search_query.is_empty() {
        Span::styled(
            " [/:Search] ",
            Style::default().fg(p.text_muted),
        )
    } else {
        Span::styled(
            format!(" [Filter: \"{}\"] ", app.search_query),
            Style::default()
                .fg(p.accent_amber)
                .add_modifier(Modifier::BOLD),
        )
    };

    let line = Line::from(vec![
        Span::styled(
            " 💧 HYDROXY TORRENT RC ",
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("| ", Style::default().fg(p.border)),
        Span::styled(
            "[r]Resume  [p]Pause  [c]Recheck  [a]Reannounce  [o]Seq  [x]Remove  [l]Logs  [t]Theme:",
            Style::default().fg(p.text_secondary),
        ),
        Span::styled(
            format!("{} ", app.theme.name()),
            Style::default()
                .fg(p.accent_purple)
                .add_modifier(Modifier::BOLD),
        ),
        search_badge,
        Span::styled(" [?]Help ", Style::default().fg(p.accent_green)),
    ]);

    let paragraph = Paragraph::new(line).block(block);
    f.render_widget(paragraph, area);
}

fn draw_main_body(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let horiz = if app.show_sidebar {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Min(40)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Min(40)])
            .split(area)
    };

    if app.show_sidebar {
        draw_sidebar(f, app, p, horiz[0]);
    }

    let right_area = horiz[1];
    if app.show_details {
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(right_area);
        draw_torrents_table(f, app, p, vert[0]);
        draw_details_panel(f, app, p, vert[1]);
    } else {
        draw_torrents_table(f, app, p, right_area);
    }
}

fn draw_sidebar(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let is_focused = app.focus == FocusPane::Sidebar;
    let border_color = if is_focused {
        p.border_focused
    } else {
        p.border
    };

    let entries = app.sidebar_entries();
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(idx, e)| {
            if e.is_header {
                let line = Line::from(vec![Span::styled(
                    format!(" ── {} ──", e.label),
                    Style::default()
                        .fg(p.text_muted)
                        .add_modifier(Modifier::BOLD),
                )]);
                ListItem::new(line)
            } else {
                let is_active = app.active_filter == e.filter_key;
                let is_cursor = is_focused && app.sidebar_index == idx;
                let indent = "  ".repeat(e.depth);
                let marker = if is_active { "▸ " } else { "  " };
                let style = if is_cursor {
                    Style::default()
                        .fg(p.text_primary)
                        .bg(p.selected_bg)
                        .add_modifier(Modifier::BOLD)
                } else if is_active {
                    Style::default()
                        .fg(p.accent_blue)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(p.text_secondary)
                };

                let line = Line::from(vec![
                    Span::styled(format!("{}{}{}", indent, marker, e.label), style),
                    Span::styled(
                        format!(" ({})", e.count),
                        Style::default().fg(p.text_muted),
                    ),
                ]);
                ListItem::new(line).style(if is_cursor {
                    Style::default().bg(p.selected_bg)
                } else {
                    Style::default()
                })
            }
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " Categories [b] ",
            Style::default()
                .fg(if is_focused {
                    p.accent_blue
                } else {
                    p.text_secondary
                })
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(p.panel_bg));

    f.render_widget(List::new(items).block(block), area);
}

fn draw_torrents_table(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let is_focused = app.focus == FocusPane::Torrents;
    let border_color = if is_focused {
        p.border_focused
    } else {
        p.border
    };

    let torrents = app.filtered_torrents();
    let arrow = if app.sort_asc { "▲" } else { "▼" };
    let col_title = |col: SortColumn, name: &'static str| -> String {
        if app.sort_col == col {
            format!("{} {}", name, arrow)
        } else {
            name.to_string()
        }
    };

    let header = Row::new(vec![
        Cell::from(col_title(SortColumn::Name, "Name")),
        Cell::from("Category"),
        Cell::from(col_title(SortColumn::Size, "Size")),
        Cell::from(col_title(SortColumn::Progress, "Progress")),
        Cell::from(col_title(SortColumn::Status, "Status")),
        Cell::from(col_title(SortColumn::Peers, "Peers")),
        Cell::from(col_title(SortColumn::DownSpeed, "Down")),
        Cell::from(col_title(SortColumn::UpSpeed, "Up")),
        Cell::from("ETA"),
        Cell::from(col_title(SortColumn::Ratio, "Ratio")),
    ])
    .style(
        Style::default()
            .fg(p.text_primary)
            .bg(p.header_bg)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = torrents
        .iter()
        .map(|t| {
            let is_checked = app.multi_selected.contains(&t.info_hash);
            let prefix = if is_checked { "[✓] " } else { "" };
            let name_str = format!("{}{}", prefix, t.name);
            let cat_str = if t.category.is_empty() {
                "—".to_string()
            } else {
                t.category.clone()
            };
            let size_str = format_bytes(t.size);
            let prog_bar = render_mini_progress(t.progress, 8);
            let prog_str = format!("{} {:>5.1}%", prog_bar, (t.progress * 100.0).clamp(0.0, 100.0));
            let status_color = status_color(&t.status, p);
            let down_str = format_speed(t.download_speed);
            let up_str = format_speed(t.upload_speed);

            Row::new(vec![
                Cell::from(Span::styled(
                    name_str,
                    Style::default().fg(p.text_primary),
                )),
                Cell::from(Span::styled(
                    cat_str,
                    Style::default().fg(p.text_secondary),
                )),
                Cell::from(Span::styled(
                    size_str,
                    Style::default().fg(p.text_secondary),
                )),
                Cell::from(Span::styled(prog_str, Style::default().fg(p.accent_blue))),
                Cell::from(Span::styled(
                    t.status.clone(),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    format!("{}", t.peers),
                    Style::default().fg(p.text_secondary),
                )),
                Cell::from(Span::styled(down_str, Style::default().fg(p.accent_green))),
                Cell::from(Span::styled(up_str, Style::default().fg(p.accent_blue))),
                Cell::from(Span::styled(
                    t.eta_string(),
                    Style::default().fg(p.text_secondary),
                )),
                Cell::from(Span::styled(
                    format!("{:.2}", t.ratio()),
                    Style::default().fg(p.text_muted),
                )),
            ])
        })
        .collect();

    let title = if app.multi_selected.is_empty() {
        format!(
            " Torrents ({}/{}) [s/S:Sort | Space:Select | Enter:Details] ",
            torrents.len(),
            app.all_torrents.len()
        )
    } else {
        format!(
            " Torrents ({}/{}) • {} Selected [Esc:Clear] ",
            torrents.len(),
            app.all_torrents.len(),
            app.multi_selected.len()
        )
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(if is_focused {
                    p.accent_blue
                } else {
                    p.text_secondary
                })
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(p.panel_bg));

    let widths = [
        Constraint::Min(22),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(9),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(p.selected_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    if !torrents.is_empty() {
        state.select(Some(app.selected_index.min(torrents.len() - 1)));
    }
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_details_panel(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let is_focused = app.focus == FocusPane::Details;
    let border_color = if is_focused {
        p.border_focused
    } else {
        p.border
    };

    let block = Block::default()
        .title(Span::styled(
            " Details [d:Hide | 1-4 or [/]:Tabs | j/k:Scroll] ",
            Style::default()
                .fg(if is_focused {
                    p.accent_blue
                } else {
                    p.text_secondary
                })
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(p.panel_bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(selected) = app.selected_torrent() else {
        let empty = Paragraph::new("Select a torrent to view details")
            .style(Style::default().fg(p.text_muted))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tabs row
            Constraint::Length(1), // Piece map row
            Constraint::Min(3),    // Tab content
        ])
        .split(inner);

    // 1. Tabs header
    let titles: Vec<Line> = DetailTab::all()
        .iter()
        .map(|t| Line::from(format!(" {} ", t.title())))
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.detail_tab.index())
        .style(Style::default().fg(p.text_secondary))
        .highlight_style(
            Style::default()
                .fg(p.accent_blue)
                .bg(p.selected_bg)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::default().fg(p.border)));
    f.render_widget(tabs, chunks[0]);

    // 2. Horizontal Piece Map Bar
    draw_piece_map_bar(f, app.current_detail.as_ref(), &selected, p, chunks[1]);

    // 3. Tab body
    match app.detail_tab {
        DetailTab::General => draw_general_tab(f, &selected, p, chunks[2]),
        DetailTab::Trackers => {
            draw_trackers_tab(f, app.current_detail.as_ref(), app.detail_scroll, p, chunks[2])
        }
        DetailTab::Peers => {
            draw_peers_tab(f, app.current_detail.as_ref(), app.detail_scroll, p, chunks[2])
        }
        DetailTab::Files => draw_files_tab(
            f,
            app.current_detail.as_ref(),
            &selected.name,
            app.detail_scroll,
            p,
            chunks[2],
        ),
    }
}

fn draw_piece_map_bar(
    f: &mut Frame,
    detail: Option<&TorrentDetail>,
    selected: &TorrentItem,
    p: &ThemePalette,
    area: Rect,
) {
    let label = format!(
        " Pieces: {}/{} ",
        selected.completed_pieces, selected.total_pieces
    );
    let bar_width = (area.width as usize).saturating_sub(label.len() + 2);
    if bar_width == 0 {
        return;
    }

    let mut spans = vec![Span::styled(
        label,
        Style::default()
            .fg(p.text_secondary)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(d) = detail {
        if d.info_hash == selected.info_hash {
            let buckets = d.sample_piece_buckets(bar_width);
            for ratio in buckets {
                let (ch, col) = if ratio >= 0.99 {
                    ("█", p.accent_green)
                } else if ratio >= 0.5 {
                    ("▓", p.accent_blue)
                } else if ratio > 0.0 {
                    ("▒", p.accent_amber)
                } else {
                    ("░", p.border)
                };
                spans.push(Span::styled(ch, Style::default().fg(col)));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    let filled = ((selected.progress.clamp(0.0, 1.0)) * (bar_width as f64)).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    spans.push(Span::styled(
        "█".repeat(filled),
        Style::default().fg(p.accent_green),
    ));
    spans.push(Span::styled("░".repeat(empty), Style::default().fg(p.border)));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_general_tab(f: &mut Frame, t: &TorrentItem, p: &ThemePalette, area: Rect) {
    let cat = if t.category.is_empty() {
        "None (Uncategorized)"
    } else {
        t.category.as_str()
    };
    let seq_str = if t.sequential {
        "Enabled (Sequential Order)"
    } else {
        "Disabled (Rarest-First)"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Name:        ", Style::default().fg(p.text_muted)),
            Span::styled(
                &t.name,
                Style::default()
                    .fg(p.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Info Hash: ", Style::default().fg(p.text_muted)),
            Span::styled(&t.info_hash, Style::default().fg(p.accent_purple)),
        ]),
        Line::from(vec![
            Span::styled(" Status:      ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<16}", t.status),
                Style::default()
                    .fg(status_color(&t.status, p))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ETA:        ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<14}", t.eta_string()),
                Style::default().fg(p.text_primary),
            ),
            Span::styled("Ratio:      ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:.2}", t.ratio()),
                Style::default().fg(p.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Downloaded:  ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<16}", format_bytes(t.downloaded)),
                Style::default().fg(p.text_primary),
            ),
            Span::styled("Uploaded:   ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<14}", format_bytes(t.uploaded)),
                Style::default().fg(p.text_primary),
            ),
            Span::styled("Total Size: ", Style::default().fg(p.text_muted)),
            Span::styled(format_bytes(t.size), Style::default().fg(p.text_primary)),
        ]),
        Line::from(vec![
            Span::styled(" Down Speed:  ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<16}", format_speed(t.download_speed)),
                Style::default().fg(p.accent_green),
            ),
            Span::styled("Up Speed:   ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<14}", format_speed(t.upload_speed)),
                Style::default().fg(p.accent_blue),
            ),
            Span::styled("Peers:      ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{} connected", t.peers),
                Style::default().fg(p.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Category:    ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<16}", cat),
                Style::default().fg(p.text_secondary),
            ),
            Span::styled("Storage:    ", Style::default().fg(p.text_muted)),
            Span::styled(
                format!("{:<14}", if t.storage.is_empty() { "default" } else { &t.storage }),
                Style::default().fg(p.accent_purple),
            ),
            Span::styled("Sequential: ", Style::default().fg(p.text_muted)),
            Span::styled(seq_str, Style::default().fg(p.accent_amber)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_trackers_tab(
    f: &mut Frame,
    detail: Option<&TorrentDetail>,
    scroll: usize,
    p: &ThemePalette,
    area: Rect,
) {
    let Some(detail) = detail else {
        f.render_widget(
            Paragraph::new(" Loading trackers...").style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    };

    if detail.trackers.is_empty() {
        f.render_widget(
            Paragraph::new(" No trackers configured for this torrent.")
                .style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("Tier"),
        Cell::from("Tracker URL"),
        Cell::from("Status"),
        Cell::from("Peers"),
        Cell::from("Reannounce In"),
    ])
    .style(
        Style::default()
            .fg(p.text_primary)
            .bg(p.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = detail
        .trackers
        .iter()
        .skip(scroll.min(detail.trackers.len().saturating_sub(1)))
        .map(|tr| {
            let low = tr.status.to_ascii_lowercase();
            let col = if low.contains("working") || low.contains("ok") {
                p.accent_green
            } else if low.contains("updating") {
                p.accent_blue
            } else if low.contains("error") || low.contains("fail") {
                p.accent_red
            } else {
                p.text_secondary
            };

            Row::new(vec![
                Cell::from(format!("{}", tr.tier)),
                Cell::from(Span::styled(
                    tr.url.clone(),
                    Style::default().fg(p.text_primary),
                )),
                Cell::from(Span::styled(tr.status.clone(), Style::default().fg(col))),
                Cell::from(format!("{}", tr.peers)),
                Cell::from(tr.reannounce_in_string()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Min(32),
        Constraint::Length(28),
        Constraint::Length(7),
        Constraint::Length(14),
    ];

    f.render_widget(Table::new(rows, widths).header(header), area);
}

fn draw_peers_tab(
    f: &mut Frame,
    detail: Option<&TorrentDetail>,
    scroll: usize,
    p: &ThemePalette,
    area: Rect,
) {
    let Some(detail) = detail else {
        f.render_widget(
            Paragraph::new(" Loading peers...").style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    };

    if detail.peer_list.is_empty() {
        f.render_widget(
            Paragraph::new(" No peers currently connected.")
                .style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    }

    let mut peers = detail.peer_list.clone();
    peers.sort_by(|a, b| {
        b.download_speed
            .partial_cmp(&a.download_speed)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let header = Row::new(vec![
        Cell::from("IP:Port"),
        Cell::from("Client"),
        Cell::from("State"),
        Cell::from("Progress"),
        Cell::from("Down Speed"),
        Cell::from("Up Speed"),
        Cell::from("Downloaded"),
        Cell::from("Uploaded"),
        Cell::from("Pieces"),
        Cell::from("Duration"),
    ])
    .style(
        Style::default()
            .fg(p.text_primary)
            .bg(p.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = peers
        .iter()
        .skip(scroll.min(peers.len().saturating_sub(1)))
        .map(|peer| {
            let st_low = peer.state.to_ascii_lowercase();
            let st_col = match st_low.as_str() {
                "downloading" => p.accent_green,
                "connected" => p.accent_blue,
                "choked" | "connecting" => p.accent_amber,
                _ => p.text_secondary,
            };
            let client = if peer.client_id.is_empty() {
                "Unknown".to_string()
            } else {
                peer.client_id.clone()
            };

            Row::new(vec![
                Cell::from(Span::styled(
                    peer.endpoint(),
                    Style::default().fg(p.text_primary),
                )),
                Cell::from(Span::styled(client, Style::default().fg(p.text_secondary))),
                Cell::from(Span::styled(
                    peer.state.clone(),
                    Style::default().fg(st_col).add_modifier(Modifier::BOLD),
                )),
                Cell::from(format!("{:.1}%", peer.progress)),
                Cell::from(Span::styled(
                    format_speed(peer.download_speed),
                    Style::default().fg(p.accent_green),
                )),
                Cell::from(Span::styled(
                    format_speed(peer.upload_speed),
                    Style::default().fg(p.accent_blue),
                )),
                Cell::from(format_bytes(peer.downloaded_bytes)),
                Cell::from(format_bytes(peer.uploaded_bytes)),
                Cell::from(format!("{}", peer.pieces_received)),
                Cell::from(format_duration(peer.connected_duration_secs)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(21),
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Length(9),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(7),
        Constraint::Length(9),
    ];

    f.render_widget(Table::new(rows, widths).header(header), area);
}

fn draw_files_tab(
    f: &mut Frame,
    detail: Option<&TorrentDetail>,
    torrent_name: &str,
    scroll: usize,
    p: &ThemePalette,
    area: Rect,
) {
    let Some(detail) = detail else {
        f.render_widget(
            Paragraph::new(" Loading files...").style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    };

    if detail.files.is_empty() {
        f.render_widget(
            Paragraph::new(" No files reported for this torrent.")
                .style(Style::default().fg(p.text_muted)),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("File Path"),
        Cell::from("Size"),
        Cell::from("Progress"),
        Cell::from("Priority"),
    ])
    .style(
        Style::default()
            .fg(p.text_primary)
            .bg(p.header_bg)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = detail
        .files
        .iter()
        .skip(scroll.min(detail.files.len().saturating_sub(1)))
        .map(|file| {
            let rel = sanitize_relative_path(&file.path, torrent_name);
            let bar = render_mini_progress(file.progress, 8);
            let prog = format!("{} {:>5.1}%", bar, (file.progress * 100.0).clamp(0.0, 100.0));
            let prio = if file.priority.is_empty() {
                "Normal".to_string()
            } else {
                file.priority.clone()
            };

            Row::new(vec![
                Cell::from(Span::styled(rel, Style::default().fg(p.text_primary))),
                Cell::from(format_bytes(file.size)),
                Cell::from(Span::styled(prog, Style::default().fg(p.accent_blue))),
                Cell::from(Span::styled(prio, Style::default().fg(p.text_secondary))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(35),
        Constraint::Length(12),
        Constraint::Length(18),
        Constraint::Length(10),
    ];

    f.render_widget(Table::new(rows, widths).header(header), area);
}

fn draw_status_bar(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let (conn_label, conn_col) = match app.connection_state {
        ConnectionState::Connected => ("● Connected", p.accent_green),
        ConnectionState::Connecting => ("● Connecting...", p.accent_amber),
        ConnectionState::Disconnected => ("● Disconnected", p.accent_red),
    };

    let total_down: f64 = app.all_torrents.iter().map(|t| t.download_speed).sum();
    let total_up: f64 = app.all_torrents.iter().map(|t| t.upload_speed).sum();

    let mem_str = if let Some(ref sys) = app.system_stats {
        format!(
            " | Daemon RAM: {} | Sys RAM: {} / {} ",
            format_bytes(sys.process_rss_bytes),
            format_bytes(sys.system_used_bytes),
            format_bytes(sys.system_total_bytes)
        )
    } else {
        String::new()
    };

    let right_msg = if let Some(toast) = app.active_toast() {
        Span::styled(
            format!(" ★ {} ", toast),
            Style::default()
                .fg(p.accent_amber)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " [Tab]Switch Pane  [q]Quit TUI  [Q]Shutdown Daemon ",
            Style::default().fg(p.text_muted),
        )
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ({}) ", conn_label, app.api.base_url),
            Style::default().fg(conn_col).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(p.border)),
        Span::styled(
            format!("↓ {} ", format_speed(total_down)),
            Style::default()
                .fg(p.accent_green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("↑ {} ", format_speed(total_up)),
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(mem_str, Style::default().fg(p.text_secondary)),
        Span::styled("│", Style::default().fg(p.border)),
        right_msg,
    ]);

    let bar = Paragraph::new(line).style(Style::default().bg(p.header_bg));
    f.render_widget(bar, area);
}

fn draw_login_screen(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let popup = centered_rect(56, 17, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " 🔑 HydroxyTorrent Remote Login ",
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.border_focused))
        .style(Style::default().bg(p.panel_bg));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let field_style = |field: LoginField| -> Style {
        if app.login_field == field {
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.text_secondary)
        }
    };

    let masked_pass = if app.login_show_pass {
        app.login_pass.clone()
    } else {
        "•".repeat(app.login_pass.chars().count())
    };

    let cursor_for = |field: LoginField| -> &'static str {
        if app.login_field == field { "█" } else { "" }
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Server URL: ", field_style(LoginField::ServerUrl)),
            Span::styled(
                format!("{}{}", app.login_url, cursor_for(LoginField::ServerUrl)),
                Style::default().fg(p.text_primary),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Username:   ", field_style(LoginField::Username)),
            Span::styled(
                format!("{}{}", app.login_user, cursor_for(LoginField::Username)),
                Style::default().fg(p.text_primary),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Password:   ", field_style(LoginField::Password)),
            Span::styled(
                format!("{}{}", masked_pass, cursor_for(LoginField::Password)),
                Style::default().fg(p.text_primary),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(ref err) = app.login_error {
        lines.push(Line::from(Span::styled(
            format!("  ⚠ {}", err),
            Style::default()
                .fg(p.accent_red)
                .add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Enter your HydroxyTorrent RPC credentials.",
            Style::default().fg(p.text_muted),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [Tab/↑↓]", Style::default().fg(p.accent_blue)),
        Span::styled(" Switch Field  ", Style::default().fg(p.text_secondary)),
        Span::styled("[F2]", Style::default().fg(p.accent_amber)),
        Span::styled(" Show Pass  ", Style::default().fg(p.text_secondary)),
        Span::styled("[Enter]", Style::default().fg(p.accent_green)),
        Span::styled(" Login  ", Style::default().fg(p.text_secondary)),
        Span::styled("[Esc]", Style::default().fg(p.accent_red)),
        Span::styled(" Quit", Style::default().fg(p.text_secondary)),
    ]));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_search_modal(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let popup = centered_rect(54, 5, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " 🔍 Search / Filter Torrents [Enter/Esc to close] ",
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.border_focused))
        .style(Style::default().bg(p.panel_bg));

    let text = Line::from(vec![
        Span::styled(" Query: ", Style::default().fg(p.text_muted)),
        Span::styled(
            format!("{}█", app.search_query),
            Style::default()
                .fg(p.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(text).block(block), popup);
}

fn draw_remove_modal(
    f: &mut Frame,
    p: &ThemePalette,
    area: Rect,
    delete_files: bool,
    targets: &[(String, String)],
) {
    let popup = centered_rect(62, 11, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " 🗑 Remove Torrent(s) ",
            Style::default()
                .fg(p.accent_red)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.accent_red))
        .style(Style::default().bg(p.panel_bg));

    let target_desc = if targets.len() == 1 {
        format!("\"{}\"", targets[0].1)
    } else {
        format!("{} selected torrents", targets.len())
    };

    let checkbox = if delete_files { "[✓]" } else { "[ ]" };
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Remove ", Style::default().fg(p.text_secondary)),
            Span::styled(
                target_desc,
                Style::default()
                    .fg(p.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("?", Style::default().fg(p.text_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} Also delete downloaded files from disk", checkbox),
                Style::default()
                    .fg(if delete_files {
                        p.accent_red
                    } else {
                        p.accent_amber
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Space/Tab]", Style::default().fg(p.accent_blue)),
            Span::styled(" Toggle Delete Files   ", Style::default().fg(p.text_secondary)),
            Span::styled("[Enter/y]", Style::default().fg(p.accent_red)),
            Span::styled(" Confirm   ", Style::default().fg(p.text_secondary)),
            Span::styled("[Esc/n]", Style::default().fg(p.accent_green)),
            Span::styled(" Cancel", Style::default().fg(p.text_secondary)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_shutdown_modal(f: &mut Frame, p: &ThemePalette, area: Rect) {
    let popup = centered_rect(54, 8, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " ⏻ Shutdown HydroxyTorrent Daemon ",
            Style::default()
                .fg(p.accent_red)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.accent_red))
        .style(Style::default().bg(p.panel_bg));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Are you sure you want to stop the background daemon?",
            Style::default().fg(p.text_primary),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter/y]", Style::default().fg(p.accent_red)),
            Span::styled(" Shutdown Daemon   ", Style::default().fg(p.text_secondary)),
            Span::styled("[Esc/n]", Style::default().fg(p.accent_green)),
            Span::styled(" Cancel", Style::default().fg(p.text_secondary)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_logs_modal(f: &mut Frame, app: &App, p: &ThemePalette, area: Rect) {
    let popup = centered_rect(
        area.width.saturating_sub(6).max(40),
        area.height.saturating_sub(4).max(12),
        area,
    );
    f.render_widget(Clear, popup);

    let filtered = app.filtered_logs();
    let title = format!(
        " 📜 Daemon Logs ({} lines) | [f]Level: {} | [/]Search: \"{}\" | [a]AutoScroll: {} | [Esc]Close ",
        filtered.len(),
        app.log_level_filter.label(),
        if app.log_search_editing {
            format!("{}█", app.log_search_query)
        } else {
            app.log_search_query.clone()
        },
        if app.log_auto_scroll { "ON" } else { "OFF" }
    );

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.border_focused))
        .style(Style::default().bg(p.panel_bg));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let visible_height = inner.height as usize;
    let start = if app.log_auto_scroll {
        filtered.len().saturating_sub(visible_height)
    } else {
        app.log_scroll
            .min(filtered.len().saturating_sub(visible_height))
    };

    let lines: Vec<Line> = filtered
        .iter()
        .skip(start)
        .take(visible_height)
        .map(|l| {
            let level_col = match l.level {
                LogLevel::Error => p.accent_red,
                LogLevel::Warn => p.accent_amber,
                LogLevel::Info => p.accent_green,
                LogLevel::Debug => p.accent_blue,
                LogLevel::Trace => p.accent_purple,
                LogLevel::All | LogLevel::Normal => p.text_secondary,
            };
            Line::from(vec![
                Span::styled(
                    format!("{:>4} │ ", l.number),
                    Style::default().fg(p.text_muted),
                ),
                Span::styled(&l.text, Style::default().fg(level_col)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_help_modal(f: &mut Frame, p: &ThemePalette, area: Rect) {
    let popup = centered_rect(68, 22, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " ⌨ HydroxyTorrent RC TUI — Keyboard Shortcuts [Esc/q to close] ",
            Style::default()
                .fg(p.accent_blue)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(p.border_focused))
        .style(Style::default().bg(p.panel_bg));

    let row = |k: &'static str, desc: &'static str| -> Line {
        Line::from(vec![
            Span::styled(
                format!("  {:<16}", k),
                Style::default()
                    .fg(p.accent_amber)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(p.text_primary)),
        ])
    };

    let lines = vec![
        Line::from(""),
        row("j / k or ↑ / ↓", "Move selection up / down (or scroll active pane)"),
        row("g / G", "Jump to first / last item"),
        row("Tab / Shift+Tab", "Cycle focus between Sidebar, Torrents, and Details"),
        row("Space / Ctrl+A", "Toggle multi-select on torrent / Select all"),
        row("Enter / d", "Toggle bottom Details panel"),
        row("b", "Toggle left Categories & Status sidebar"),
        row("1 / 2 / 3 / 4", "Switch Details tab (General, Trackers, Peers, Content)"),
        row("[ / ]", "Previous / Next Details tab"),
        row("r / p", "Resume / Pause selected torrent(s)"),
        row("c / a", "Force Recheck / Force Reannounce selected torrent(s)"),
        row("o", "Toggle Sequential vs Rarest-First download mode"),
        row("x / Delete", "Remove selected torrent(s) (with delete-files option)"),
        row("s / S", "Cycle sort column / Invert sort order (▲/▼)"),
        row("/", "Search / Filter torrents by name, category, or hash"),
        row("l", "Open live Daemon Log Viewer modal"),
        row("t", "Cycle color theme (Dark, Dracula, Nord, Matrix, Light)"),
        row("O / Q / q", "Logout / Shutdown daemon / Quit TUI"),
    ];

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn status_color(status: &str, p: &ThemePalette) -> Color {
    match status.to_ascii_lowercase().as_str() {
        "downloading" => p.accent_green,
        "seeding" | "completed" => p.accent_blue,
        "paused" | "stopped" | "stalled" | "queued" | "checking" => p.accent_amber,
        s if s.contains("error") => p.accent_red,
        _ => p.text_secondary,
    }
}

fn render_mini_progress(progress: f64, width: usize) -> String {
    let clamped = progress.clamp(0.0, 1.0);
    let filled = (clamped * (width as f64)).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "━".repeat(filled), "─".repeat(empty))
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}
