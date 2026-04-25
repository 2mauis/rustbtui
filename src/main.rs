mod codeql_demo;

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
        Line::from("Use `x` to inject a simulated bad event."),
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
            "q quit   j/k move   space cycle state   tab switch view   a add event   x bad event   r reset",
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
    incident_index: usize,
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
            incident_index: 0,
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
            KeyCode::Char('x') => {
                let incident = self.next_incident();
                self.push_log(incident);
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
        self.incident_index = 0;
        self.history.fill(12);
        for item in &mut self.items {
            item.state = TaskState::Idle;
        }
        self.logs.clear();
        self.push_log("reset: dashboard state restored".to_string());
    }

    fn next_incident(&mut self) -> String {
        let name = self.current_item().name.clone();
        let message = match self.incident_index % 3 {
            0 => format!("incident: unwrap() panic simulated in {name}"),
            1 => format!("incident: vulnerability alert simulated for {name}"),
            _ => format!("incident: stalled worker simulated for {name}"),
        };
        self.incident_index += 1;
        message
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

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;

    use super::*;

    // ── TaskState ──────────────────────────────────────────────────────────────

    #[test]
    fn task_state_label_idle() {
        assert_eq!(TaskState::Idle.label(), "idle");
    }

    #[test]
    fn task_state_label_running() {
        assert_eq!(TaskState::Running.label(), "running");
    }

    #[test]
    fn task_state_label_done() {
        assert_eq!(TaskState::Done.label(), "done");
    }

    // ── Item ───────────────────────────────────────────────────────────────────

    #[test]
    fn item_new_creates_correctly() {
        let item = Item::new("TestName", "TestFocus", "Test description");
        assert_eq!(item.name, "TestName");
        assert_eq!(item.focus, "TestFocus");
        assert_eq!(item.description, "Test description");
        assert!(matches!(item.state, TaskState::Idle));
    }

    #[test]
    fn item_cycle_state_idle_to_running() {
        let mut item = Item::new("T", "F", "D");
        item.cycle_state();
        assert!(matches!(item.state, TaskState::Running));
    }

    #[test]
    fn item_cycle_state_running_to_done() {
        let mut item = Item::new("T", "F", "D");
        item.state = TaskState::Running;
        item.cycle_state();
        assert!(matches!(item.state, TaskState::Done));
    }

    #[test]
    fn item_cycle_state_done_to_idle() {
        let mut item = Item::new("T", "F", "D");
        item.state = TaskState::Done;
        item.cycle_state();
        assert!(matches!(item.state, TaskState::Idle));
    }

    // ── AppMode ────────────────────────────────────────────────────────────────

    #[test]
    fn app_mode_next_overview_to_metrics() {
        assert!(matches!(AppMode::Overview.next(), AppMode::Metrics));
    }

    #[test]
    fn app_mode_next_metrics_to_events() {
        assert!(matches!(AppMode::Metrics.next(), AppMode::Events));
    }

    #[test]
    fn app_mode_next_events_to_overview() {
        assert!(matches!(AppMode::Events.next(), AppMode::Overview));
    }

    #[test]
    fn app_mode_index() {
        assert_eq!(AppMode::Overview.index(), 0);
        assert_eq!(AppMode::Metrics.index(), 1);
        assert_eq!(AppMode::Events.index(), 2);
    }

    #[test]
    fn app_mode_label() {
        assert_eq!(AppMode::Overview.label(), "overview");
        assert_eq!(AppMode::Metrics.label(), "metrics");
        assert_eq!(AppMode::Events.label(), "events");
    }

    #[test]
    fn app_mode_titles_count() {
        assert_eq!(AppMode::titles().len(), 3);
    }

    // ── App::new ───────────────────────────────────────────────────────────────

    #[test]
    fn app_new_defaults() {
        let app = App::new();
        assert_eq!(app.items.len(), 5);
        assert_eq!(app.selected, 0);
        assert!(matches!(app.mode, AppMode::Overview));
        assert_eq!(app.ticks, 0);
        assert_eq!(app.history.len(), HISTORY_LEN);
        assert_eq!(app.logs.len(), 2);
        assert_eq!(app.incident_index, 0);
    }

    // ── App counters ───────────────────────────────────────────────────────────

    #[test]
    fn app_done_count_initially_zero() {
        let app = App::new();
        assert_eq!(app.done_count(), 0);
    }

    #[test]
    fn app_running_count_initially_zero() {
        let app = App::new();
        assert_eq!(app.running_count(), 0);
    }

    #[test]
    fn app_done_count_after_setting_states() {
        let mut app = App::new();
        app.items[0].state = TaskState::Done;
        app.items[2].state = TaskState::Done;
        assert_eq!(app.done_count(), 2);
    }

    #[test]
    fn app_running_count_after_setting_states() {
        let mut app = App::new();
        app.items[1].state = TaskState::Running;
        app.items[3].state = TaskState::Running;
        assert_eq!(app.running_count(), 2);
    }

    // ── App::progress ──────────────────────────────────────────────────────────

    #[test]
    fn app_progress_zero_when_no_done() {
        let app = App::new();
        assert_eq!(app.progress(), 0);
    }

    #[test]
    fn app_progress_100_when_all_done() {
        let mut app = App::new();
        for item in &mut app.items {
            item.state = TaskState::Done;
        }
        assert_eq!(app.progress(), 100);
    }

    #[test]
    fn app_progress_partial() {
        let mut app = App::new();
        // 2 out of 5 done = 40%
        app.items[0].state = TaskState::Done;
        app.items[1].state = TaskState::Done;
        assert_eq!(app.progress(), 40);
    }

    // ── App::current_item ──────────────────────────────────────────────────────

    #[test]
    fn app_current_item_returns_selected() {
        let app = App::new();
        assert_eq!(app.current_item().name, app.items[0].name);
    }

    #[test]
    fn app_current_item_after_selection_change() {
        let mut app = App::new();
        app.selected = 3;
        assert_eq!(app.current_item().name, app.items[3].name);
    }

    // ── App::push_log ──────────────────────────────────────────────────────────

    #[test]
    fn app_push_log_appends_entry() {
        let mut app = App::new();
        let initial_len = app.logs.len();
        app.push_log("test event".to_string());
        assert_eq!(app.logs.len(), initial_len + 1);
        assert_eq!(app.logs.last().unwrap(), "test event");
    }

    #[test]
    fn app_push_log_truncates_to_max_logs() {
        let mut app = App::new();
        for i in 0..MAX_LOGS + 5 {
            app.push_log(format!("log {i}"));
        }
        assert_eq!(app.logs.len(), MAX_LOGS);
    }

    #[test]
    fn app_push_log_keeps_newest_entries() {
        let mut app = App::new();
        app.logs.clear();
        for i in 0..MAX_LOGS + 3 {
            app.push_log(format!("log {i}"));
        }
        // The last entry should be the most-recently pushed one.
        assert_eq!(app.logs.last().unwrap(), &format!("log {}", MAX_LOGS + 2));
    }

    // ── App::reset ─────────────────────────────────────────────────────────────

    #[test]
    fn app_reset_clears_state() {
        let mut app = App::new();
        app.ticks = 50;
        app.mode = AppMode::Events;
        app.incident_index = 2;
        app.items[0].state = TaskState::Done;
        app.history[0] = 99;

        app.reset();

        assert_eq!(app.ticks, 0);
        assert!(matches!(app.mode, AppMode::Overview));
        assert!(app.items.iter().all(|i| matches!(i.state, TaskState::Idle)));
        assert!(app.history.iter().all(|&v| v == 12));
        assert_eq!(app.logs.len(), 1);
        assert_eq!(app.incident_index, 0);
        assert_eq!(app.logs[0], "reset: dashboard state restored");
    }

    // ── App::status_line ───────────────────────────────────────────────────────

    #[test]
    fn app_status_line_all_idle() {
        let app = App::new();
        assert_eq!(app.status_line(), "0 done, 0 running, 5 total");
    }

    #[test]
    fn app_status_line_with_mixed_states() {
        let mut app = App::new();
        app.items[0].state = TaskState::Done;
        app.items[1].state = TaskState::Running;
        assert_eq!(app.status_line(), "1 done, 1 running, 5 total");
    }

    // ── App::on_tick ───────────────────────────────────────────────────────────

    #[test]
    fn on_tick_increments_ticks() {
        let mut app = App::new();
        app.on_tick();
        assert_eq!(app.ticks, 1);
    }

    #[test]
    fn on_tick_updates_history_last_slot() {
        let mut app = App::new();
        app.history.fill(0);
        app.on_tick();
        // After rotate_left(1) and writing the new value the last slot holds
        // the freshly computed value, which must be >= 8.
        assert!(*app.history.last().unwrap() >= 8);
    }

    #[test]
    fn on_tick_logs_at_multiple_of_30() {
        let mut app = App::new();
        app.logs.clear();
        for _ in 0..30 {
            app.on_tick();
        }
        assert!(
            app.logs
                .iter()
                .any(|l| l.contains("tick: 30 cycles elapsed"))
        );
    }

    #[test]
    fn on_tick_does_not_log_before_multiple_of_30() {
        let mut app = App::new();
        app.logs.clear();
        for _ in 0..29 {
            app.on_tick();
        }
        assert!(app.logs.is_empty());
    }

    // ── App::handle_key ────────────────────────────────────────────────────────

    #[test]
    fn handle_key_q_signals_quit() {
        let mut app = App::new();
        assert!(app.handle_key(KeyCode::Char('q')));
    }

    #[test]
    fn handle_key_esc_signals_quit() {
        let mut app = App::new();
        assert!(app.handle_key(KeyCode::Esc));
    }

    #[test]
    fn handle_key_j_moves_selection_down() {
        let mut app = App::new();
        assert!(!app.handle_key(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn handle_key_down_moves_selection_down() {
        let mut app = App::new();
        assert!(!app.handle_key(KeyCode::Down));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn handle_key_j_wraps_at_bottom() {
        let mut app = App::new();
        app.selected = app.items.len() - 1;
        app.handle_key(KeyCode::Char('j'));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn handle_key_k_moves_selection_up() {
        let mut app = App::new();
        app.selected = 2;
        assert!(!app.handle_key(KeyCode::Char('k')));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn handle_key_up_moves_selection_up() {
        let mut app = App::new();
        app.selected = 2;
        assert!(!app.handle_key(KeyCode::Up));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn handle_key_k_wraps_at_top() {
        let mut app = App::new();
        app.selected = 0;
        app.handle_key(KeyCode::Char('k'));
        assert_eq!(app.selected, app.items.len() - 1);
    }

    #[test]
    fn handle_key_tab_cycles_through_all_modes() {
        let mut app = App::new();
        assert!(matches!(app.mode, AppMode::Overview));
        app.handle_key(KeyCode::Tab);
        assert!(matches!(app.mode, AppMode::Metrics));
        app.handle_key(KeyCode::Tab);
        assert!(matches!(app.mode, AppMode::Events));
        app.handle_key(KeyCode::Tab);
        assert!(matches!(app.mode, AppMode::Overview));
    }

    #[test]
    fn handle_key_space_cycles_task_state() {
        let mut app = App::new();
        assert!(matches!(app.items[0].state, TaskState::Idle));
        app.handle_key(KeyCode::Char(' '));
        assert!(matches!(app.items[0].state, TaskState::Running));
        app.handle_key(KeyCode::Char(' '));
        assert!(matches!(app.items[0].state, TaskState::Done));
        app.handle_key(KeyCode::Char(' '));
        assert!(matches!(app.items[0].state, TaskState::Idle));
    }

    #[test]
    fn handle_key_a_appends_heartbeat_log() {
        let mut app = App::new();
        let before_len = app.logs.len();
        app.handle_key(KeyCode::Char('a'));
        assert_eq!(app.logs.len(), before_len + 1);
        assert!(app.logs.last().unwrap().contains("heartbeat"));
    }

    #[test]
    fn handle_key_x_appends_bad_event_log() {
        let mut app = App::new();
        let before_len = app.logs.len();
        app.handle_key(KeyCode::Char('x'));
        assert_eq!(app.logs.len(), before_len + 1);
        assert!(app.logs.last().unwrap().contains("unwrap()"));
    }

    #[test]
    fn handle_key_x_cycles_bad_event_messages() {
        let mut app = App::new();
        app.logs.clear();
        app.handle_key(KeyCode::Char('x'));
        app.handle_key(KeyCode::Char('x'));

        assert!(app.logs[0].contains("unwrap()"));
        assert!(app.logs[1].contains("vulnerability"));
    }

    #[test]
    fn handle_key_r_resets_app() {
        let mut app = App::new();
        app.ticks = 100;
        app.handle_key(KeyCode::Char('r'));
        assert_eq!(app.ticks, 0);
    }

    #[test]
    fn handle_key_unknown_does_not_quit() {
        let mut app = App::new();
        assert!(!app.handle_key(KeyCode::F(1)));
    }

    #[test]
    fn handle_key_navigation_logs_cursor_event() {
        let mut app = App::new();
        app.logs.clear();
        app.handle_key(KeyCode::Char('j'));
        assert!(app.logs.iter().any(|l| l.starts_with("cursor:")));
    }

    #[test]
    fn handle_key_tab_logs_view_change() {
        let mut app = App::new();
        app.logs.clear();
        app.handle_key(KeyCode::Tab);
        assert!(app.logs.iter().any(|l| l.starts_with("view:")));
    }

    #[test]
    fn handle_key_space_logs_task_state_change() {
        let mut app = App::new();
        app.logs.clear();
        app.handle_key(KeyCode::Char(' '));
        assert!(app.logs.iter().any(|l| l.starts_with("task:")));
    }
}
