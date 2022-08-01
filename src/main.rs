extern crate core;

use std::{collections::{HashMap, VecDeque}, error::Error, fmt, io, iter, mem, thread};
use std::cmp::Ordering;
use std::ops::Add;

use std::str::FromStr;
use std::sync::mpsc;
use num_format::{Locale, ToFormattedString};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use bytesize::ByteSize;
use chrono::{DateTime, Utc};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::{Deserialize, Serialize};
use crossterm::event::KeyModifiers;

use tui::{
    backend::{Backend, CrosstermBackend},
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    Terminal,
    text::{Span, Spans, Text}, widgets::{Block, Borders, Paragraph},
};
use tui::layout::Alignment;
use tui::widgets::Wrap;
use search_thread::command_message::CommandMessage;
use search_thread::result_message::ResultMessage;

mod merge;
mod search_thread;

/// App holds the state of the application
struct App {
    input: Vec<char>,
    mode: Mode,
    input_index: usize,
    messages: Vec<Message>,
    skip: usize,
    size: u64,
    length: usize,
    elapsed: Duration,
    window_size: u16,
    tx: Sender<CommandMessage>,
    rx_result: Receiver<ResultMessage>,
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
    fn iter(&self) -> Box<dyn Iterator<Item=&Message> + '_> {
        let x: Vec<&VecDeque<Message>> = self.map.values().into_iter().collect::<Vec<_>>();
        if x.len() == 0 {
            return Box::new(iter::empty::<&Message>());
        }

        let mut ma: Box<dyn Iterator<Item=_>> = Box::new(x[0].iter().map(|i| i));
        for v in x.iter().skip(1) {
            ma = Box::new(merge::MergeAscending::new(ma, v.iter().map(|i| i)));
        };
        return ma;
    }

    fn put(&mut self, m: Message) {
        self.count += 1;
        self.size += m.value.len() as u64;
        self.size += m.system.len() as u64;
        self.size += mem::size_of_val(&m.timestamp) as u64;
        self.map.entry(m.system.to_string()).or_insert_with(|| VecDeque::new()).push_front(m);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Clone)]
pub struct Message {
    timestamp: DateTime<Utc>,
    system: String,
    level: Level,
    value: String,
}

impl Ord for Message {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Copy, Clone)]
enum Level {
    INFO,
    WARN,
    ERROR,
    DEBUG,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Level::INFO => { write!(f, "INFO") }
            Level::WARN => { write!(f, "WARN") }
            Level::ERROR => { write!(f, "ERROR") }
            Level::DEBUG => { write!(f, "DEBUG") }
        }
    }
}

impl FromStr for Level {
    type Err = ();

    fn from_str(input: &str) -> Result<Level, Self::Err> {
        match input {
            "INFO" => Ok(Level::INFO),
            "WARN" => Ok(Level::WARN),
            "DEBUG" => Ok(Level::DEBUG),
            "ERROR" => Ok(Level::ERROR),
            _ => Err(()),
        }
    }
}

impl App {
    fn default(tx: Sender<CommandMessage>, rx_result: Receiver<ResultMessage>) -> App {
        App {
            mode: Mode::Search,
            input: Vec::new(),
            input_index: 0,
            messages: Vec::new(),
            skip: 0,
            length: 0,
            size: 0,
            elapsed: Duration::from_micros(0),
            window_size: 0,
            tx,
            rx_result,
        }
    }
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
    let (tx_result, rx_result) = mpsc::channel();
    let sender = tx.clone();
    let app = App::default(tx, rx_result);

