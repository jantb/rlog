mod merge;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{collections::{HashMap, VecDeque}, error::Error, io, iter, option::Iter, time::{self, SystemTime, Instant}};
use std::borrow::Borrow;
use std::ops::{Add, Sub};
use std::time::Duration;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

enum InputMode {
    Normal,
    Editing,
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: Vec<char>,
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
            input_index: 0,
            input_mode: InputMode::Normal,
            messages: Messages::new(),
            skip: 0,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::default();

    let x1 = app.messages.map.entry("System".to_string()).or_insert_with(|| VecDeque::new());
    let arg: &'static String = Box::leak(Box::new("heyOldest".to_string()));
    x1.push_back(Message { timestamp: SystemTime::now().sub(Duration::from_secs(10)), value:  &arg });
    let arg1: &'static String = Box::leak(Box::new("hey11".to_string()));
    x1.push_back(Message { timestamp: SystemTime::now(), value:  &arg1 });

    let x2 = app.messages.map.entry("System2".to_string()).or_insert_with(|| VecDeque::new());
    let arg: &'static String = Box::leak(Box::new("hey2Oldest".to_string()));
    x2.push_back(Message { timestamp: SystemTime::now().sub(Duration::from_secs(1)), value:  &arg });
    let arg1: &'static String = Box::leak(Box::new("hey21".to_string()));
    x2.push_back(Message { timestamp: SystemTime::now(), value:  &arg1 });

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
                        // app.messages.push_front(app.input.drain(..).collect());
                        app.input_index = 0;
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
        .margin(2)
        .constraints(
            [
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
                .as_ref(),
        )
        .split(f.size());

    let mut messages: Vec<ListItem> = app
        .messages
        .iter()
        .enumerate()
        .filter(|&x| x.1.value.len() > 0)
        .skip(app.skip)
        .map(|(i, m)| {
            let content = vec![Spans::from(Span::raw(format!("{}: {}", i, m.value)))];
            ListItem::new(content)
        })
        .take(chunks[0].height.into())
        .collect();
    let messages = List::new(messages).block(Block::default().borders(Borders::NONE));
    f.render_widget(messages, chunks[0]);

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to start editing."),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to record the message"),
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
