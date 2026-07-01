use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use tui_input::Input;
use vc_types::AgentEvent;

const ACCENT: Color = Color::Rgb(0x6c, 0x7c, 0xff);
const OK: Color = Color::Rgb(0x5b, 0xd6, 0xa4);
const WARN: Color = Color::Rgb(0xe0, 0xb3, 0x41);
const ERR: Color = Color::Rgb(0xf0, 0x61, 0x6d);
const FG: Color = Color::Rgb(0xe9, 0xe9, 0xee);
const MUTED: Color = Color::Rgb(0x76, 0x7a, 0x86);
const FAINT: Color = Color::Rgb(0x3c, 0x41, 0x4e);
const URL: Color = Color::Rgb(0x93, 0xa7, 0xff);
const CODE: Color = Color::Rgb(0xa9, 0xb6, 0xff);
const LINE: Color = Color::Rgb(0x2b, 0x30, 0x3d);

const RAIL_WIDTH: u16 = 34;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
            Kind::User => vec![
                Line::from(Span::styled("you", Style::default().fg(MUTED))),
                Line::from(Span::styled(self.text.clone(), Style::default().fg(FG))),
                Line::default(),
            ],
            Kind::Assistant => {
                let mut lines = vec![Line::from(Span::styled(
                    "codelight",
                    Style::default().fg(MUTED),
                ))];
                lines.extend(render_markdown(&self.text));
                lines.push(Line::default());
                lines
            }
            Kind::ToolRunning => vec![tool_line(&self.text, "●", WARN)],
            Kind::ToolDone => vec![tool_line(&self.text, "✓", OK)],
            Kind::ToolFailed => vec![tool_line(&self.text, "✗", ERR)],
            Kind::Error => vec![
                Line::from(Span::styled(
                    format!("! {}", self.text),
                    Style::default().fg(ERR),
                )),
                Line::default(),
            ],
            Kind::Info => vec![Line::from(Span::styled(
                self.text.clone(),
                Style::default().fg(FAINT),
            ))],
        }
    }
}

fn tool_line(label: &str, mark: &str, mark_color: Color) -> Line<'static> {
    let (verb, target) = label.split_once(' ').unwrap_or((label, ""));
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<10}", verb.to_uppercase()),
            Style::default().fg(MUTED),
        ),
        Span::styled(target.to_string(), Style::default().fg(FG)),
        Span::raw("  "),
        Span::styled(mark.to_string(), Style::default().fg(mark_color)),
    ])
}

pub enum Preview {
    Idle,
    Ready { url: String, secs: String },
}

pub struct App {
    pub input: Input,
    project: String,
    model: String,
    entries: Vec<Entry>,
    tools: HashMap<String, usize>,
    assistant: Option<usize>,
    preview: Preview,
    steps: u32,
    pub thinking: bool,
    scroll: u16,
    follow: bool,
    tick: usize,
}

