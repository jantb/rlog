use std::{collections::{HashMap, VecDeque}, error::Error, io, iter, time::SystemTime};
use std::ops::{Add, Sub};
use std::time::{Duration, Instant};

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
    text::{Span, Spans, Text}, widgets::{Block, Borders, List, ListItem, Paragraph},
};

mod merge;

enum InputMode {
    Normal,
    Editing,
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: Vec<char>,
    filter: Regex,
    input_index: usize,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Messages,
    skip: usize,
}

struct Messages {
    map: HashMap<String, VecDeque<Message>>,
}

impl Messages {
    fn new() -> Messages {
        Messages { map: HashMap::new() }
    }
    fn iter(&self) -> Box<dyn Iterator<Item=Message> + '_> {
        let x: Vec<&VecDeque<Message>> = self.map.values().into_iter().collect::<Vec<_>>();
        if x.len() == 0 {
            return Box::new(iter::empty::<Message>());
        }
        return merge::merging_iterator_from!(x);
    }
    fn put(&mut self, timestamp: SystemTime, system: &str, message: &str) {
        let value: &'static String = Box::leak(Box::new(message.to_string()));
        self.map.entry(system.to_string()).or_insert_with(|| VecDeque::new()).push_front(Message { timestamp, value: &value });
    }
    fn len(& self) -> usize {
        let mut count = 0;
        for x in self.map.values() {
           count += x.len()
        }
        return count;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
struct Message {
    timestamp: SystemTime,
    value: &'static String,
}

impl Default for App {
    fn default() -> App {
        App {
            input: Vec::new(),
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            input_index: 0,
            input_mode: InputMode::Normal,
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
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('i') => {
                        app.input_mode = InputMode::Editing;
                    }
                    KeyCode::Up => {
                        app.skip += 1;
                    }
                    KeyCode::Down => {
                        if app.skip > 0 {
                            app.skip -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        app.skip -= 0;
                    }
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        let s: String = app.input.iter().collect();
                        app.filter = Regex::new(format!(r#"{}"#, s).as_str()).unwrap_or(Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap());
                    }
                    KeyCode::Char(c) => {
                        app.input.insert(app.input_index, c);
                        app.input_index += 1;
                    }
                    KeyCode::Backspace => {
                        if app.input_index > 0 {
                            app.input_index -= 1;
                            app.input.remove(app.input_index);
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
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }
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

    let mut messages: Vec<ListItem> = skip
        .map(|(_i, m)| {
            let content = vec![
                Spans::from(
                    vec![Span::styled(format!("{}: ", iso8601(&m.timestamp)), Style::default().fg(Color::Cyan)),
                         Span::raw(format!("{}", m.value))]), ];
            ListItem::new(content)
        })
        .take(chunks[0].height.into())
        .collect();
    messages.reverse();
    let elapsed = now.elapsed();

    let messages = List::new(messages).block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to search "),
                Span::raw(format!("{:.2?}", elapsed)),
                Span::raw(" "),
                Span::raw(format!("{}",  app.messages.len())),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop searching using regex, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to execute the search "),
                Span::raw(format!("{:.2?}", elapsed)),
                Span::raw(" "),
                Span::raw(format!("{}",  app.messages.len())),
            ],
            Style::default(),
        ),
    };
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[1]);
    let s: String = app.input.iter().collect();
    let input = Paragraph::new(s.as_ref())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(input, chunks[2]);
    match app.input_mode {
        InputMode::Normal =>
        // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[2].x + app.input_index as u16,
                // Move one line down, from the border to the input line
                chunks[2].y,
            )
        }
    }
}
