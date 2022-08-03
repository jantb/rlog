use std::cmp::Ordering;

use chrono::{DateTime, Utc};

use crate::Level;

#[derive(PartialEq, Eq, PartialOrd, Clone)]
pub struct Message {
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) system: String,
    pub(crate) level: Level,
    pub(crate) value: String,
}

impl Ord for Message {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}
