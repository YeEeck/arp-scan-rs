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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    pub is_scanning: bool,
    pub can_cancel: bool,
    pub status_text: String,
    pub progress_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowMutation {
    None,
    Clear,
    Insert { index: usize, row: ResultRow },
    Update { index: usize, row: ResultRow },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub applied: bool,
    pub row_mutation: RowMutation,
}

impl ApplyOutcome {
    fn ignored() -> Self {
        Self {
            applied: false,
            row_mutation: RowMutation::None,
        }
    }

    fn applied(row_mutation: RowMutation) -> Self {
        Self {
            applied: true,
            row_mutation,
        }
    }
}

#[derive(Debug)]
pub struct ScanUiState {
    active_task_id: Option<u64>,
    total_hosts: usize,
    scanned_hosts: usize,
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
            status_text: "Idle".to_string(),
            results: BTreeMap::new(),
            is_scanning: false,
        }
    }
}

impl ScanUiState {
    pub fn begin_scan(&mut self, task_id: u64) -> RowMutation {
        self.active_task_id = Some(task_id);
        self.total_hosts = 0;
        self.scanned_hosts = 0;
        self.status_text = "Scanning".to_string();
        self.results.clear();
        self.is_scanning = true;
        RowMutation::Clear
    }

    pub fn apply_event(&mut self, event: ScanEvent) -> ApplyOutcome {
        if !self.belongs_to_active_task(&event) {
            return ApplyOutcome::ignored();
        }

        match event {
            ScanEvent::Started { total_hosts, .. } => {
                self.total_hosts = total_hosts;
                self.status_text = "Scanning".to_string();
                ApplyOutcome::applied(RowMutation::None)
            }
            ScanEvent::Progress {
                scanned_hosts,
                total_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.total_hosts = total_hosts;
                ApplyOutcome::applied(RowMutation::None)
            }
            ScanEvent::HostFound { ip, mac, .. } => {
                let key = ip_to_key(&ip);
                let index = self.results.range(..key).count();
                let row = ResultRow { ip, mac };

                let row_mutation = if self.results.insert(key, row.clone()).is_some() {
                    RowMutation::Update { index, row }
                } else {
                    RowMutation::Insert { index, row }
                };

                ApplyOutcome::applied(row_mutation)
            }
            ScanEvent::HostnameResolved { .. } => ApplyOutcome::applied(RowMutation::None),
            ScanEvent::Finished {
                scanned_hosts,
                found_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.status_text = format!("Finished ({} hosts)", found_hosts);
                self.is_scanning = false;
                ApplyOutcome::applied(RowMutation::None)
            }
            ScanEvent::Cancelled {
                scanned_hosts,
                found_hosts,
                ..
            } => {
                self.scanned_hosts = scanned_hosts;
                self.status_text = format!("Cancelled ({} hosts)", found_hosts);
                self.is_scanning = false;
                ApplyOutcome::applied(RowMutation::None)
            }
            ScanEvent::Failed { message, .. } => {
                self.status_text = format!("Failed: {message}");
                self.is_scanning = false;
                ApplyOutcome::applied(RowMutation::None)
            }
        }
    }

    pub fn fail_to_start(&mut self, message: impl Into<String>) -> RowMutation {
        self.active_task_id = None;
        self.total_hosts = 0;
        self.scanned_hosts = 0;
        self.results.clear();
        self.status_text = format!("Failed: {}", message.into());
        self.is_scanning = false;
        RowMutation::Clear
    }

    pub fn rows(&self) -> Vec<ResultRow> {
        self.results.values().cloned().collect()
    }

    pub fn view_state(&self) -> ViewState {
        ViewState {
            is_scanning: self.is_scanning,
            can_cancel: self.is_scanning,
            status_text: self.status_text.clone(),
            progress_text: format!("{} / {}", self.scanned_hosts, self.total_hosts),
        }
    }

    pub fn snapshot(&self) -> ViewSnapshot {
        let view_state = self.view_state();
        ViewSnapshot {
            is_scanning: view_state.is_scanning,
            can_cancel: view_state.can_cancel,
            status_text: view_state.status_text,
            progress_text: view_state.progress_text,
            rows: self.rows(),
        }
    }

    fn belongs_to_active_task(&self, event: &ScanEvent) -> bool {
        let event_task_id = match event {
            ScanEvent::Started { task_id, .. }
            | ScanEvent::Progress { task_id, .. }
            | ScanEvent::HostFound { task_id, .. }
            | ScanEvent::HostnameResolved { task_id, .. }
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
