use std::{collections::{HashMap, VecDeque}, error::Error, io, iter, mem, time::SystemTime};
use std::ops::{Add, Sub};
use std::time::{Duration, Instant};
use bytesize::ByteSize;
use get_size::GetSize;
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use regex::Regex;
use tui::{
    backend::{Backend, CrosstermBackend},
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    Terminal,
    text::{Span, Spans, Text}, widgets::{Block, Borders, Paragraph},
};
use tui::layout::Alignment;
use tui::widgets::Wrap;

mod merge;

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: Vec<char>,
    filter: Regex,
    input_index: usize,
    /// History of recorded messages
    messages: Messages,
    skip: usize,
}

struct Messages {
    count: usize,
    size: u64,
    map: HashMap<String, VecDeque<Message>>,
}

impl Messages {
    fn new() -> Messages {
        Messages { count: 0, size: 0, map: HashMap::new() }
    }
    fn iter(&self) -> Box<dyn Iterator<Item=Message> + '_> {
        let x: Vec<&VecDeque<Message>> = self.map.values().into_iter().collect::<Vec<_>>();
        if x.len() == 0 {
            return Box::new(iter::empty::<Message>());
        }
        return merge::merging_iterator_from!(x);
    }

    fn put(&mut self, timestamp: SystemTime, system: &str, message: &str) {
        self.count += 1;
        let value: &'static String = Box::leak(Box::new(message.to_string()));
        let system: &'static String = Box::leak(Box::new(system.to_string()));
        let m = Message { timestamp, value: &value, system, level: "INFO"};
        self.size += value.get_heap_size() as u64;
        self.size += system.get_heap_size() as u64;
        self.size += mem::size_of_val(&timestamp) as u64;
        self.map.entry(system.to_string()).or_insert_with(|| VecDeque::new()).push_front(m);
    }

    fn len(&self) -> usize {
        return self.count;
    }
    fn size(&self) -> u64 {
        return self.map.get_heap_size() as u64 + self.size;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, GetSize)]
struct Message {
    timestamp: SystemTime,
    system: &'static String,
    level: &'static str,
    value: &'static String,
}

impl Default for App {
    fn default() -> App {
        App {
            input: Vec::new(),
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            input_index: 0,
            messages: Messages::new(),
            skip: 0,
        }
    }
}

fn iso8601(st: &SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    format!("{}", dt.format("%+"))
    // formats like "2001-07-08T00:34:60.026490+09:30"
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::default();

    for j in 0..10 {
        for i in 0..1_000 {
            let time = SystemTime::now().sub(Duration::from_secs(10000)).add(Duration::from_secs(i));
            app.messages.put(time, j.to_string().as_str(), format!("very long line indeed I wonder if it wraps \nsystem:{} datapoint: {}", j, i).as_str());
        }
    }

    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up => {
                    app.skip += 1;
                }
                KeyCode::Down => {
                    if app.skip > 0 {
                        app.skip -= 1;
                    }
                }
                KeyCode::Esc => {
                    return Ok(());
                }
                KeyCode::Enter => {
                    app.skip -= 0;
                }
                KeyCode::Char(c) => {
                    app.input.insert(app.input_index, c);
                    app.input_index += 1;
                    filter(&mut app);
                }
                KeyCode::Backspace => {
                    if app.input_index > 0 {
                        app.input_index -= 1;
                        app.input.remove(app.input_index);
                        filter(&mut app);
                    }
                }
                KeyCode::Left => {
                    if app.input_index > 0 {
                        let (x, y) = terminal.get_cursor().unwrap();
                        terminal.set_cursor(x - 1, y).ok();
                        app.input_index -= 1
                    }
                }

                KeyCode::Right => {
                    if app.input_index < app.input.len() {
                        let (x, y) = terminal.get_cursor().unwrap();
                        terminal.set_cursor(x + 1, y).ok();
                        app.input_index += 1
                    }
                }
                _ => {}
            }
        }
    }
}

fn filter(app: &mut App) {
    let s: String = app.input.iter().collect();
    app.filter = Regex::new(format!(r#".*{}.*"#, s).as_str()).unwrap_or(Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
                .as_ref(),
        )
        .split(f.size());
    let now = Instant::now();

    let skip = app
        .messages
        .iter()
        .enumerate()
        .filter(|&x| app.filter.is_match(x.1.value))
        .skip(app.skip);

    let mut messages: Vec<Spans> = skip
        .map(|(_i, m)| {
            let content =
                Spans::from(
                    vec![
                        Span::styled(format!("{} ", iso8601(&m.timestamp)), Style::default().fg(Color::Cyan)),
                        Span::styled(format!("{} ", m.system), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("{} ", m.level), Style::default().fg(Color::Green)),
                        Span::raw(format!("{}", m.value))]);
            content
        })
        .take(chunks[0].height.into())
        .collect();
    messages.reverse();
    let elapsed = now.elapsed();

    let messages = Paragraph::new(messages).wrap(Wrap { trim: false }).block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);

    let (msg, style) = (
        vec![
            Span::raw("Press "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to quit"),
            Span::styled("┌─ ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:.2?}──", elapsed), Style::default().fg(Color::Cyan)),
            Span::styled(match app.skip {
                0 => { "Follow mode" }
                _ => { "" }
            }, Style::default().fg(Color::Cyan)),
            Span::styled(format!(" total lines {} ", app.messages.len()), Style::default().fg(Color::Cyan)),
            Span::styled("", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", ByteSize::b(app.messages.size())), Style::default().fg(Color::Cyan)),
        ],
        Style::default());
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text).alignment(Alignment::Right);
    f.render_widget(help_message, chunks[1]);
    let s: String = app.input.iter().collect();
    let input = Paragraph::new(s.as_ref())
        .style(
            Style::default()
        )
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(input, chunks[2]);
    f.set_cursor(
        chunks[2].x + app.input_index as u16,
        chunks[2].y,
    )
}
