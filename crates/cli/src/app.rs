use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap};
use tui_input::Input;
use types::AgentEvent;

const ACCENT: Color = Color::Rgb(0x6c, 0x7c, 0xff);
const OK: Color = Color::Rgb(0x5b, 0xd6, 0xa4);
const WARN: Color = Color::Rgb(0xe0, 0xb3, 0x41);
const ERR: Color = Color::Rgb(0xf0, 0x61, 0x6d);
const FG: Color = Color::Rgb(0xe9, 0xe9, 0xee);
const MUTED: Color = Color::Rgb(0x76, 0x7a, 0x86);
const FAINT: Color = Color::Rgb(0x3c, 0x41, 0x4e);
const CODE: Color = Color::Rgb(0xa9, 0xb6, 0xff);
const LINE: Color = Color::Rgb(0x2b, 0x30, 0x3d);

const RAIL_WIDTH: u16 = 34;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

enum Kind {
    User,
    Assistant,
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

pub struct App {
    pub input: Input,
    project: String,
    model: String,
    entries: Vec<Entry>,
    assistant: Option<usize>,
    steps: u32,
    pub thinking: bool,
    scroll: u16,
    follow: bool,
    tick: usize,
    picker: Option<Picker>,
    approval: Option<crate::PendingApproval>,
}

struct Picker {
    query: String,
    all: Vec<String>,
    filtered: Vec<usize>,
    selected: usize,
    loading: bool,
}

impl Picker {
    fn refilter(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .enumerate()
            .filter(|(_, m)| needle.is_empty() || m.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
    }

    fn selected_model(&self) -> Option<&str> {
        self.filtered
            .get(self.selected)
            .map(|&i| self.all[i].as_str())
    }
}

impl App {
    pub fn new(project: String, model: String) -> Self {
        Self {
            input: Input::default(),
            project,
            model,
            entries: Vec::new(),
            assistant: None,
            steps: 0,
            thinking: false,
            scroll: 0,
            follow: true,
            tick: 0,
            picker: None,
            approval: None,
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
            AgentEvent::ToolStarted { .. } => {
                self.assistant = None;
            }
            AgentEvent::ToolFinished { .. } => {}
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
            AgentEvent::ModelList(models) => self.set_picker_models(models),
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

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn info(&mut self, message: &str) {
        self.entries.push(Entry {
            kind: Kind::Info,
            text: message.to_string(),
        });
        self.follow = true;
    }

    pub fn open_picker(&mut self, query: &str) {
        self.picker = Some(Picker {
            query: query.to_string(),
            all: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            loading: true,
        });
    }

    pub fn set_picker_models(&mut self, models: Vec<String>) {
        if let Some(picker) = self.picker.as_mut() {
            picker.all = models;
            picker.loading = false;
            picker.refilter();
        }
    }

    pub fn picker_is_open(&self) -> bool {
        self.picker.is_some()
    }

    pub fn picker_input(&mut self, c: char) {
        if let Some(picker) = self.picker.as_mut() {
            picker.query.push(c);
            picker.refilter();
        }
    }

    pub fn picker_backspace(&mut self) {
        if let Some(picker) = self.picker.as_mut() {
            picker.query.pop();
            picker.refilter();
        }
    }

    pub fn picker_up(&mut self) {
        if let Some(picker) = self.picker.as_mut()
            && picker.selected > 0
        {
            picker.selected -= 1;
        }
    }

    pub fn picker_down(&mut self) {
        if let Some(picker) = self.picker.as_mut()
            && picker.selected + 1 < picker.filtered.len()
        {
            picker.selected += 1;
        }
    }

    pub fn picker_selected(&self) -> Option<&str> {
        self.picker.as_ref().and_then(Picker::selected_model)
    }

    pub fn close_picker(&mut self) {
        self.picker = None;
    }

    pub fn open_approval(&mut self, pending: crate::PendingApproval) {
        self.approval = Some(pending);
    }

    pub fn approval_is_open(&self) -> bool {
        self.approval.is_some()
    }

    pub fn approval_action(&self) -> Option<&str> {
        self.approval.as_ref().map(|p| p.request.action.as_str())
    }

    pub fn approval_offers_always(&self) -> bool {
        self.approval
            .as_ref()
            .map(|p| p.request.suggested_pattern.is_some())
            .unwrap_or(false)
    }

    pub fn resolve_approval(&mut self, decision: types::Decision) {
        if let Some(pending) = self.approval.take() {
            pending.respond.send(decision).ok();
        }
    }

    pub fn seed_demo(&mut self) {
        self.push_user("fix the config loader and add a regression test");
        self.entries.push(Entry {
            kind: Kind::Assistant,
            text: "Demo session: updated the config loader to use defaults when the file is missing and added a regression test. No files were changed or commands run.".to_string(),
        });
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

        if self.picker.is_some() {
            self.render_picker(frame);
        }

        if self.approval.is_some() {
            self.render_approval(frame);
        }
    }

    fn render_approval(&self, frame: &mut Frame) {
        let Some(pending) = self.approval.as_ref() else {
            return;
        };
        let area = centered_rect(72, 50, frame.area());
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(WARN))
            .title(format!(" approve {}? ", pending.request.tool))
            .padding(Padding::new(1, 1, 0, 0));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

        let action = Paragraph::new(Line::from(Span::styled(
            self.approval_action().unwrap_or_default().to_string(),
            Style::default().fg(FG),
        )))
        .wrap(Wrap { trim: false });
        frame.render_widget(action, rows[0]);

        let mut hints = vec!["y allow once".to_string()];
        if let Some(pattern) = pending.request.suggested_pattern.as_ref() {
            hints.push(format!("a always allow ({pattern})"));
        }
        hints.push("n/esc deny".to_string());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hints.join(" · "),
                Style::default().fg(FAINT),
            ))),
            rows[1],
        );
    }

    fn render_picker(&self, frame: &mut Frame) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let area = centered_rect(72, 72, frame.area());
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .title(" select model ")
            .padding(Padding::new(1, 1, 0, 0));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

        let search = Line::from(vec![
            Span::styled("search ", Style::default().fg(MUTED)),
            Span::styled(picker.query.clone(), Style::default().fg(FG)),
        ]);
        frame.render_widget(Paragraph::new(search), rows[0]);

        let status = if picker.loading {
            "loading models…".to_string()
        } else {
            format!(
                "{} matches · ↑↓ move · enter select · esc cancel",
                picker.filtered.len()
            )
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(status, Style::default().fg(FAINT)))),
            rows[1],
        );

        let height = rows[2].height as usize;
        let start = if height > 0 && picker.selected >= height {
            picker.selected + 1 - height
        } else {
            0
        };
        let end = (start + height).min(picker.filtered.len());
        let mut lines = Vec::new();
        for i in start..end {
            let id = &picker.all[picker.filtered[i]];
            if i == picker.selected {
                lines.push(Line::from(Span::styled(
                    format!("› {id}"),
                    Style::default().fg(Color::Black).bg(ACCENT),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {id}"),
                    Style::default().fg(FG),
                )));
            }
        }
        frame.render_widget(Paragraph::new(lines), rows[2]);

        let cursor_x = rows[0].x + 7 + picker.query.chars().count() as u16;
        frame.set_cursor_position((cursor_x, rows[0].y));
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
        let mut lines: Vec<Line> = self.entries.iter().flat_map(Entry::to_lines).collect();
        if self.thinking && self.assistant.is_none() {
            lines.push(thinking_line(self.tick));
        }

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
        let lines = self.rail_idle();
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
            label("workspace"),
            Line::from(Span::styled("local project", Style::default().fg(MUTED))),
            Line::from(Span::styled("ready to code", Style::default().fg(FAINT))),
            Line::default(),
            label("project"),
            Line::from(Span::styled(self.project.clone(), Style::default().fg(FG))),
            Line::default(),
            label("model"),
            Line::from(Span::styled(self.model.clone(), Style::default().fg(MUTED))),
            Line::default(),
            label("session"),
            Line::from(Span::styled(
                format!("step {}/20", self.steps),
                Style::default().fg(MUTED),
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

fn thinking_line(tick: usize) -> Line<'static> {
    const TRACK: usize = 14;
    const WIN: usize = 3;
    let pos = tick % (TRACK - WIN + 1);
    let mut spans = vec![Span::styled("  thinking  ", Style::default().fg(MUTED))];
    for i in 0..TRACK {
        let (glyph, color) = if i >= pos && i < pos + WIN {
            ("━", ACCENT)
        } else {
            ("─", FAINT)
        };
        spans.push(Span::styled(glyph, Style::default().fg(color)));
    }
    Line::from(spans)
}

fn centered_rect(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let w = area.width * pct_x / 100;
    let h = area.height * pct_y / 100;
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

fn label(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_uppercase(),
        Style::default().fg(MUTED),
    ))
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
    fn idle_shows_header_and_local_workspace() {
        let mut app = app();
        let text = draw(&mut app);
        assert!(text.contains("codelight"));
        assert!(text.contains("my-app"));
        assert!(text.contains("local project"));
    }

    #[test]
    fn sidebar_shows_the_model() {
        let mut app = app();
        let text = draw(&mut app);
        assert!(text.contains("MODEL"));
        assert!(text.contains("claude-sonnet-4-6"));
        app.set_model("gpt-5".to_string());
        assert!(draw(&mut app).contains("gpt-5"));
    }

    #[test]
    fn picker_filters_and_navigates() {
        let mut app = app();
        app.open_picker("opus");
        app.set_picker_models(vec![
            "anthropic/claude-haiku-4.5".to_string(),
            "anthropic/claude-opus-4.5".to_string(),
            "anthropic/claude-opus-4.6".to_string(),
            "openai/gpt-5".to_string(),
        ]);
        assert!(app.picker_is_open());
        assert_eq!(app.picker_selected(), Some("anthropic/claude-opus-4.5"));
        app.picker_down();
        assert_eq!(app.picker_selected(), Some("anthropic/claude-opus-4.6"));
        app.picker_input('x');
        assert_eq!(app.picker_selected(), None);
        app.picker_backspace();
        assert_eq!(app.picker_selected(), Some("anthropic/claude-opus-4.5"));
        app.close_picker();
        assert!(!app.picker_is_open());
    }

    #[test]
    fn picker_modal_renders() {
        let mut app = app();
        app.open_picker("claude");
        app.set_picker_models(vec![
            "anthropic/claude-haiku-4.5".to_string(),
            "openai/gpt-5".to_string(),
        ]);
        let text = draw(&mut app);
        assert!(text.contains("select model"));
        assert!(text.contains("claude-haiku-4.5"));
        assert!(!text.contains("gpt-5"));
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
        assert!(text.contains("Found it."));
        assert!(!text.contains("app/dashboard/page.tsx"));
    }

    #[test]
    fn demo_shows_local_coding_session() {
        let mut app = app();
        app.seed_demo();
        let text = draw(&mut app);
        assert!(text.contains("config loader"));
        assert!(text.contains("local project"));
        assert!(!text.contains("DEPLOYMENT"));
    }

    #[test]
    fn shows_thinking_indicator_while_working() {
        let mut app = app();
        app.push_user("do something");
        let text = draw(&mut app);
        assert!(text.contains("thinking"));
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
