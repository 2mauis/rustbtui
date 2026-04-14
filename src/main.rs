use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Sparkline, Tabs, Wrap},
};

const TICK_RATE: Duration = Duration::from_millis(120);
const HISTORY_LEN: usize = 48;
const MAX_LOGS: usize = 8;

fn main() -> Result<()> {
    let terminal = setup_terminal()?;
    let result = run(terminal);
    restore_terminal()?;
    result
}

fn setup_terminal() -> Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    Ok(ratatui::init())
}

fn restore_terminal() -> Result<()> {
    ratatui::restore();
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            let Event::Key(key) = event::read()? else {
                continue;
            };

            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.handle_key(key.code) {
                break;
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(18),
            Constraint::Length(4),
        ])
        .split(area);

    draw_header(frame, vertical[0], app);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(vertical[1]);

    draw_tasks(frame, body[0], app);
    draw_workspace(frame, body[1], app);
    draw_footer(frame, vertical[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(area);

    let title = Line::from(vec![
        Span::styled(
            " rustbtui ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw("  native ratatui dashboard"),
        Span::raw("  "),
        Span::styled(
            format!("tick {}", app.ticks),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("mode {}", app.mode.label()),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), rows[0]);

    let tabs = Tabs::new(AppMode::titles())
        .block(Block::default().borders(Borders::ALL).title("Views"))
        .select(app.mode.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(symbols::DOT);
    frame.render_widget(tabs, rows[1]);
}

fn draw_tasks(frame: &mut Frame, area: Rect, app: &App) {
    let items = app.items.iter().enumerate().map(|(idx, item)| {
        let marker = if idx == app.selected { ">" } else { " " };
        let status_style = match item.state {
            TaskState::Idle => Style::default().fg(Color::DarkGray),
            TaskState::Running => Style::default().fg(Color::Yellow),
            TaskState::Done => Style::default().fg(Color::Green),
        };

        ListItem::new(Line::from(vec![
            Span::raw(format!("{marker} ")),
            Span::styled(&item.name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(item.state.label(), status_style),
        ]))
    });

    let list = List::new(items)
        .block(Block::default().title("Pipeline").borders(Borders::ALL))
        .highlight_symbol(">> ");
    let mut state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_workspace(frame: &mut Frame, area: Rect, app: &App) {
    match app.mode {
        AppMode::Overview => draw_overview(frame, area, app),
        AppMode::Metrics => draw_metrics(frame, area, app),
        AppMode::Events => draw_events(frame, area, app),
    }
}

fn draw_overview(frame: &mut Frame, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Min(8),
        ])
        .split(area);

    let task = app.current_item();
    let details = Paragraph::new(vec![
        Line::from(vec![
            "Selected: ".into(),
            task.name.clone().bold().fg(Color::Green),
        ]),
        Line::from(format!("State: {}", task.state.label())),
        Line::from(format!("Focus: {}", task.focus)),
        Line::from(""),
        Line::from(task.description.as_str()),
    ])
    .block(Block::default().title("Details").borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    frame.render_widget(details, sections[0]);

    let gauge = Gauge::default()
        .block(Block::default().title("Completion").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Magenta))
        .label(format!("{}%", app.progress()))
        .ratio(f64::from(app.progress()) / 100.0)
        .use_unicode(true);
    frame.render_widget(gauge, sections[1]);

    let log_items = app
        .logs
        .iter()
        .rev()
        .map(|entry| ListItem::new(Line::from(entry.as_str())));
    let logs = List::new(log_items).block(
        Block::default()
            .title("Recent Activity")
            .borders(Borders::ALL),
    );
    frame.render_widget(logs, sections[2]);
}

fn draw_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(5),
        ])
        .split(area);

    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(sections[0]);

    let done_card = metric_card(
        "Done",
        format!("{}/{}", app.done_count(), app.items.len()),
        Color::Green,
    );
    let running_card = metric_card("Running", app.running_count().to_string(), Color::Yellow);
    let focus_card = metric_card("Focus", app.current_item().focus, Color::Cyan);
    frame.render_widget(done_card, cards[0]);
    frame.render_widget(running_card, cards[1]);
    frame.render_widget(focus_card, cards[2]);

    let pulse = Sparkline::default()
        .block(Block::default().title("Pulse").borders(Borders::ALL))
        .data(&app.history)
        .style(Style::default().fg(Color::Cyan))
        .max(100)
        .bar_set(symbols::bar::NINE_LEVELS);
    frame.render_widget(pulse, sections[1]);

    let hints = Paragraph::new(vec![
        Line::from("Use `space` to cycle task state."),
        Line::from("Use `Tab` to switch the right-hand view."),
        Line::from("Use `a` to append a synthetic activity event."),
        Line::from("Use `r` to reset the board."),
    ])
    .block(
        Block::default()
            .title("Operator Notes")
            .borders(Borders::ALL),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(hints, sections[2]);
}

fn draw_events(frame: &mut Frame, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8)])
        .split(area);

    let summary = Paragraph::new(vec![
        Line::from(format!("Events captured: {}", app.logs.len())),
        Line::from(format!("Selected task: {}", app.current_item().name)),
        Line::from(format!("Running tasks: {}", app.running_count())),
        Line::from(""),
        Line::from(app.status_line()),
    ])
    .block(Block::default().title("Session").borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    frame.render_widget(summary, sections[0]);

    let log_items = app
        .logs
        .iter()
        .rev()
        .map(|entry| ListItem::new(Line::from(entry.as_str())));
    let logs =
        List::new(log_items).block(Block::default().title("Event Log").borders(Borders::ALL));
    frame.render_widget(logs, sections[1]);
}

