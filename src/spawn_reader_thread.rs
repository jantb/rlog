use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use crate::{App, CommandMessage, OtherOrdering, parse_and_send};

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
                let should_i_stop2 = should_i_stop.clone();
                spawn(move || {
                    let mut reader = BufReader::new(l);
                    let mut buf = String::new();
                    while !should_i_stop.load(OtherOrdering::SeqCst) {
                        let result = reader.read_line(&mut buf);
                        match result {
                            Ok(result) => {
                                if result == 0 {
                                    thread::sleep(Duration::from_millis(100));
                                    continue;
                                }
                                parse_and_send(&buf, &sender);
                                buf.clear()
                            }
                            Err(_) => {}
                        }
                    }
                });
                while !should_i_stop2.load(OtherOrdering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                }
                child.kill().unwrap()
            }
        }
    });
}


pub fn clean_up_threads(app: &mut App) {
    app.stops.iter().for_each(|s| { s.store(true, OtherOrdering::SeqCst) });
    while app.handles.len() > 0 {
        let handle = app.handles.remove(0); // moves it into cur_thread
        handle.join().unwrap();
    }
}
