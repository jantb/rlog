use std::time::Duration;

use crate::Message;

pub enum ResultMessage {
    Messages(Vec<Message>),
    Elapsed(Duration),
    Size(u64),
    Length(usize),
    Skip(usize),
}
