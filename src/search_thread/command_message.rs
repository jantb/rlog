pub enum CommandMessage {
    FilterRegex(String),
    InsertJson(String),
    SetSkip(usize),
    SetResultSize(usize),
    Exit,
}
