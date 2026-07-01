use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tui_input::Input;
use vc_types::AgentEvent;

enum Kind {
    User,
    Assistant,
    ToolRunning,
    ToolDone,
    ToolFailed,
    Error,
    Info,
}

struct Entry {
    kind: Kind,
    text: String,
}

impl Entry {
    fn to_lines(&self) -> Vec<Line<'static>> {
        match self.kind {
            Kind::User => vec![Line::from(vec![
                Span::styled(
                    "› ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(self.text.clone(), Style::default().fg(Color::Cyan)),
            ])],
            Kind::Assistant => self
                .text
                .split('\n')
                .map(|line| Line::from(line.to_string()))
                .collect(),
            Kind::ToolRunning => vec![Line::from(Span::styled(
                format!("⚙ {} …", self.text),
                Style::default().fg(Color::Yellow),
            ))],
            Kind::ToolDone => vec![Line::from(Span::styled(
                format!("✓ {}", self.text),
                Style::default().fg(Color::Green),
            ))],
            Kind::ToolFailed => vec![Line::from(Span::styled(
                format!("✗ {}", self.text),
                Style::default().fg(Color::Red),
            ))],
            Kind::Error => vec![Line::from(Span::styled(
                format!("! {}", self.text),
                Style::default().fg(Color::Red),
            ))],
            Kind::Info => vec![Line::from(Span::styled(
                self.text.clone(),
                Style::default().fg(Color::DarkGray),
            ))],
        }
    }
}

pub struct App {
    pub input: Input,
    entries: Vec<Entry>,
    tools: HashMap<String, usize>,
    assistant: Option<usize>,
    pub thinking: bool,
    scroll: u16,
    follow: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: Input::default(),
            entries: Vec::new(),
            tools: HashMap::new(),
            assistant: None,
            thinking: false,
            scroll: 0,
            follow: true,
        }
    }

    pub fn push_user(&mut self, text: &str) {
        self.entries.push(Entry {
            kind: Kind::User,
            text: text.to_string(),
        });
        self.assistant = None;
        self.thinking = true;
        self.follow = true;
    }

    pub fn apply(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Token(text) => match self.assistant {
                Some(index) => self.entries[index].text.push_str(&text),
                None => {
                    self.entries.push(Entry {
                        kind: Kind::Assistant,
                        text,
                    });
                    self.assistant = Some(self.entries.len() - 1);
                }
            },
            AgentEvent::ToolStarted { id, label } => {
                self.assistant = None;
                self.entries.push(Entry {
                    kind: Kind::ToolRunning,
                    text: label,
                });
                self.tools.insert(id, self.entries.len() - 1);
            }
            AgentEvent::ToolFinished { id, ok } => {
                if let Some(&index) = self.tools.get(&id) {
                    self.entries[index].kind = if ok { Kind::ToolDone } else { Kind::ToolFailed };
                }
            }
            AgentEvent::StepComplete => self.assistant = None,
            AgentEvent::Error(message) => {
                self.assistant = None;
                self.entries.push(Entry {
                    kind: Kind::Error,
                    text: message,
                });
            }
            AgentEvent::Info(message) => self.entries.push(Entry {
                kind: Kind::Info,
                text: message,
            }),
            AgentEvent::ModelSelected(model) => self.entries.push(Entry {
                kind: Kind::Info,
                text: format!("model: {model}"),
            }),
            AgentEvent::Done => {
                self.thinking = false;
                self.assistant = None;
                self.follow = true;
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let areas =
            Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(frame.area());
        let convo_area = areas[0];
        let input_area = areas[1];

        let lines: Vec<Line> = self.entries.iter().flat_map(Entry::to_lines).collect();

        let width = convo_area.width.saturating_sub(2).max(1);
        let height = convo_area.height.saturating_sub(2);
        let total: u16 = lines.iter().map(|line| wrapped_rows(line, width)).sum();
        let max_scroll = total.saturating_sub(height);

        if self.follow {
            self.scroll = max_scroll;
        } else if self.scroll >= max_scroll {
            self.scroll = max_scroll;
            self.follow = true;
        }

        let title = if self.thinking {
            " codelight · thinking… "
        } else {
            " codelight "
        };

        let convo = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(convo, convo_area);

        let prompt = "› ";
        let shown = format!("{prompt}{}", self.input.value());
        let input =
            Paragraph::new(shown).block(Block::default().borders(Borders::ALL).title(" message "));
        frame.render_widget(input, input_area);

        let cursor_x =
            input_area.x + 1 + prompt.chars().count() as u16 + self.input.visual_cursor() as u16;
        let cursor_y = input_area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn wrapped_rows(line: &Line, width: u16) -> u16 {
    let content = line.width() as u16;
    if width == 0 {
        return 1;
    }
    content.div_ceil(width).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn buffer_text(buffer: &Buffer) -> String {
        let area = buffer.area();
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buffer.cell((x, y)) {
                    text.push_str(cell.symbol());
                }
            }
        }
        text
    }

    fn draw(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn renders_conversation_and_tool_lifecycle() {
        let mut app = App::new();
        app.push_user("read package.json");
        app.apply(AgentEvent::ToolStarted {
            id: "call_1".to_string(),
            label: "read_file".to_string(),
        });
        app.apply(AgentEvent::ToolFinished {
            id: "call_1".to_string(),
            ok: true,
        });
        app.apply(AgentEvent::Token("Next.js 15".to_string()));
        app.apply(AgentEvent::Done);

        let text = draw(&mut app);
        assert!(text.contains("read package.json"));
        assert!(text.contains("read_file"));
        assert!(text.contains("Next.js 15"));
        assert!(text.contains("codelight"));
        assert!(!app.thinking);
    }

    #[test]
    fn a_failed_tool_is_marked() {
        let mut app = App::new();
        app.apply(AgentEvent::ToolStarted {
            id: "call_9".to_string(),
            label: "read_file".to_string(),
        });
        app.apply(AgentEvent::ToolFinished {
            id: "call_9".to_string(),
            ok: false,
        });

        let text = draw(&mut app);
        assert!(text.contains("✗"));
    }
}
