use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Instant;

use regex::Regex;

use command_message::CommandMessage;
use result_message::ResultMessage;

use crate::search_thread::messages::Messages;

pub mod command_message;
pub mod result_message;
mod messages;
mod merge;

struct Storage {
    filter: Regex,
    filter_not: Vec<Regex>,
    messages: Messages,
    skip_messages: Messages,
    skip: usize,
    result_size: usize,
}

impl Default for Storage {
    fn default() -> Storage {
        Storage {
            filter: Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap(),
            filter_not: Vec::new(),
            messages: Messages::new(),
            skip_messages: Messages::new(),
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
                                    .filter(|x| storage.filter.is_match(x.value.as_str()))
                                    .filter(|x| if storage.filter_not.len() == 0 { true } else { !storage.filter_not.iter().any(|y| y.is_match(x.value.as_str())) })
                                    .skip(storage.skip)
                                    .take(storage.result_size)
                                    .map(|m| { m.clone() }).collect())).unwrap();
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
                CommandMessage::InsertJson(message) => {
                    if storage.skip == 0 {
                        storage.messages.put(message);
                    } else {
                        storage.skip_messages.put(message);
                    }
                    match tx_result.send(ResultMessage::Size(storage.messages.size + storage.skip_messages.size)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                    match tx_result.send(ResultMessage::Length(storage.messages.count + storage.skip_messages.count)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                }
                CommandMessage::SetSkip(i) => {
                    if storage.skip == 1 && i == 0 {
                        let x1: Vec<_> = storage.skip_messages.map.into_iter().map(|f| {
                            let x2: Vec<_> = f.1.into_iter().rev().collect();
                            x2
                        }).flatten().collect();
                        let len = x1.len();
                        x1.into_iter().for_each(|m| storage.messages.put(m));
                        storage.skip_messages = Messages::new();
                        storage.skip = len;
                        match tx_result.send(ResultMessage::Skip(storage.skip)) {
                            Ok(_) => {}
                            Err(_) => { return; }
                        };
                        continue;
                    } else if storage.skip > 1 && i == 0 {
                        storage.skip_messages.map.into_iter().for_each(|f| f.1.into_iter().rev().for_each(|f| storage.messages.put(f)));
                        storage.skip_messages = Messages::new()
                    }
                    storage.skip = i;
                }
                CommandMessage::SetResultSize(i) => {
                    storage.result_size = i;
                }
                CommandMessage::Clear => {
                    storage.messages.map = HashMap::new();
                    storage.messages.count = 0;
                    storage.messages.size = 0;
                    match tx_result.send(ResultMessage::Size(storage.messages.size)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                    match tx_result.send(ResultMessage::Length(storage.messages.count)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                }
                CommandMessage::FilterNotRegexes(s) => {
                    let filters: Vec<_> = s.iter()
                        .map(|v| Regex::new(format!(r#".*{}.*"#, v).as_str()).unwrap_or(Regex::new(format!(r#"{}"#, ".*").as_str()).unwrap()))
                        .collect();
                    storage.filter_not = filters;
                }
                CommandMessage::ToggleInfo() => {
                    storage.messages.info()
                }
                CommandMessage::ToggleDebug() => {
                    storage.messages.debug()
                }
                CommandMessage::ToggleWarn() => {
                    storage.messages.warn()
                }
                CommandMessage::ToggleError() => {
                    storage.messages.error()
                }
            }
        }
    });
}
