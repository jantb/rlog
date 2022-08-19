use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, PartialOrd, Copy, Clone, Deserialize, Serialize)]
pub enum Level {
    INFO,
    WARN,
    ERROR,
    DEBUG,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Level::INFO => { write!(f, "INFO") }
            Level::WARN => { write!(f, "WARN") }
            Level::ERROR => { write!(f, "ERROR") }
            Level::DEBUG => { write!(f, "DEBUG") }
        }
    }
}

impl FromStr for Level {
    type Err = ();

    fn from_str(input: &str) -> Result<Level, Self::Err> {
        match input {
            "INFO" => Ok(Level::INFO),
            "WARN" => Ok(Level::WARN),
            "DEBUG" => Ok(Level::DEBUG),
            "ERROR" => Ok(Level::ERROR),
            _ => Err(()),
        }
    }
}
