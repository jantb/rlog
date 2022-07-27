use std::{collections::{HashMap, VecDeque}, error::Error, io, iter, mem, thread, time::SystemTime};
use std::ops::{Add, Sub};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use bytesize::ByteSize;
use get_size::GetSize;
use chrono::{DateTime, Utc};
use chrono::format::Fixed::TimezoneName;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::{Deserialize, Serialize};
use crossterm::event::KeyModifiers;
use regex::Regex;
use tui::{
    backend::{Backend, CrosstermBackend},
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    Terminal,
    text::{Span, Spans, Text}, widgets::{Block, Borders, Paragraph},
};
use tui::layout::{Alignment};
use tui::widgets::Wrap;

mod merge;

/// App holds the state of the application
struct App {
    input: Vec<char>,
    filter: Regex,
    input_index: usize,
    messages: Messages,
    skip: usize,
    tx: Sender<CommandMessage>,
}

struct Storage {
    filter: Regex,
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

    fn put(&mut self, timestamp: DateTime<Utc>, system: &str, message: &str, level: &str) {
        self.count += 1;
        let value: &'static String = Box::leak(Box::new(message.to_string()));
        let system: &'static String = Box::leak(Box::new(system.to_string()));
        let level: &'static String = Box::leak(Box::new(level.to_string()));
        let m = Message { timestamp, value: &value, system, level };
        self.size += value.get_heap_size() as u64;
        self.size += system.get_heap_size() as u64;
        self.size += mem::size_of_val(&timestamp) as u64;
        self.map.entry(system.to_string()).or_insert_with(|| VecDeque::new()).push_front(m);
    }

    fn len(&self) -> usize {
        return self.count;
    }
    fn size(&self) -> u64 {
        return  self.size;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
struct Message {
    timestamp: DateTime<Utc>,
    system: &'static String,
    level: &'static str,
    value: &'static String,
}

impl App {
    fn default(tx: Sender<CommandMessage>) -> App {
        App {
            input: Vec::new(),
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            input_index: 0,
            messages: Messages::new(),
            skip: 0,
            tx,
        }
    }
}

impl Default for Storage {
    fn default() -> Storage {
        Storage {
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            messages: Messages::new(),
            skip: 0,
        }
    }
}


enum CommandMessage {
    FilterRegex(String),
    InsertJson(String),
    SetSkip(usize),
    Exit,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    //Command channel for searching etc
    let (tx, rx) = mpsc::channel();
    let sender = tx.clone();
    let app = App::default(tx);

    thread::spawn(move || {
        let mut storage = Storage::default();
        loop {
            let message = rx.recv().unwrap();
            match message {
                CommandMessage::FilterRegex(s) => {
                    storage.filter = Regex::new(format!(r#".*{}.*"#, s).as_str()).unwrap_or(Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap())
                }
                CommandMessage::Exit => {
                    break;
                }
                CommandMessage::InsertJson(json) => {
                    let log_entry: LogFormat = serde_json::from_str(json.as_str()).unwrap();
                    let dt = DateTime::parse_from_str(log_entry.timestamp.add("00").as_str(),"%Y-%m-%dT%H:%M:%S%z");
                    if dt.is_ok() {
                     storage.messages.put(dt.unwrap().with_timezone(&Utc), &log_entry.application, &log_entry.message, &log_entry.level)
                    }
                }
                CommandMessage::SetSkip(i) => {
                    storage.skip = i;
                }
            }
        }
    });
    for j in 0..10 {
        for i in 0..1_000 {
            app.tx.send(CommandMessage::InsertJson(r#"{"@timestamp": "2022-08-07T04:10:21+02", "message": "Message number 999999", "level": "INFO", "application": "appname"}"#.to_string())).unwrap();
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
    sender.send(CommandMessage::Exit).unwrap();
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up => {
                    app.skip += 1;
                    app.tx.send(CommandMessage::SetSkip(app.skip)).unwrap();
                }
                KeyCode::Down => {
                    if app.skip > 0 {
                        app.skip -= 1;
                        app.tx.send(CommandMessage::SetSkip(app.skip)).unwrap();
                    }
                }
                KeyCode::Esc => {
                    return Ok(());
                }
                KeyCode::Enter => {
                    app.skip = 0;
                    app.tx.send(CommandMessage::SetSkip(0)).unwrap();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                        return Ok(());
                    }
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
    app.tx.send(CommandMessage::FilterRegex(s)).unwrap();
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

    let skip = get_messages(app, chunks[0].height.into());

    let mut messages: Vec<Spans> = skip.iter()
        .map(|m| {
            let content =
                Spans::from(
                    vec![
                        Span::styled(format!("{} ", format!("{}", m.timestamp.format("%+"))), Style::default().fg(Color::Cyan)),
                        Span::styled(format!("{} ", m.system), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("{} ", m.level), Style::default().fg(Color::Green)),
                        Span::raw(format!("{}", m.value))]);
            content
        }).collect();
    messages.reverse();
    let elapsed = now.elapsed();

    let messages = Paragraph::new(messages).wrap(Wrap { trim: false }).block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);

    let (msg, style) = (
        vec![
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

fn get_messages(app: &App, i: usize) -> Vec<Message> {
    let skip: Vec<Message> = app
        .messages
        .iter()
        .enumerate()
        .filter(|&x| app.filter.is_match(x.1.value))
        .skip(app.skip)
        .take(i)
        .map(|(_i, m)| { m }).collect();
    skip
}

//{"@timestamp": "2022-08-07T04:10:21+02", "message": "Message number 999999", "level": "INFO", "application": "appname"}
#[derive(Deserialize, Serialize)]
struct LogFormat {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    message: String,
    level: String,
    application: String,
}
