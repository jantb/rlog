use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::formats::Flexible;
use serde_with::TimestampMilliSeconds;

use crate::Level;

#[serde_with::serde_as]
#[derive(PartialEq, Eq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Message {
    #[serde_as(as = "TimestampMilliSeconds<String, Flexible>")]
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