impl App {
    pub fn new(project: String, model: String) -> Self {
        Self {
            input: Input::default(),
            project,
            model,
            entries: Vec::new(),
            tools: HashMap::new(),
            assistant: None,
            preview: Preview::Idle,
            steps: 0,
            thinking: false,
            scroll: 0,
            follow: true,
            tick: 0,
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
            AgentEvent::StepComplete => {
                self.assistant = None;
                self.steps += 1;
            }
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
            AgentEvent::ModelSelected(model) => self.model = model,
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

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn seed_demo(&mut self) {
        self.push_user("ship the dashboard spinner to a preview");
        self.entries.push(Entry {
            kind: Kind::Assistant,
            text: "Typechecking and deploying a preview - progress is in the rail.".to_string(),
        });
        self.entries.push(Entry {
            kind: Kind::ToolDone,
            text: "write app/dashboard/loading.tsx".to_string(),
        });
        self.entries.push(Entry {
            kind: Kind::ToolDone,
            text: "typecheck tsc --noEmit".to_string(),
        });
        self.entries.push(Entry {
            kind: Kind::ToolDone,
            text: "deploy preview".to_string(),
        });
        self.entries.push(Entry {
            kind: Kind::Assistant,
            text: "Live. The link is in the rail.".to_string(),
        });
        self.preview = Preview::Ready {
            url: "my-app-git-spinner.vercel.app".to_string(),
            secs: "8.2s".to_string(),
        };
        self.steps = 4;
        self.thinking = false;
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let root = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

        self.render_header(frame, root[0]);

        let body = Layout::horizontal([Constraint::Min(20), Constraint::Length(RAIL_WIDTH)])
            .split(root[1]);
        self.render_conversation(frame, body[0]);
        self.render_rail(frame, body[1]);

        self.render_input(frame, root[2]);
        self.render_status(frame, root[3]);
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let cols = Layout::horizontal([Constraint::Min(10), Constraint::Length(40)]).split(area);
        let wordmark = Line::from(vec![
            Span::styled("code", Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled(
                "light",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(wordmark).block(Block::default().padding(Padding::new(1, 0, 0, 0))),
            cols[0],
        );
        let ctx = Line::from(Span::styled(
            format!("{} · {}", self.project, self.model),
            Style::default().fg(MUTED),
        ));
        frame.render_widget(
            Paragraph::new(ctx)
                .alignment(Alignment::Right)
                .block(Block::default().padding(Padding::new(0, 1, 0, 0))),
            cols[1],
        );
    }

    fn render_conversation(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines: Vec<Line> = self.entries.iter().flat_map(Entry::to_lines).collect();

        let width = area.width.saturating_sub(2).max(1);
        let height = area.height;
        let total: u16 = lines.iter().map(|line| wrapped_rows(line, width)).sum();
        let max_scroll = total.saturating_sub(height);
        if self.follow {
            self.scroll = max_scroll;
        } else if self.scroll >= max_scroll {
            self.scroll = max_scroll;
            self.follow = true;
        }

        let convo = Paragraph::new(lines)
            .block(Block::default().padding(Padding::new(1, 1, 0, 0)))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(convo, area);
    }

    fn render_rail(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines = match &self.preview {
            Preview::Idle => self.rail_idle(),
            Preview::Ready { url, secs } => self.rail_ready(url, secs),
        };
        let rail = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(Color::Rgb(0x19, 0x1c, 0x24)))
                .padding(Padding::new(2, 1, 0, 0)),
        );
        frame.render_widget(rail, area);
    }

    fn rail_idle(&self) -> Vec<Line<'static>> {
        vec![
            label("preview"),
            Line::from(Span::styled("○ no preview yet", Style::default().fg(MUTED))),
            Line::from(Span::styled(
                "deploy to see it live",
                Style::default().fg(FAINT),
            )),
            Line::default(),
            label("project"),
            Line::from(Span::styled(self.project.clone(), Style::default().fg(FG))),
            Line::default(),
            label("session"),
            Line::from(Span::styled(
                format!("step {}/20", self.steps),
                Style::default().fg(MUTED),
            )),
        ]
    }

    fn rail_ready(&self, url: &str, secs: &str) -> Vec<Line<'static>> {
        vec![
            Line::from(vec![
                Span::styled("DEPLOYMENT", Style::default().fg(MUTED)),
                Span::styled(format!("  ready · {secs}"), Style::default().fg(OK)),
            ]),
            step_done("install"),
            step_done("compile"),
            step_done("static"),
            step_done("ready"),
            Line::default(),
            label("preview"),
            Line::from(Span::styled("● live", Style::default().fg(OK))),
            Line::from(Span::styled(
                url.to_string(),
                Style::default().fg(URL).add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(Span::styled(
                "open · copy · logs",
                Style::default().fg(ACCENT),
            )),
        ]
    }

    fn render_input(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let value = self.input.value();
        let mut spans = vec![Span::styled("› ", Style::default().fg(ACCENT))];
        if value.is_empty() {
            spans.push(Span::styled("type a message…", Style::default().fg(FAINT)));
        } else {
            spans.push(Span::styled(value.to_string(), Style::default().fg(FG)));
        }
        let border = if self.thinking { ACCENT } else { LINE };
        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border))
                    .padding(Padding::horizontal(1)),
            ),
            area,
        );
        let cursor_x = area.x + 4 + self.input.visual_cursor() as u16;
        frame.set_cursor_position((cursor_x, area.y + 1));
    }

    fn render_status(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let cols = Layout::horizontal([Constraint::Min(10), Constraint::Length(26)]).split(area);

        let (glyph, word, color) = if self.thinking {
            (SPINNER[self.tick % SPINNER.len()], "working", WARN)
        } else {
            ("●", "ready", OK)
        };
        let left = Line::from(vec![
            Span::styled(glyph, Style::default().fg(color)),
            Span::styled(
                format!(" {word} · step {}/20 · {}", self.steps, self.project),
                Style::default().fg(MUTED),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(left).block(Block::default().padding(Padding::new(1, 0, 0, 0))),
            cols[0],
        );

        let hints = Line::from(Span::styled(
            "enter send · esc quit",
            Style::default().fg(FAINT),
        ));
        frame.render_widget(
            Paragraph::new(hints)
                .alignment(Alignment::Right)
                .block(Block::default().padding(Padding::new(0, 1, 0, 0))),
            cols[1],
        );
    }
}

fn label(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_uppercase(),
        Style::default().fg(MUTED),
    ))
}

