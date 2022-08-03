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
                                let vec = storage
                                    .messages
                                    .iter()
                                    .enumerate()
                                    .filter(|x| storage.filter.is_match(x.1.value.as_str()))
                                    .skip(storage.skip)
                                    .take(storage.result_size)
                                    .map(|(_i, m)| { m.clone() }).collect();
                                tx_result.send(ResultMessage::Messages(vec)).unwrap();
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
                    storage.messages.put(message);

                    match tx_result.send(ResultMessage::Size(storage.messages.size)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                    match tx_result.send(ResultMessage::Length(storage.messages.count)) {
                        Ok(_) => {}
                        Err(_) => { return; }
                    };
                }
                CommandMessage::SetSkip(i) => {
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
            }
        }
    });
}
