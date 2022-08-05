use std::{iter, mem};
use std::collections::{HashMap, VecDeque};

use crate::{Level, Message};
use crate::search_thread::merge::MergeAscending;

pub struct Messages {
    pub(crate) count: usize,
    pub(crate) size: u64,
    pub(crate) map: HashMap<String, VecDeque<Message>>,
    show_info: bool,
    show_warn: bool,
    show_debug: bool,
    show_error: bool,
}

impl Messages {
    pub(crate) fn new() -> Messages {
        Messages { count: 0, size: 0, map: HashMap::new(), show_info: true, show_warn: true, show_debug: true, show_error: true }
    }

    pub(crate) fn info(&mut self) -> () {
        self.show_info = !self.show_info;
    }
    pub(crate) fn debug(&mut self) -> () {
        self.show_debug = !self.show_debug;
    }
    pub(crate) fn warn(&mut self) -> () {
        self.show_warn = !self.show_warn;
    }
    pub(crate) fn error(&mut self) -> () {
        self.show_error = !self.show_error;
    }

    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item=&Message> + '_> {
        let x: Vec<&VecDeque<Message>> = self.map.iter().filter(|entry| {
            if self.show_debug {
                if entry.0.starts_with(Level::DEBUG.to_string().as_str()) {
                    return true;
                }
            }
            if self.show_error {
                if entry.0.starts_with(Level::ERROR.to_string().as_str()) {
                    return true;
                }
            }
            if self.show_info {
                if entry.0.starts_with(Level::INFO.to_string().as_str()) {
                    return true;
                }
            }
            if self.show_warn {
                if entry.0.starts_with(Level::WARN.to_string().as_str()) {
                    return true;
                }
            }
            return false;
        }).map(|entry| entry.1).collect::<Vec<_>>();
        if x.len() == 0 {
            return Box::new(iter::empty::<&Message>());
        }

        let mut ma: Box<dyn Iterator<Item=_>> = Box::new(x[0].iter().map(|i| i));
        for v in x.iter().skip(1) {
            ma = Box::new(MergeAscending::new(ma, v.iter().map(|i| i)));
        };
        return ma;
    }

    pub(crate) fn put(&mut self, m: Message) {
        if self.size > 1_000_000_000 {
            self.map.values_mut().for_each(|v| {
                match v.pop_back() {
                    None => {}
                    Some(m) => {
                        self.size -= m.value.len() as u64;
                        self.size -= m.system.len() as u64;
                        self.size -= mem::size_of_val(&m.timestamp) as u64;
                        self.count -= 1;
                    }
                };
            });
            self.map.shrink_to_fit();
        }
        self.count += 1;
        self.size += m.value.len() as u64;
        self.size += m.system.len() as u64;
        self.size += mem::size_of_val(&m.timestamp) as u64;
        let key = m.system.to_string();
        self.map.entry(format!("{} {}", m.level.to_string(), key)).or_insert_with(|| VecDeque::new()).push_front(m);
    }
}
