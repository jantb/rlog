use crate::Message;

pub enum CommandMessage {
    FilterRegex(String),
    FilterNotRegexes(Vec<String>),
    InsertJson(Message),
    ToggleInfo(),
    ToggleDebug(),
    ToggleWarn(),
    ToggleError(),
    SetSkip(usize),
    SetResultSize(usize),
    Clear,
    Exit,
}
