use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;

use super::hostname::{HostnameResolver, SystemHostnameResolver};
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
    HostnameResolved {
        task_id: u64,
        ip: String,
        hostname: String,
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
const HOSTNAME_WORKER_COUNT: usize = 2;

pub fn next_task_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn start_scan(cidr: &str, max_in_flight: usize) -> io::Result<ScanTask> {
    start_scan_with_probe_and_hostname_resolver(
        cidr,
        max_in_flight,
        Arc::new(SystemArpProbe),
        Arc::new(SystemHostnameResolver),
    )
}

pub fn start_scan_with_probe<P>(
    cidr: &str,
    max_in_flight: usize,
    probe: Arc<P>,
) -> io::Result<ScanTask>
where
    P: ArpProbe + Send + Sync + 'static,
{
    start_scan_with_probe_and_hostname_resolver(
        cidr,
        max_in_flight,
        probe,
        Arc::new(NoopHostnameResolver),
    )
}

pub fn start_scan_with_probe_and_hostname_resolver<P, H>(
    cidr: &str,
    max_in_flight: usize,
    probe: Arc<P>,
    hostname_resolver: Arc<H>,
) -> io::Result<ScanTask>
where
    P: ArpProbe + Send + Sync + 'static,
    H: HostnameResolver + Send + Sync + 'static,
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
    let hostname_resolver: Arc<dyn HostnameResolver + Send + Sync> = hostname_resolver;
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
            hostname_resolver,
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
enum ProbeResult {
    Completed {
        ip: String,
        result: io::Result<Option<[u8; 6]>>,
    },
    WorkerPanicked {
        ip: String,
    },
}

#[derive(Clone, Copy, Debug, Default)]
struct NoopHostnameResolver;

impl HostnameResolver for NoopHostnameResolver {
    fn resolve(&self, _ip: &str) -> Option<String> {
        None
    }
}

fn run_scan_loop<P>(
    task_id: u64,
    total_hosts: usize,
    mut hosts: super::ip_box::HostIter,
    max_in_flight: usize,
    probe: Arc<P>,
    hostname_resolver: Arc<dyn HostnameResolver + Send + Sync>,
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
    let mut pending_failure: Option<String> = None;
    let mut dispatch_tx = Some(result_tx);
    let hostname_executor = hostname_executor();

    while !dispatch_exhausted || in_flight > 0 {
        while pending_failure.is_none()
            && !dispatch_exhausted
            && in_flight < max_in_flight
            && !cancel_flag.load(Ordering::Relaxed)
        {
            let Some(ip) = hosts.next() else {
                dispatch_exhausted = true;
                break;
            };

            let probe = Arc::clone(&probe);
            let result_tx = dispatch_tx
                .as_ref()
                .expect("dispatch sender should exist while dispatch is active")
                .clone();
            in_flight += 1;

            thread::spawn(move || {
                let probe_result = match panic::catch_unwind(AssertUnwindSafe(|| probe.probe(&ip)))
                {
                    Ok(result) => ProbeResult::Completed { ip, result },
                    Err(_) => ProbeResult::WorkerPanicked { ip },
                };
                let _ = result_tx.send(probe_result);
            });
        }

        if pending_failure.is_some() || dispatch_exhausted || cancel_flag.load(Ordering::Relaxed) {
            dispatch_tx.take();
        }

        if in_flight == 0 {
            break;
        }

        let probe_result = match result_rx.recv() {
            Ok(probe_result) => probe_result,
            Err(_) => {
                pending_failure
                    .get_or_insert_with(|| "scan worker channel closed unexpectedly".to_string());
                break;
            }
        };

        in_flight -= 1;

        match probe_result {
            ProbeResult::Completed { ip, result } => {
                if pending_failure.is_some() {
                    continue;
                }

                scanned_hosts += 1;

                match result {
                    Ok(Some(mac)) => {
                        found_hosts += 1;
                        let formatted_mac = format_mac(mac);
                        let _ = event_tx.send(ScanEvent::HostFound {
                            task_id,
                            ip: ip.clone(),
                            mac: formatted_mac,
                        });
                        hostname_executor.submit(HostnameJob {
                            task_id,
                            ip,
                            hostname_resolver: Arc::clone(&hostname_resolver),
                            cancel_flag: Arc::clone(&cancel_flag),
                            event_tx: event_tx.clone(),
                        });
                    }
                    Ok(None) => {}
                    Err(err) if err.kind() == io::ErrorKind::Unsupported => {
                        pending_failure = Some(err.to_string());
                        continue;
                    }
                    Err(_) => {}
                }

                let _ = event_tx.send(ScanEvent::Progress {
                    task_id,
                    scanned_hosts,
                    total_hosts,
                });
            }
            ProbeResult::WorkerPanicked { ip } => {
                pending_failure =
                    Some(format!("probe worker panicked before sending a result for {ip}"));
            }
        }
    }

    let terminal_event = if let Some(message) = pending_failure {
        ScanEvent::Failed { task_id, message }
    } else if cancel_flag.load(Ordering::Relaxed) {
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

#[derive(Clone)]
struct HostnameJob {
    task_id: u64,
    ip: String,
    hostname_resolver: Arc<dyn HostnameResolver + Send + Sync>,
    cancel_flag: Arc<AtomicBool>,
    event_tx: mpsc::Sender<ScanEvent>,
}

struct HostnameExecutor {
    job_tx: mpsc::Sender<HostnameJob>,
}

impl HostnameExecutor {
    fn submit(&self, job: HostnameJob) {
        let _ = self.job_tx.send(job);
    }
}

fn hostname_executor() -> &'static HostnameExecutor {
    static HOSTNAME_EXECUTOR: OnceLock<HostnameExecutor> = OnceLock::new();
    HOSTNAME_EXECUTOR.get_or_init(|| {
        let (job_tx, job_rx) = mpsc::channel();
        let job_rx = Arc::new(Mutex::new(job_rx));

        for _ in 0..HOSTNAME_WORKER_COUNT {
            let job_rx = Arc::clone(&job_rx);
            thread::spawn(move || {
                while let Some(job) = recv_hostname_job(&job_rx) {
                    if job.cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let Some(hostname) = job.hostname_resolver.resolve(&job.ip) else {
                        continue;
                    };

                    if job.cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let _ = job.event_tx.send(ScanEvent::HostnameResolved {
                        task_id: job.task_id,
                        ip: job.ip,
                        hostname,
                    });
                }
            });
        }

        HostnameExecutor { job_tx }
    })
}

fn recv_hostname_job(job_rx: &Arc<Mutex<mpsc::Receiver<HostnameJob>>>) -> Option<HostnameJob> {
    job_rx
        .lock()
        .expect("hostname job receiver should not be poisoned")
        .recv()
        .ok()
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