fn step_done(text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("▸ {text} "), Style::default().fg(MUTED)),
        Span::styled("✓", Style::default().fg(OK)),
    ])
}

fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let base = Style::default().fg(FG);
    let mut lines = Vec::new();
    let mut in_code = false;

    for raw in text.split('\n') {
        let stripped = raw.trim_start();

        if stripped.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            lines.push(Line::from(vec![
                Span::styled("  ", base),
                Span::styled(raw.to_string(), Style::default().fg(CODE)),
            ]));
            continue;
        }
        if stripped.is_empty() {
            lines.push(Line::default());
            continue;
        }
        if is_rule(stripped) {
            lines.push(Line::from(Span::styled(
                "─".repeat(26),
                Style::default().fg(FAINT),
            )));
            continue;
        }
        if let Some((level, content)) = heading(stripped) {
            let color = if level == 1 { ACCENT } else { FG };
            lines.push(Line::from(Span::styled(
                content,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if let Some(rest) = bullet(stripped) {
            let mut spans = vec![Span::styled("  • ", Style::default().fg(MUTED))];
            spans.extend(inline_spans(rest, base));
            lines.push(Line::from(spans));
            continue;
        }
        if let Some((marker, rest)) = numbered(stripped) {
            let mut spans = vec![Span::styled(
                format!("  {marker} "),
                Style::default().fg(MUTED),
            )];
            spans.extend(inline_spans(rest, base));
            lines.push(Line::from(spans));
            continue;
        }

        lines.push(Line::from(inline_spans(stripped, base)));
    }

    lines
}

fn is_rule(text: &str) -> bool {
    let text = text.trim();
    text.len() >= 3
        && (text.chars().all(|c| c == '-')
            || text.chars().all(|c| c == '*')
            || text.chars().all(|c| c == '_'))
}

fn heading(text: &str) -> Option<(usize, String)> {
    let hashes = text.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = &text[hashes..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Some((hashes, rest.trim().to_string()));
        }
    }
    None
}

fn bullet(text: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .into_iter()
        .find_map(|marker| text.strip_prefix(marker))
}

fn numbered(text: &str) -> Option<(String, &str)> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    text[digits.len()..]
        .strip_prefix(". ")
        .map(|rest| (format!("{digits}."), rest))
}

fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut i = 0;

    while i < text.len() {
        if text[i..].starts_with("**") {
            if let Some(rel) = text[i + 2..].find("**") {
                if plain_start < i {
                    spans.push(Span::styled(text[plain_start..i].to_string(), base));
                }
                spans.push(Span::styled(
                    text[i + 2..i + 2 + rel].to_string(),
                    base.add_modifier(Modifier::BOLD),
                ));
                i = i + 2 + rel + 2;
                plain_start = i;
                continue;
            }
        } else if text.as_bytes()[i] == b'`'
            && let Some(rel) = text[i + 1..].find('`')
        {
            if plain_start < i {
                spans.push(Span::styled(text[plain_start..i].to_string(), base));
            }
            spans.push(Span::styled(
                text[i + 1..i + 1 + rel].to_string(),
                Style::default().fg(CODE),
            ));
            i = i + 1 + rel + 1;
            plain_start = i;
            continue;
        }
        i += text[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }

    if plain_start < text.len() {
        spans.push(Span::styled(text[plain_start..].to_string(), base));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base));
    }
    spans
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
        let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    fn app() -> App {
        App::new("my-app".to_string(), "claude-sonnet-4-6".to_string())
    }

    #[test]
    fn idle_shows_header_and_empty_preview() {
        let mut app = app();
        let text = draw(&mut app);
        assert!(text.contains("codelight"));
        assert!(text.contains("my-app"));
        assert!(text.contains("no preview yet"));
    }

    #[test]
    fn conversation_and_tool_lifecycle_render() {
        let mut app = app();
        app.push_user("read package.json");
        app.apply(AgentEvent::ToolStarted {
            id: "call_1".to_string(),
            label: "read app/dashboard/page.tsx".to_string(),
        });
        app.apply(AgentEvent::ToolFinished {
            id: "call_1".to_string(),
            ok: true,
        });
        app.apply(AgentEvent::Token("Found it.".to_string()));
        app.apply(AgentEvent::Done);

        let text = draw(&mut app);
        assert!(text.contains("read package.json"));
        assert!(text.contains("READ"));
        assert!(text.contains("Found it."));
    }

    #[test]
    fn ready_preview_shows_url_and_actions() {
        let mut app = app();
        app.seed_demo();
        let text = draw(&mut app);
        assert!(text.contains("DEPLOYMENT"));
        assert!(text.contains("my-app-git-spinner.vercel.app"));
        assert!(text.contains("open · copy · logs"));
    }

    #[test]
    fn a_failed_tool_is_marked() {
        let mut app = app();
        app.apply(AgentEvent::ToolStarted {
            id: "call_9".to_string(),
            label: "deploy preview".to_string(),
        });
        app.apply(AgentEvent::ToolFinished {
            id: "call_9".to_string(),
            ok: false,
        });
        let text = draw(&mut app);
        assert!(text.contains("✗"));
    }

    #[test]
    fn renders_markdown_headings_bullets_and_inline_code() {
        let mut app = app();
        app.push_user("q");
        app.apply(AgentEvent::Token(
            "## Setup\n- install `next`\n- run it\n\n**Done** now\n\n---".to_string(),
        ));
        app.apply(AgentEvent::Done);

        let text = draw(&mut app);
        assert!(text.contains("Setup"));
        assert!(!text.contains("##"));
        assert!(text.contains("•"));
        assert!(!text.contains('`'));
        assert!(!text.contains("**"));
        assert!(text.contains("next"));
        assert!(text.contains("Done"));
        assert!(text.contains("─"));
    }

    #[test]
    fn renders_fenced_code_block_without_fences() {
        let mut app = app();
        app.push_user("q");
        app.apply(AgentEvent::Token("```tsx\nconst x = 1\n```".to_string()));
        app.apply(AgentEvent::Done);

        let text = draw(&mut app);
        assert!(text.contains("const x = 1"));
        assert!(!text.contains("```"));
    }
}
