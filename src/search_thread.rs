use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Instant;
use regex::Regex;
use chrono::{DateTime, Utc};
use std::ops::Add;
use crate::{LogFormat, Messages};
use command_message::CommandMessage;
use result_message::ResultMessage;

pub mod command_message;
pub mod result_message;

struct Storage {
    filter: Regex,
    messages: Messages,
    skip: usize,
    result_size: usize,
}

impl Default for Storage {
    fn default() -> Storage {
        Storage {
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            messages: Messages::new(),
            skip: 0,
            result_size: 0,
        }
    }
}

pub fn search_thread(rx: Receiver<CommandMessage>, tx_result: Sender<ResultMessage>) {
    thread::spawn(move || {
        let mut storage = Storage::default();
        loop {
            let command_message =
                match rx.try_recv() {
                    Ok(message) => {
                        message
                    }
                    Err(error) => {
                        match error {
                            TryRecvError::Empty => {
                                let now = Instant::now();
                                tx_result.send(ResultMessage::Messages(storage
                                    .messages
                                    .iter()
                                    .enumerate()
                                    .filter(|&x| storage.filter.is_match(x.1.value))
                                    .skip(storage.skip)
                                    .take(storage.result_size)
                                    .map(|(_i, m)| { m }).collect())).unwrap();
                                tx_result.send(ResultMessage::Elapsed(now.elapsed())).unwrap();
                                rx.recv().unwrap()
                            }
                            TryRecvError::Disconnected => { panic!("{}", error.to_string()) }
                        }
                    }
                };
            match command_message {
                CommandMessage::FilterRegex(s) => {
                    storage.filter = Regex::new(format!(r#".*{}.*"#, s).as_str()).unwrap_or(Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap())
                }
                CommandMessage::Exit => {
                    break;
                }
                CommandMessage::InsertJson(json) => {
                    let log_entry: LogFormat = serde_json::from_str(json.as_str()).unwrap();
                    let dt = DateTime::parse_from_str(log_entry.timestamp.add("00").as_str(), "%Y-%m-%dT%H:%M:%S%z");
                    if dt.is_ok() {
                        storage.messages.put(dt.unwrap().with_timezone(&Utc), &log_entry.application, &log_entry.message, &log_entry.level);
                        tx_result.send(ResultMessage::Size(storage.messages.size)).unwrap();
                        tx_result.send(ResultMessage::Length(storage.messages.count)).unwrap();
                    }
                }
                CommandMessage::SetSkip(i) => {
                    storage.skip = i;
                }
                CommandMessage::SetResultSize(i) => {
                    storage.result_size = i;
                }
            }
        }
    });
}
