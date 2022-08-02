extern crate core;

use std::{collections::{HashMap, VecDeque}, error::Error, fmt, io, iter, mem, thread};

use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::ops::{Add};
use std::process::{Command, Stdio};

use std::str::FromStr;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering as OtherOrdering};
use num_format::{Locale, ToFormattedString};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use bytesize::ByteSize;
use chrono::{DateTime, Utc};

mod pod;

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
use tui::layout::{Alignment, Rect};
use tui::style::Modifier;
use tui::widgets::{List, ListItem, ListState, Wrap};
use search_thread::command_message::CommandMessage;
use search_thread::result_message::ResultMessage;
use crate::Mode::{Search, SelectPods};

mod merge;
mod search_thread;

/// App holds the state of the application
struct App {
    stops: Vec<Arc<AtomicBool>>,
    pods: StatefulList<Pod>,
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
            stops: Vec::new(),
            pods: StatefulList::with_items(vec![Pod { name: "Pod1".to_string() }, Pod { name: "Pod2".to_string() }, Pod { name: "Pod3".to_string() }]),
            mode: Search,
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
    let mut app = App::default(tx, rx_result);

    search_thread::search_thread(rx, tx_result);

    let output = Command::new("oc")
        .arg("get")
        .arg("pods")
        .arg("-o")
        .arg("json")
        .output()
        .expect("ls command failed to start");
    match output.status.success() {
        true => {
            let result: Result<pod::pods::Pods, _> = serde_json::from_str(String::from_utf8_lossy(&output.stdout).to_string().as_str());

            let pods = match result {
                Ok(l) => { l }
                Err(err) => {
                    println!("{}", err.to_string());
                    return Ok(());
                }
            };
            app.pods = StatefulList::with_items(pods.items.iter()
                .map(|p| { Pod { name: p.metadata.name.clone() } }).collect());
        }
        false => {
            println!("{}", String::from_utf8_lossy(&output.stderr));
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

fn parse_and_send(x: &str, sender: &Sender<CommandMessage>) {
    let result: Result<LogFormat, _> = serde_json::from_str(x.to_string().as_str());
    let log_entry = match result {
        Ok(l) => { l }
        Err(_) => { return; }
    };
    let dt = DateTime::parse_from_str(log_entry.timestamp.add("00").as_str(), "%Y-%m-%dT%H:%M:%S%z");
    if dt.is_ok() {
        let time = dt.unwrap().with_timezone(&Utc);
        let m = Message {
            timestamp: time,
            value: log_entry.message,
            system: log_entry.application,
            level: match Level::from_str(&log_entry.level) {
                Ok(s) => { s }
                Err(_) => { return; }
            },
        };
        match sender.send(CommandMessage::InsertJson(m)) {
            Ok(_) => {}
            Err(_) => { return; }
        };
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
        if !event::poll(Duration::from_millis(8)).unwrap() {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            changed = true;
            match app.mode {
                SelectPods => {
                    match key.code {
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                                app.tx.send(CommandMessage::Exit).unwrap();
                                return Ok(());
                            }
                            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                                app.mode = Search;
                                app.stops.clear();

                                let selected_pods: Vec<_> = app.pods.selected.iter().map(|pod_index| { &app.pods.items[*pod_index] }).collect();
                                let stops: Vec<_> = selected_pods.iter().map(|pod| {
                                    let name = pod.name.clone();
                                    let sender = app.tx.clone();
                                    let please_stop = Arc::new(AtomicBool::new(false));
                                    let should_i_stop = please_stop.clone();
                                    thread::spawn(move || {
                                        let stdout = Command::new("oc")
                                            .stdout(Stdio::piped())
                                            .arg("logs")
                                            .arg("-f")
                                            .arg(name)
                                            .arg("--since=200h")
                                            .spawn().expect("Unable to start tool");
                                        match stdout.stdout {
                                            None => {}
                                            Some(l) => {
                                                let mut reader = BufReader::new(l);
                                                let mut buf = String::new();
                                                while !should_i_stop.load(OtherOrdering::SeqCst) {
                                                    let result = reader.read_line(&mut buf).expect("Unable to read");
                                                    if result == 0 {
                                                        thread::sleep(Duration::from_millis(100));
                                                        continue;
                                                    }
                                                    parse_and_send(&buf, &sender);
                                                    buf.clear()
                                                }
                                            }
                                        }
                                    });
                                    return please_stop;
                                }).collect();
                                app.stops = stops;
                                continue;
                            }
                        }
                        KeyCode::Down => app.pods.next(),
                        KeyCode::Up => app.pods.previous(),
                        KeyCode::Enter => app.pods.select(),
                        _ => {}
                    }
                }
                Search => {
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
                            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                                app.mode = SelectPods;
                                app.stops.iter().for_each(|s| {s.store(true, OtherOrdering::SeqCst)});
                                app.tx.send(CommandMessage::Clear).unwrap();
                                continue;
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
    match app.mode {
        SelectPods => {
            let items: Vec<ListItem> = app
                .pods
                .items
                .iter()
                .enumerate()
                .map(|i| {
                    ListItem::new(Spans::from(i.1.name.clone())).style(Style::default().fg(match app.pods.selected().contains(&i.0) {
                        true => { Color::Red }
                        false => { Color::White }
                    }))
                })
                .collect();

            let items = List::new(items)
                .block(Block::default().borders(Borders::NONE).title("Select pods"))
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("");

            f.render_stateful_widget(items, chunks[0], &mut app.pods.state);
        }
        Search => {
            render_search(f, app, chunks)
        }
    }
}

fn render_search<B: Backend>(f: &mut Frame<B>, app: &mut App, chunks: Vec<Rect>) {
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
                0 => { " Follow mode " }
                _ => { "" }
            }, Style::default().fg(Color::Cyan)),
            Span::styled(format!("── total lines {} ── ", app.length.to_formatted_string(&Locale::fr)), Style::default().fg(Color::Cyan)),
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

#[derive(Deserialize, Serialize)]
struct Pod {
    name: String,
}

struct StatefulList<Pod> {
    state: ListState,
    items: Vec<Pod>,
    selected: HashSet<usize>,
}

impl<Pod> StatefulList<Pod> {
    fn with_items(items: Vec<Pod>) -> StatefulList<Pod> {
        StatefulList {
            state: ListState::default(),
            items,
            selected: HashSet::new(),
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }


    fn selected(&self) -> &HashSet<usize> {
        return &self.selected;
    }

    fn select(&mut self) {
        let x = &self.state.selected().unwrap();
        if self.selected.contains(x) {
            self.selected.remove(x);
        } else {
            let _ = self.selected.insert(self.state.selected().unwrap());
        };
    }
}
