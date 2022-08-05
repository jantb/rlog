use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{CommandMessage, Message, Mode, Pod, ResultMessage, Search, StatefulList};

/// App holds the state of the application
pub struct App {
    pub(crate) stops: Vec<Arc<AtomicBool>>,
    pub(crate) screen_height: usize,
    pub(crate) dropped_top_messages: usize,
    pub(crate) dropped_bottom_messages: usize,
    pub(crate) top_skip: usize,
    pub(crate) handles: Vec<JoinHandle<()>>,
    pub(crate) pods: StatefulList<Pod>,
    pub(crate) input: Vec<char>,
    pub(crate) mode: Mode,
    pub(crate) input_index: usize,
    pub(crate) messages: Vec<Message>,
    pub(crate) skip: usize,
    pub(crate) size: u64,
    pub(crate) length: usize,
    pub(crate) elapsed: Duration,
    pub(crate) window_size: u16,
    pub(crate) tx: Sender<CommandMessage>,
    pub(crate) rx_result: Receiver<ResultMessage>,
}


impl App {
    pub fn default(tx: Sender<CommandMessage>, rx_result: Receiver<ResultMessage>) -> App {
        App {
            dropped_top_messages: 0,
            dropped_bottom_messages: 0,
            stops: Vec::new(),
            handles: Vec::new(),
            pods: StatefulList::with_items(vec![]),
            mode: Search,
            input: Vec::new(),
            input_index: 0,
            messages: Vec::new(),
            skip: 0,
            length: 0,
            size: 0,
            elapsed: Duration::from_micros(0),
            window_size: 0,
            tx,
            rx_result,
            screen_height: 0,
            top_skip: 0,
        }
    }
}
