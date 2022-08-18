extern crate core;

use std::{
    cmp::max,
    collections::HashSet,
    error::Error,
    io,
    sync::Arc,
    sync::atomic::AtomicBool,
    sync::atomic::Ordering as OtherOrdering,
    sync::mpsc,
    time::Duration,
};

use bytesize::ByteSize;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use crossterm::event::{KeyModifiers, MouseEventKind};
use num_format::{Locale, ToFormattedString};
use serde::{Deserialize, Serialize};
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
use tui::widgets::{List, ListItem, ListState};

use search_thread::command_message::CommandMessage;
use search_thread::result_message::ResultMessage;

use crate::app::App;
use crate::level::Level;
use crate::message::Message;
use crate::Mode::{Search, SelectPods, SelectTopics};
use crate::parse_send::parse_and_send;
use crate::pod::populate_pods::{populate_pods, populate_topics};
use crate::spawn_reader_thread::{clean_up_threads, spawn_reader_thread, spawn_reader_thread_kafka};

mod pod;
mod search_thread;
mod app;
mod message;
mod parse_send;
mod spawn_reader_thread;
mod level;

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen,EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    //Command channel for searching etc
    let (tx, rx) = mpsc::channel();
    let (tx_result, rx_result) = mpsc::channel();
    let app = App::default(tx, rx_result);

    search_thread::search_thread(rx, tx_result);

    let res = run_app(&mut terminal, app);
    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }
    Ok(())
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
                ResultMessage::Skip(s) => {
                    app.skip = s
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
        changed = true;
        match event::read()? {
            Event::Key(key) => {
                match app.mode {
                    SelectPods | SelectTopics => {
                        match key.code {
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                                    clean_up_threads(&mut app);
                                    app.tx.send(CommandMessage::Exit).unwrap();
                                    return Ok(());
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'a' {
                                    app.pods.select_all();
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' || key.modifiers.contains(KeyModifiers::CONTROL) && c == 'k' {
                                    let selected_pods: Vec<_> = app.pods.selected.iter().map(|pod_index| { &app.pods.items[*pod_index] }).collect();

                                    app.stops.clear();
                                    let stops = match app.mode {
                                        SelectPods => {
                                            let stops: Vec<_> = selected_pods.iter().map(|pod| {
                                                let name = pod.name.clone();
                                                let sender = app.tx.clone();
                                                let please_stop = Arc::new(AtomicBool::new(false));
                                                let should_i_stop = please_stop.clone();
                                                return (please_stop, spawn_reader_thread(name, sender, should_i_stop));
                                            }).collect();
                                            stops
                                        }
                                        SelectTopics => {
                                            let pods: Vec<_> = selected_pods.iter().map(|pod| { pod.name.clone() }).collect();
                                            let name = pods.join(" ");
                                            let sender = app.tx.clone();
                                            let please_stop = Arc::new(AtomicBool::new(false));
                                            let should_i_stop = please_stop.clone();
                                            let vec: Vec<_> = vec![(please_stop, spawn_reader_thread_kafka(name, sender, should_i_stop))];
                                            vec
                                        }
                                        _ => { panic!("Not possible") }
                                    };


                                    let (x, y): (Vec<_>, Vec<_>) = stops.into_iter().map(|(a, b)| (a, b)).unzip();
                                    app.stops = x;
                                    app.handles = y;
                                    app.mode = Search;
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
                                app.dropped_bottom_messages += 1;
                            }
                            KeyCode::Down => {
                                if app.dropped_bottom_messages > 0 {
                                    app.dropped_bottom_messages -= 1;
                                } else {
                                    if app.skip > 0 {
                                        app.skip -= 1;
                                        app.tx.send(CommandMessage::SetSkip(app.skip)).unwrap();
                                        app.just_skipped_bottom = true;
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                app.skip = 0;
                                app.dropped_bottom_messages = 0;
                                app.tx.send(CommandMessage::SetSkip(0)).unwrap();
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                                    clean_up_threads(&mut app);
                                    app.tx.send(CommandMessage::Exit).unwrap();
                                    return Ok(());
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'q' {
                                    app.show_debug = !app.show_debug;
                                    app.tx.send(CommandMessage::ToggleDebug()).unwrap();
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'w' {
                                    app.show_info = !app.show_info;
                                    app.tx.send(CommandMessage::ToggleInfo()).unwrap();
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'e' {
                                    app.show_warn = !app.show_warn;
                                    app.tx.send(CommandMessage::ToggleWarn()).unwrap();
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'r' {
                                    app.show_error = !app.show_error;
                                    app.tx.send(CommandMessage::ToggleError()).unwrap();
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'l' {
                                    app.wrap = !app.wrap;
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'k' {
                                    app.mode = SelectTopics;
                                    clean_up_threads(&mut app);

                                    app.tx.send(CommandMessage::Clear).unwrap();
                                    populate_topics(&mut app);
                                    continue;
                                }
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                                    app.mode = SelectPods;
                                    clean_up_threads(&mut app);

                                    app.tx.send(CommandMessage::Clear).unwrap();
                                    populate_pods(&mut app);
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
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(_) => {}
                    MouseEventKind::Up(_) => {}
                    MouseEventKind::Drag(_) => {}
                    MouseEventKind::Moved => {}
                    MouseEventKind::ScrollDown => {
                        if app.dropped_bottom_messages > 0 {
                            app.dropped_bottom_messages -= 1;
                        } else {
                            if app.skip > 0 {
                                app.skip -= 1;
                                app.tx.send(CommandMessage::SetSkip(app.skip)).unwrap();
                                app.just_skipped_bottom = true;
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        app.dropped_bottom_messages += 1;
                    }
                }
            }
            Event::Resize(_, _) => {}
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Paste(_) => {}
        }
    }
}


fn filter(app: &mut App) {
    let query: String = app.input.iter().collect();
    let x: Vec<_> = query.split(" ").collect();
    let neg_query: Vec<_> = x.iter().filter(|v| v.starts_with("!") && v.len() > 1).map(|v| v.strip_prefix("!").unwrap().to_string()).collect();
    let pos_query: Vec<_> = x.iter().filter(|&v| !v.starts_with("!")).map(|v| *v).collect();
    let pos_query = pos_query.join(" ");
    app.tx.send(CommandMessage::FilterRegex(pos_query)).unwrap();
    app.tx.send(CommandMessage::FilterNotRegexes(neg_query)).unwrap();
}

enum Mode {
    SelectPods,
    SelectTopics,
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
        SelectPods | SelectTopics => {
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
                .block(Block::default().borders(Borders::NONE).title("Select"))
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
    let screen_height = chunks[0].height;
    let mut m = Vec::new();

    let mut line_shifts: usize = 0;
    let mut count = 0;
    for x in &app.messages {
        m.push(x.clone());
        if app.wrap {
            line_shifts += x.value.chars().filter(|c| *c == '\n').count() + 1;
            if line_shifts > screen_height as usize && count > 0 {
                break;
            }
        }
        count += 1;
    }
    if m.len() > 0 {
        let mut mm = Vec::new();
        let option: Option<&Message> = m.last();
        mm.push(option.unwrap().clone());
        app.last_message_height = get_concatinated(&map_from_messages_to_text(&chunks, &mm, app.wrap)).height()
    }

    let messages = map_from_messages_to_text(&chunks, &m, app.wrap);
    let con_messages = get_concatinated(&messages);

    if app.just_skipped_bottom {
        app.dropped_bottom_messages += app.last_message_height - 1;
        app.just_skipped_bottom = false
    }

    if app.just_skipped {
        if app.dropped_bottom_messages >= app.last_message_height {
            app.dropped_bottom_messages -= app.last_message_height;
        }
    }
    let messages_height = con_messages.height();
    let top_skip: usize = max(messages_height as i32 - screen_height as i32 - app.dropped_bottom_messages as i32, 0).try_into().unwrap();
    if messages_height >= screen_height as usize {
        if app.dropped_bottom_messages >= app.last_message_height {
            app.skip += 1;
            app.tx.send(CommandMessage::SetSkip(app.skip)).unwrap();
            app.just_skipped = true;
            return render_search(f, app, chunks);
        }
    }
    app.just_skipped = false;

    let x: Vec<_> = con_messages.lines.into_iter().skip(top_skip).take(screen_height as usize).collect();
    let messages = Paragraph::new(Text::from(x)).block(Block::default().borders(Borders::NONE));

    let (msg, style) = (
        vec![
            Span::styled("┌─ ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:.2?}── ", app.elapsed), Style::default().fg(Color::Cyan)),
            Span::styled(match app.skip {
                0 => { "Following" }
                _ => { "Enter to follow" }
            }, Style::default().fg(Color::Cyan)),
            Span::styled(format!(" ── total lines {} ── ", app.length.to_formatted_string(&Locale::fr)), Style::default().fg(Color::Cyan)),
            Span::styled("", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", ByteSize::b(app.size)), Style::default().fg(Color::Cyan)),
            Span::styled(format!(" ── {}", "CTRL-q "), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", "DEBUG"), Style::default().fg(match app.show_debug {
                true => { Color::Blue }
                false => { Color::Cyan }
            })),
            Span::styled(format!("{}", ", CTRL-w "), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", "INFO"), Style::default().fg(match app.show_info {
                true => { Color::Green }
                false => { Color::Cyan }
            })),
            Span::styled(format!("{}", ", CTRL-e "), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", "WARN"), Style::default().fg(match app.show_warn {
                true => { Color::Magenta }
                false => { Color::Cyan }
            })),
            Span::styled(format!("{}", ", CTRL-r "), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", "ERROR"), Style::default().fg(match app.show_error {
                true => { Color::Red }
                false => { Color::Cyan }
            })),
            Span::styled(format!("{}", ", CTRL-l wrap"), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", ", CTRL-p pods"), Style::default().fg(Color::Cyan)),
        ],
        Style::default());
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text).alignment(Alignment::Right);

    let s: String = app.input.iter().collect();
    let input = Paragraph::new(s.as_ref())
        .style(
            Style::default()
        )
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);
    f.render_widget(help_message, chunks[1]);
    f.render_widget(input, chunks[2]);
    f.set_cursor(
        chunks[2].x + app.input_index as u16,
        chunks[2].y,
    );
}

fn get_concatinated<'a>(messages: &'a Vec<Text>) -> Text<'a> {
    messages.iter().fold(Text::raw(""), |mut sum, val| {
        sum.extend(val.clone());
        sum
    })
}

fn map_from_messages_to_text<'b>(chunks: &Vec<Rect>, messages: &'b Vec<Message>, wrap: bool) -> Vec<Text<'b>> {
    let messages: Vec<_> = messages.iter()
        .map(|m| {
            let mut content = vec![
                Span::styled(format!("{} ", format!("{}", m.timestamp.format("%+"))), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{} ", m.system), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{} ", m.level), Style::default().fg(match m.level {
                    Level::INFO => { Color::Green }
                    Level::WARN => { Color::Magenta }
                    Level::ERROR => { Color::Red }
                    Level::DEBUG => { Color::Blue }
                }))];
            if m.value.contains("\n") && wrap {
                let n: Vec<_> = m.value.splitn(2, |c| c == '\n').collect();
                content.push(Span::raw(n.get(0).unwrap().to_string()));
            } else {
                let text2 = Text::from(Spans::from(content.clone()));
                let take: String = m.value.chars().take(chunks[0].width as usize + 10 - text2.width()).collect();
                content.push(Span::raw(take.clone()));
            }
            let mut text = Text::from(Spans::from(content));
            if m.value.contains("\n") && wrap {
                let n: Vec<_> = m.value.splitn(2, |c| c == '\n').collect();
                text.extend(Text::raw(n.get(1).unwrap().to_string()));
            }
            return text;
        }).rev().collect();

    let messages: Vec<_> = if wrap {
        let messages: Vec<_> = messages.into_iter().map(|m| {
            if m.width() > chunks[0].width as usize {
                let x1: Vec<_> = m.lines.iter().map(|s| {
                    if s.width() < chunks[0].width as usize {
                        return Text::from(s.clone());
                    }
                    let spans = s.clone().0;
                    let (f, l) = spans.split_at(spans.len() - 1);
                    let len = max(chunks[0].width as i32 - Text::from(Spans::from(Vec::from(f))).width() as i32, 0) as usize;

                    let mut ll = 0;
                    for ch in l[0].content.to_string().chars().by_ref().take(len) {
                        ll += ch.len_utf8();
                    }
                    let line = l[0].content.split_at(ll);
                    let mut first_part = Vec::from(f);
                    first_part.push(Span::from(line.0.to_string()));
                    let text1 = sub_strings(line.1, chunks[0].width as usize).iter()
                        .map(|f| Text::raw(f.to_string())).fold(Text::from(Spans::from(first_part)), |mut sum, f| {
                        sum.extend(f);
                        sum
                    });
                    text1
                }).collect();
                x1
            } else {
                vec![m]
            }
        }
        )
            .flatten().collect();
        messages
    } else {
        return messages;
    };
    messages
}

//{"@timestamp": "2022-08-07T04:10:21+02", "message": "Message number 999999", "level": "INFO", "application": "appname"}
#[derive(Deserialize, Serialize)]
struct LogFormat {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    #[serde(default)]
    stack: String,
    #[serde(default)]
    stack_trace: String,
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
        let x = match self.state.selected() {
            None => { return; }
            Some(i) => { i }
        };
        if self.selected.contains(&x) {
            self.selected.remove(&x);
        } else {
            let _ = self.selected.insert(self.state.selected().unwrap());
        };
    }
    fn select_all(&mut self) {
        self.items.iter().enumerate().for_each(|i| {
            let _ = self.selected.insert(i.0);
        });
    }
}


fn sub_strings(string: &str, sub_len: usize) -> Vec<&str> {
    if sub_len == 0 {
        return Vec::new();
    }
    let mut subs = Vec::with_capacity(string.len() / sub_len);
    let mut iter = string.chars();
    let mut pos = 0;

    while pos < string.len() {
        let mut len = 0;
        for ch in iter.by_ref().take(sub_len) {
            len += ch.len_utf8();
        }
        subs.push(&string[pos..pos + len]);
        pos += len;
    }
    subs
}