fn metric_card<'a, T>(title: &'a str, value: T, color: Color) -> Paragraph<'a>
where
    T: Into<String>,
{
    Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            value.into(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ])
    .block(Block::default().title(title).borders(Borders::ALL))
    .centered()
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let footer = Paragraph::new(vec![
        Line::from(
            "q quit   j/k move   space cycle state   tab switch view   a add event   r reset",
        ),
        Line::from(app.status_line()),
    ])
    .block(Block::default().title("Controls").borders(Borders::ALL));
    frame.render_widget(footer, area);
}

struct App {
    items: Vec<Item>,
    selected: usize,
    mode: AppMode,
    ticks: u64,
    history: Vec<u64>,
    logs: Vec<String>,
}

impl App {
    fn new() -> Self {
        Self {
            items: vec![
                Item::new(
                    "Input",
                    "Events",
                    "Capture keyboard input and keep the terminal responsive.",
                ),
                Item::new(
                    "Layout",
                    "Viewport",
                    "Split the screen into stable panes that resize cleanly.",
                ),
                Item::new(
                    "State",
                    "Store",
                    "Drive rendering from explicit app state instead of ad hoc flags.",
                ),
                Item::new(
                    "Render",
                    "Widgets",
                    "Mix list, gauge, tabs, paragraphs, and sparkline widgets.",
                ),
                Item::new(
                    "Shutdown",
                    "Terminal",
                    "Restore the terminal cleanly when the app exits.",
                ),
            ],
            selected: 0,
            mode: AppMode::Overview,
            ticks: 0,
            history: vec![12; HISTORY_LEN],
            logs: vec![
                "boot: ratatui dashboard initialized".to_string(),
                "hint: press Tab to switch views".to_string(),
            ],
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => true,
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % self.items.len();
                self.push_log(format!("cursor: selected {}", self.current_item().name));
                false
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = if self.selected == 0 {
                    self.items.len() - 1
                } else {
                    self.selected - 1
                };
                self.push_log(format!("cursor: selected {}", self.current_item().name));
                false
            }
            KeyCode::Tab => {
                self.mode = self.mode.next();
                self.push_log(format!("view: switched to {}", self.mode.label()));
                false
            }
            KeyCode::Char(' ') => {
                let item = &mut self.items[self.selected];
                item.cycle_state();
                let name = item.name.clone();
                let state = item.state.label();
                self.push_log(format!("task: {name} -> {state}"));
                false
            }
            KeyCode::Char('a') => {
                self.push_log(format!(
                    "event: heartbeat accepted for {}",
                    self.current_item().name
                ));
                false
            }
            KeyCode::Char('r') => {
                self.reset();
                false
            }
            _ => false,
        }
    }

    fn on_tick(&mut self) {
        self.ticks += 1;
        let done = self.done_count() as u64;
        let running = self.running_count() as u64;
        let value = ((self.ticks * 3 + done * 11 + running * 17) % 100).max(8);
        self.history.rotate_left(1);
        if let Some(last) = self.history.last_mut() {
            *last = value;
        }

        if self.ticks.is_multiple_of(30) {
            self.push_log(format!("tick: {} cycles elapsed", self.ticks));
        }
    }

    fn done_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.state, TaskState::Done))
            .count()
    }

    fn running_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.state, TaskState::Running))
            .count()
    }

    fn progress(&self) -> u16 {
        ((self.done_count() * 100) / self.items.len()) as u16
    }

    fn current_item(&self) -> &Item {
        &self.items[self.selected]
    }

    fn push_log(&mut self, entry: String) {
        self.logs.push(entry);
        if self.logs.len() > MAX_LOGS {
            let overflow = self.logs.len() - MAX_LOGS;
            self.logs.drain(0..overflow);
        }
    }

    fn reset(&mut self) {
        self.ticks = 0;
        self.mode = AppMode::Overview;
        self.history.fill(12);
        for item in &mut self.items {
            item.state = TaskState::Idle;
        }
        self.logs.clear();
        self.push_log("reset: dashboard state restored".to_string());
    }

    fn status_line(&self) -> String {
        format!(
            "{} done, {} running, {} total",
            self.done_count(),
            self.running_count(),
            self.items.len()
        )
    }
}

#[derive(Clone, Copy)]
enum AppMode {
    Overview,
    Metrics,
    Events,
}

impl AppMode {
    fn titles() -> Vec<Line<'static>> {
        vec!["Overview", "Metrics", "Events"]
            .into_iter()
            .map(Line::from)
            .collect()
    }

    fn next(self) -> Self {
        match self {
            Self::Overview => Self::Metrics,
            Self::Metrics => Self::Events,
            Self::Events => Self::Overview,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Metrics => 1,
            Self::Events => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Metrics => "metrics",
            Self::Events => "events",
        }
    }
}

struct Item {
    name: String,
    focus: &'static str,
    description: String,
    state: TaskState,
}

impl Item {
    fn new(name: &str, focus: &'static str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            focus,
            description: description.to_string(),
            state: TaskState::Idle,
        }
    }

    fn cycle_state(&mut self) {
        self.state = match self.state {
            TaskState::Idle => TaskState::Running,
            TaskState::Running => TaskState::Done,
            TaskState::Done => TaskState::Idle,
        };
    }
}

#[derive(Clone, Copy)]
enum TaskState {
    Idle,
    Running,
    Done,
}

impl TaskState {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Done => "done",
        }
    }
}
