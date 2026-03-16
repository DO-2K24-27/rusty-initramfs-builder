use crate::tui::app::{App, Screen, WizardMode};
use crate::tui::screens;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn render_app(frame: &mut Frame, app: &App) {
    let root_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let mode_label = match app.mode {
        WizardMode::Quick => "Quick mode",
        WizardMode::Advanced => "Advanced mode",
    };
    let header_title = format!(" initramfs-builder interactive [{}] ", mode_label);
    let header_widget = Paragraph::new(header_title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header_widget, root_chunks[0]);

    let body_area = root_chunks[1];
    match app.screen {
        Screen::Language => screens::language::draw(frame, body_area, &app.language_screen),
        Screen::Image => screens::image::draw(frame, body_area, &app.image_screen),
        Screen::Architecture => screens::arch::draw(frame, body_area, &app.arch_screen),
        Screen::Inject => screens::inject::draw(frame, body_area, &app.inject_screen),
        Screen::Init => screens::init::draw(frame, body_area, &app.init_screen),
        Screen::Compression => screens::compress::draw(frame, body_area, &app.compress_screen),
        Screen::Summary => screens::summary::draw(frame, body_area, app),
        Screen::Build => render_build_status(frame, body_area, app),
    }

    let help_bar_text = match app.screen {
        Screen::Language => " ↑↓ Select | ←→ Version | Enter Next | Esc Quit ",
        Screen::Image => " Type image ref | Enter Next | Esc Back ",
        Screen::Architecture => " ↑↓ Select | Enter Next | Esc Back ",
        Screen::Inject => {
            " a Add | d Delete | ↑↓ Select | Tab Switch field | Enter Next | Esc Back "
        }
        Screen::Init => " ↑↓ Select | Enter Next | Esc Back ",
        Screen::Compression => " ↑↓ Select | Enter Next | Esc Back ",
        Screen::Summary => " Enter Build | a Advanced options | Esc Back | q Quit ",
        Screen::Build => {
            if app.build_success || app.build_error.is_some() {
                " Enter/q Quit "
            } else {
                " Building... please wait "
            }
        }
    };
    let help_bar = Paragraph::new(help_bar_text)
        .style(Style::default().fg(Color::Black).bg(Color::White))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(help_bar, root_chunks[2]);
}

fn render_build_status(frame: &mut Frame, area: Rect, app: &App) {
    let status_text = if let Some(error) = &app.build_error {
        format!("Build Failed:\n{}", error)
    } else if let Some(progress_message) = &app.build_progress {
        if app.build_success {
            progress_message.clone()
        } else {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let spinner_char = spinner[app.loading_frame % spinner.len()];
            format!("{} {}", spinner_char, progress_message)
        }
    } else {
        "Initializing build...".to_string()
    };

    let status_color = if app.build_error.is_some() {
        Color::Red
    } else if app.build_success {
        Color::Green
    } else {
        Color::Yellow
    };

    let status_panel = Paragraph::new(status_text)
        .style(Style::default().fg(status_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Build Status "),
        );
    frame.render_widget(status_panel, area);
}
