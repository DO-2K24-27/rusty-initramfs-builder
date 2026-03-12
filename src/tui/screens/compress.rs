use initramfs_builder::Compression;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub struct CompressScreen {
    pub selected: usize,
    pub options: [(Compression, &'static str); 3],
}

impl CompressScreen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            options: [
                (Compression::Gzip, "gzip - Default, widely compatible"),
                (Compression::Zstd, "zstd - Better compression, faster"),
                (Compression::None, "none - No compression"),
            ],
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.options.len();
    }

    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.options.len() - 1;
        }
    }

    pub fn get_selected(&self) -> Compression {
        self.options[self.selected].0
    }
}

pub fn draw(frame: &mut Frame, area: Rect, screen: &CompressScreen) {
    let items: Vec<ListItem> = screen
        .options
        .iter()
        .enumerate()
        .map(|(i, (_, desc))| {
            let prefix = if i == screen.selected { "● " } else { "○ " };
            let style = if i == screen.selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}{}", prefix, desc)).style(style)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(screen.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Compression "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(list, area, &mut state);
}
