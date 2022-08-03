use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use crate::{CommandMessage, OtherOrdering, parse_and_send};

pub fn spawn_reader_thread(name: String, sender: Sender<CommandMessage>, should_i_stop: Arc<AtomicBool>) -> JoinHandle<()> {
    return spawn(move || {
        let mut child = Command::new("oc")
            .stdout(Stdio::piped())
            .arg("logs")
            .arg("-f")
            .arg("--since=200h")
            .arg(name)
            .spawn().expect("Unable to start tool");
        match child.stdout.take() {
            None => {}
            Some(l) => {
                let mut reader = BufReader::new(l);
                let mut buf = String::new();
                while !should_i_stop.load(OtherOrdering::SeqCst) {
                    let result = reader.read_line(&mut buf).expect("Unable to read");
                    if result == 0 {
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    parse_and_send(&buf, &sender);
                    buf.clear()
                }
                child.kill().unwrap()
            }
        }
    });
}
