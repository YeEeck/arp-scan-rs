use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Debug)]
pub struct ScanTask {
    pub task_id: u64,
    pub events: Receiver<ScanEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub join_handle: JoinHandle<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    Started {
        task_id: u64,
        total_hosts: usize,
    },
    Progress {
        task_id: u64,
        scanned_hosts: usize,
        total_hosts: usize,
    },
    HostFound {
        task_id: u64,
        ip: String,
        mac: String,
    },
    Finished {
        task_id: u64,
        scanned_hosts: usize,
        found_hosts: usize,
    },
    Cancelled {
        task_id: u64,
        scanned_hosts: usize,
        found_hosts: usize,
    },
    Failed {
        task_id: u64,
        message: String,
    },
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}
