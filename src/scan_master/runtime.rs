use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

use super::ip_box::{host_count, host_iter};
use super::probe::{ArpProbe, SystemArpProbe};

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

pub fn start_scan(cidr: &str, max_in_flight: usize) -> io::Result<ScanTask> {
    start_scan_with_probe(cidr, max_in_flight, Arc::new(SystemArpProbe))
}

pub fn start_scan_with_probe<P>(
    cidr: &str,
    max_in_flight: usize,
    probe: Arc<P>,
) -> io::Result<ScanTask>
where
    P: ArpProbe + Send + Sync + 'static,
{
    if max_in_flight == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_in_flight must be greater than zero",
        ));
    }

    let total_hosts = host_count(cidr)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid CIDR"))?;
    let hosts = host_iter(cidr)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid CIDR"))?;

    let task_id = next_task_id();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (event_tx, event_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let cancel_for_thread = Arc::clone(&cancel_flag);

    let join_handle = thread::spawn(move || {
        let _ = event_tx.send(ScanEvent::Started {
            task_id,
            total_hosts,
        });
        run_scan_loop(
            task_id,
            total_hosts,
            hosts,
            max_in_flight,
            probe,
            cancel_for_thread,
            event_tx,
            result_tx,
            result_rx,
        );
    });

    Ok(ScanTask {
        task_id,
        events: event_rx,
        cancel_flag,
        join_handle,
    })
}

#[derive(Debug)]
struct ProbeResult {
    ip: String,
    result: io::Result<Option<[u8; 6]>>,
}

fn run_scan_loop<P>(
    task_id: u64,
    total_hosts: usize,
    mut hosts: super::ip_box::HostIter,
    max_in_flight: usize,
    probe: Arc<P>,
    cancel_flag: Arc<AtomicBool>,
    event_tx: mpsc::Sender<ScanEvent>,
    result_tx: mpsc::Sender<ProbeResult>,
    result_rx: mpsc::Receiver<ProbeResult>,
) where
    P: ArpProbe + Send + Sync + 'static,
{
    let mut scanned_hosts = 0usize;
    let mut found_hosts = 0usize;
    let mut in_flight = 0usize;
    let mut dispatch_exhausted = false;

    while !dispatch_exhausted || in_flight > 0 {
        while !dispatch_exhausted
            && in_flight < max_in_flight
            && !cancel_flag.load(Ordering::Relaxed)
        {
            let Some(ip) = hosts.next() else {
                dispatch_exhausted = true;
                break;
            };

            let probe = Arc::clone(&probe);
            let result_tx = result_tx.clone();
            in_flight += 1;

            thread::spawn(move || {
                let result = probe.probe(&ip);
                let _ = result_tx.send(ProbeResult { ip, result });
            });
        }

        if in_flight == 0 {
            break;
        }

        let probe_result = match result_rx.recv() {
            Ok(probe_result) => probe_result,
            Err(_) => {
                let _ = event_tx.send(ScanEvent::Failed {
                    task_id,
                    message: "scan worker channel closed unexpectedly".to_string(),
                });
                return;
            }
        };

        in_flight -= 1;
        scanned_hosts += 1;

        match probe_result.result {
            Ok(Some(mac)) => {
                found_hosts += 1;
                let _ = event_tx.send(ScanEvent::HostFound {
                    task_id,
                    ip: probe_result.ip,
                    mac: format_mac(mac),
                });
            }
            Ok(None) => {}
            Err(err) if err.kind() == io::ErrorKind::Unsupported => {
                let _ = event_tx.send(ScanEvent::Failed {
                    task_id,
                    message: err.to_string(),
                });
                return;
            }
            Err(_) => {}
        }

        let _ = event_tx.send(ScanEvent::Progress {
            task_id,
            scanned_hosts,
            total_hosts,
        });
    }

    let terminal_event = if cancel_flag.load(Ordering::Relaxed) {
        ScanEvent::Cancelled {
            task_id,
            scanned_hosts,
            found_hosts,
        }
    } else {
        ScanEvent::Finished {
            task_id,
            scanned_hosts,
            found_hosts,
        }
    };
    let _ = event_tx.send(terminal_event);
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
