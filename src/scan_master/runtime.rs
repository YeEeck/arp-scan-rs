use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTask {
    pub id: u64,
    pub cidr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    Started { task: ScanTask },
    HostFound { task_id: u64, ip: String, mac: String },
    Finished { task_id: u64 },
}

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn new_scan_task(cidr: impl Into<String>) -> ScanTask {
    ScanTask {
        id: next_task_id(),
        cidr: cidr.into(),
    }
}
