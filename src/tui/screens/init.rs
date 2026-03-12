use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum InitMode {
    Default,
    CustomFile(PathBuf),
}

pub struct InitScreen {
    pub selected: usize,
    pub path_input: String,
}

impl InitScreen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            path_input: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.selected = 1 - self.selected;
    }

    pub fn get_init_mode(&self) -> InitMode {
        if self.selected == 0 {
            InitMode::Default
        } else {
            InitMode::CustomFile(PathBuf::from(&self.path_input))
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, screen: &InitScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    let options = [
        ("Default", "Auto-generate minimal /init script"),
        ("Custom file", "Load init script from local file"),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let prefix = if i == screen.selected { "● " } else { "○ " };
            let style = if i == screen.selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {}{}", prefix, name), style),
                Span::styled(format!(" - {}", desc), Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(screen.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Init Script "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(list, chunks[0], &mut state);

    if screen.selected == 1 {
        let input = Paragraph::new(format!("{}_", screen.path_input))
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(" File path "));
        frame.render_widget(input, chunks[1]);
    }
}
