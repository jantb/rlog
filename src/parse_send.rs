use std::str::FromStr;
use std::sync::mpsc::Sender;

use chrono::{DateTime, Utc};

use crate::{CommandMessage, Level, LogFormat, Message};

pub fn parse_and_send(x: &str, sender: &Sender<CommandMessage>) {
    let result: Result<LogFormat, _> = serde_json::from_str(x.to_string().as_str());
    let log_entry = match result {
        Ok(l) => { l }
        Err(_) => {
            //  println!("{}", e.to_string());
            return;
        }
    };
    let dt = DateTime::parse_from_rfc3339(log_entry.timestamp.as_str());
    match dt {
        Ok(time) => {
            let time = time.with_timezone(&Utc);
            let m = Message {
                timestamp: time,
                value: format!("{} {}{}", log_entry.message, log_entry.stack, log_entry.stack_trace),
                system: log_entry.application,
                level: match Level::from_str(&log_entry.level) {
                    Ok(s) => { s }
                    Err(_) => { return; }
                },
            };
            match sender.send(CommandMessage::InsertJson(m)) {
                Ok(_) => {}
                Err(_) => { return; }
            };
        }
        Err(_) => {
            //   println!("{}", e);
        }
    }
}
