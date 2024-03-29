use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{CommandMessage, Message, Mode, Pod, ResultMessage, Search, StatefulList};

/// App holds the state of the application
pub struct App {
    pub(crate) show_info: bool,
    pub(crate) show_warn: bool,
    pub(crate) show_debug: bool,
    pub(crate) show_error: bool,
    pub(crate) wrap: bool,
    pub(crate) stops: Vec<Arc<AtomicBool>>,
    pub(crate) dropped_bottom_messages: usize,
    pub(crate) last_message_height: usize,
    pub(crate) just_skipped: bool,
    pub(crate) just_skipped_bottom: bool,
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
            show_info: true,
            show_warn: true,
            show_debug: true,
            show_error: true,
            dropped_bottom_messages: 0,
            last_message_height: 0,
            just_skipped: false,
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
            wrap: true,
            just_skipped_bottom: false,
        }
    }
}