    search_thread::search_thread(rx, tx_result);
    thread::spawn(move || {
        for _ in 0..1_000_000 {
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:21+02", "message": "Message number 999999", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:22+02", "message": "Message number 999991", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:23+02", "message": "Message number 999992", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:24+02", "message": "Message number 999993", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:25+02", "message": "Message number 999993", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:26+02", "message": "Message number 999993", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:27+02", "message": "Message number 999993", "level": "ERROR", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:28+02", "message": "Messnage sssdsdjsndasdasdasdadskjsndksjndksjndskjndskjndskjndksjndksjndksjndksjndksjndksjndkjsndkjsndkjsdnskd sdjnskdjnskjdnskjdnksj dsdjskdnskndskjndksndksndksjnds skjdnskndksndksjndjksd skdj skjdsknumber 999993", "level": "INFO", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:29+02", "message": "Message number 999993", "level": "WARN", "application": "appname"}"#, &sender);
            parse_and_send(r#"{"@timestamp": "2022-08-07T04:10:30+02", "message": "Message number 999993", "level": "DEBUG", "application": "appname"}"#, &sender);
        }
    });
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

fn parse_and_send(x: &str, sender: &Sender<CommandMessage>) {
    let log_entry: LogFormat = serde_json::from_str(x.to_string().as_str()).unwrap();
    let dt = DateTime::parse_from_str(log_entry.timestamp.add("00").as_str(), "%Y-%m-%dT%H:%M:%S%z");
    if dt.is_ok() {
        let time = dt.unwrap().with_timezone(&Utc);
        let m = Message {
            timestamp: time,
            value: log_entry.message,
            system: log_entry.application,
            level: Level::from_str(&log_entry.level).unwrap(),
        };
        sender.send(CommandMessage::InsertJson(m)).unwrap();
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let mut changed = false;
    loop {
        while let Ok(result_message) = app.rx_result.try_recv() {
            changed = true;
            match result_message {
                ResultMessage::Messages(messages) => {
                    app.messages = messages;
                }
                ResultMessage::Elapsed(elapsed) => {
                    app.elapsed = elapsed;
                }
                ResultMessage::Size(size) => {
                    app.size = size
                }
                ResultMessage::Length(length) => {
                    app.length = length
                }
            }
        }
        if changed {
            changed = false;
            terminal.draw(|f| ui(f, &mut app))?;
        }
        if !event::poll(Duration::from_millis(10)).unwrap() {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            changed = true;
            match app.mode {
                Mode::SelectPods => {}
                Mode::Search => {
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
                        KeyCode::Enter => {
                            app.skip = 0;
                            app.tx.send(CommandMessage::SetSkip(0)).unwrap();
                        }
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                                app.tx.send(CommandMessage::Exit).unwrap();
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
    }
}

fn filter(app: &mut App) {
    app.tx.send(CommandMessage::FilterRegex(app.input.iter().collect())).unwrap();
}

enum Mode {
    SelectPods,
    Search,
}

fn ui<B: Backend>(f: &mut Frame<B>, mut app: &mut App) {
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
    if app.window_size != chunks[0].height {
        app.window_size = chunks[0].height;
        app.tx.send(CommandMessage::SetResultSize(chunks[0].height.into())).unwrap();
    }


    let mut messages: Vec<Spans> = app.messages.iter()
        .map(|m| {
            let content =
                Spans::from(
                    vec![
                        Span::styled(format!("{} ", format!("{}", m.timestamp.format("%+"))), Style::default().fg(Color::Cyan)),
                        Span::styled(format!("{} ", m.system), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("{} ", m.level), Style::default().fg(match m.level {
                            Level::INFO => { Color::Green }
                            Level::WARN => { Color::Magenta }
                            Level::ERROR => { Color::Red }
                            Level::DEBUG => { Color::Blue }
                        })),
                        Span::raw(format!("{}", m.value))]);
            content
        }).collect();
    messages.reverse();

    let messages = Paragraph::new(messages).wrap(Wrap { trim: false }).block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);

    let (msg, style) = (
        vec![
            Span::styled("┌─ ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:.2?}──", app.elapsed), Style::default().fg(Color::Cyan)),
            Span::styled(match app.skip {
                0 => { "Follow mode" }
                _ => { "" }
            }, Style::default().fg(Color::Cyan)),
            Span::styled(format!(" total lines {} ", app.length.to_formatted_string(&Locale::fr)), Style::default().fg(Color::Cyan)),
            Span::styled("", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", ByteSize::b(app.size)), Style::default().fg(Color::Cyan)),
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

//{"@timestamp": "2022-08-07T04:10:21+02", "message": "Message number 999999", "level": "INFO", "application": "appname"}
#[derive(Deserialize, Serialize)]
struct LogFormat {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    message: String,
    level: String,
    application: String,
}
