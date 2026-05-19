use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use crate::scan_master::ScanEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRow {
    pub ip: String,
    pub mac: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewSnapshot {
    pub is_scanning: bool,
    pub can_cancel: bool,
    pub status_text: String,
    pub progress_text: String,
    pub rows: Vec<ResultRow>,
}

#[derive(Debug)]
pub struct ScanUiState {
    active_task_id: Option<u64>,
    total_hosts: usize,
    scanned_hosts: usize,
    found_hosts: usize,
    status_text: String,
    results: BTreeMap<u32, ResultRow>,
    is_scanning: bool,
}

impl Default for ScanUiState {
    fn default() -> Self {
        Self {
            active_task_id: None,
            total_hosts: 0,
            scanned_hosts: 0,
            found_hosts: 0,
            status_text: "Idle".to_string(),
            results: BTreeMap::new(),
            is_scanning: false,
        }
    }
}

impl ScanUiState {
    pub fn begin_scan(&mut self, task_id: u64) {
        self.active_task_id = Some(task_id);
        self.total_hosts = 0;
        self.scanned_hosts = 0;
        self.found_hosts = 0;
        self.status_text = "Scanning".to_string();
        self.results.clear();
        self.is_scanning = true;
    }

    pub fn apply_event(&mut self, event: ScanEvent) {
        if !self.belongs_to_active_task(&event) {
            return;
        }

        match event {
            ScanEvent::Started { total_hosts, .. } => {
                self.total_hosts = total_hosts;
                self.status_text = "Scanning".to_string();
            }
            ScanEvent::Progress {
                scanned_hosts,
                total_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.total_hosts = total_hosts;
            }
            ScanEvent::HostFound { ip, mac, .. } => {
                self.results.insert(ip_to_key(&ip), ResultRow { ip, mac });
                self.found_hosts = self.results.len();
            }
            ScanEvent::Finished {
                scanned_hosts,
                found_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.found_hosts = found_hosts;
                self.status_text = format!("Finished ({} hosts)", found_hosts);
                self.is_scanning = false;
            }
            ScanEvent::Cancelled {
                scanned_hosts,
                found_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.found_hosts = found_hosts;
                self.status_text = format!("Cancelled ({} hosts)", found_hosts);
                self.is_scanning = false;
            }
            ScanEvent::Failed { message, .. } => {
                self.status_text = format!("Failed: {message}");
                self.is_scanning = false;
            }
        }
    }

    pub fn fail_to_start(&mut self, message: impl Into<String>) {
        self.status_text = format!("Failed: {}", message.into());
        self.is_scanning = false;
    }

    pub fn rows(&self) -> Vec<ResultRow> {
        self.results.values().cloned().collect()
    }

    pub fn snapshot(&self) -> ViewSnapshot {
        ViewSnapshot {
            is_scanning: self.is_scanning,
            can_cancel: self.is_scanning,
            status_text: self.status_text.clone(),
            progress_text: format!("{} / {}", self.scanned_hosts, self.total_hosts),
            rows: self.rows(),
        }
    }

    fn belongs_to_active_task(&self, event: &ScanEvent) -> bool {
        let event_task_id = match event {
            ScanEvent::Started { task_id, .. }
            | ScanEvent::Progress { task_id, .. }
            | ScanEvent::HostFound { task_id, .. }
            | ScanEvent::Finished { task_id, .. }
            | ScanEvent::Cancelled { task_id, .. }
            | ScanEvent::Failed { task_id, .. } => *task_id,
        };

        self.active_task_id == Some(event_task_id)
    }
}

fn ip_to_key(ip: &str) -> u32 {
    ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(u32::MAX)
}
