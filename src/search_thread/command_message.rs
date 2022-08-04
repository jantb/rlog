use crate::Message;

pub enum CommandMessage {
    FilterRegex(String),
    FilterNotRegexes(Vec<String>),
    InsertJson(Message),
    SetSkip(usize),
    SetResultSize(usize),
    Clear,
    Exit,
}
