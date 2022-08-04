use std::{iter, mem};
use std::collections::{HashMap, VecDeque};

use crate::Message;
use crate::search_thread::merge::MergeAscending;

pub struct Messages {
    pub(crate) count: usize,
    pub(crate) size: u64,
    pub(crate) map: HashMap<String, VecDeque<Message>>,
}

impl Messages {
    pub(crate) fn new() -> Messages {
        Messages { count: 0, size: 0, map: HashMap::new() }
    }
    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item=&Message> + '_> {
        let x: Vec<&VecDeque<Message>> = self.map.values().into_iter().collect::<Vec<_>>();
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
        self.map.entry(m.system.to_string()).or_insert_with(|| VecDeque::new()).push_front(m);
    }
}
