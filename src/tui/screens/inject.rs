use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

#[derive(Debug, Clone)]
pub struct Injection {
    pub src: String,
    pub dest: String,
}

pub struct InjectScreen {
    pub items: Vec<(String, String)>,
    pub selected: usize,
    pub editing: bool,
    pub edit_field: usize,
    pub src_input: String,
    pub dest_input: String,
}

impl InjectScreen {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            editing: false,
            edit_field: 0,
            src_input: String::new(),
            dest_input: String::new(),
        }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.items.is_empty() {
            if self.selected > 0 {
                self.selected -= 1;
            } else {
                self.selected = self.items.len() - 1;
            }
        }
    }

    pub fn start_add(&mut self) {
        self.editing = true;
        self.edit_field = 0;
        self.src_input.clear();
        self.dest_input.clear();
    }

    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.src_input.clear();
        self.dest_input.clear();
    }

    pub fn confirm_edit(&mut self) {
        if !self.src_input.is_empty() && !self.dest_input.is_empty() {
            self.items
                .push((self.src_input.clone(), self.dest_input.clone()));
        }
        self.editing = false;
        self.src_input.clear();
        self.dest_input.clear();
    }

    pub fn delete_selected(&mut self) {
        if !self.items.is_empty() {
            self.items.remove(self.selected);
            if self.selected > 0 && self.selected >= self.items.len() {
                self.selected = self.items.len() - 1;
            }
        }
    }

    pub fn toggle_field(&mut self) {
        self.edit_field = 1 - self.edit_field;
    }

    pub fn input_char(&mut self, c: char) {
        if self.edit_field == 0 {
            self.src_input.push(c);
        } else {
            self.dest_input.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.edit_field == 0 {
            self.src_input.pop();
        } else {
            self.dest_input.pop();
        }
    }

    pub fn get_injections(&self) -> Vec<Injection> {
        self.items
            .iter()
            .map(|(src, dest)| Injection {
                src: src.clone(),
                dest: dest.clone(),
            })
            .collect()
    }
}

pub fn draw(frame: &mut Frame, area: Rect, screen: &InjectScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    if screen.items.is_empty() && !screen.editing {
        let empty = Paragraph::new(" No injections configured. Press 'a' to add.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Injections "));
        frame.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = screen
            .items
            .iter()
            .enumerate()
            .map(|(i, (src, dest))| {
                let style = if i == screen.selected && !screen.editing {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default()
                };
                ListItem::new(format!("  {} → {}", src, dest)).style(style)
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(screen.selected));

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Injections "))
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[0], &mut state);
    }

    if screen.editing {
        let edit_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let src_style = if screen.edit_field == 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let dest_style = if screen.edit_field == 1 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let src = Paragraph::new(format!("{}_", screen.src_input))
            .style(src_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Source (local) "),
            );
        let dest = Paragraph::new(format!("{}_", screen.dest_input))
            .style(dest_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Destination (in initramfs) "),
            );

        frame.render_widget(src, edit_chunks[0]);
        frame.render_widget(dest, edit_chunks[1]);
    } else {
        let help = Paragraph::new(" 'a' Add  'd' Delete  Enter Continue")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, chunks[1]);
    }
}
