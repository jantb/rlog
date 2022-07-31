use crate::Message;

pub enum CommandMessage {
    FilterRegex(String),
    InsertJson(Message),
    SetSkip(usize),
    SetResultSize(usize),
    Exit,
}
