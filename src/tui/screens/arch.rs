use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct ArchScreen {
    pub selected: usize,
    pub options: [&'static str; 2],
}

impl ArchScreen {
    pub fn new_with_default(arch: &str) -> Self {
        let selected = match arch {
            "arm64" => 1,
            _ => 0,
        };
        Self {
            selected,
            options: ["amd64", "arm64"],
        }
    }

    pub fn sync_from_config(&mut self, arch: &str) {
        self.selected = match arch {
            "arm64" => 1,
            _ => 0,
        };
    }

    pub fn toggle(&mut self) {
        self.selected = 1 - self.selected;
    }

    pub fn get_selected(&self) -> &'static str {
        self.options[self.selected]
    }
}

pub fn draw(frame: &mut Frame, area: Rect, screen: &ArchScreen) {
    let items: Vec<ListItem> = screen
        .options
        .iter()
        .enumerate()
        .map(|(i, arch)| {
            let prefix = if i == screen.selected { "● " } else { "○ " };
            let style = if i == screen.selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}{}", prefix, arch)).style(style)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(screen.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Target Architecture "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(list, area, &mut state);
}
