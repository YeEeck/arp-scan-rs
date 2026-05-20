use std::io;
use std::sync::mpsc;
use std::thread;

mod hostname;
mod arp_core;
mod ip_box;
pub mod probe;
pub mod runtime;

pub use hostname::{HostnameResolver, SystemHostnameResolver};
pub use ip_box::{
    first_ip, host_count, host_iter, hosts, next_ip, HostIter, HostMaterializationError,
    HostMaterializeError,
};
pub use arp_core::parse_ip;
pub use probe::{ArpProbe, SystemArpProbe};
pub use runtime::{
    next_task_id, start_scan, start_scan_with_probe,
    start_scan_with_probe_and_hostname_resolver, ScanEvent, ScanTask,
};

const DEFAULT_MAX_IN_FLIGHT: usize = 256;

#[cfg(target_os = "windows")]
pub fn scan_by_arp(cidr: &str) -> io::Result<Vec<IpCheckResult>> {
    collect_scan_results(start_scan(cidr, DEFAULT_MAX_IN_FLIGHT)?)
}

#[cfg(not(target_os = "windows"))]
pub fn scan_by_arp(_cidr: &str) -> io::Result<Vec<IpCheckResult>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "ARP scanning is only supported on Windows",
    ))
}

pub fn scan_by_arp_with_probe<P>(cidr: &str, probe: P) -> io::Result<Vec<IpCheckResult>>
where
    P: ArpProbe + Sync,
{
    let mut hosts = host_iter(cidr)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid CIDR"))?;
    let (result_tx, result_rx) = mpsc::channel();
    let probe = &probe;
    let mut in_flight = 0usize;
    let mut dispatch_index = 0usize;
    let mut dispatch_exhausted = false;
    let mut terminal_error = None;
    let mut results = Vec::new();

    thread::scope(|scope| {
        while !dispatch_exhausted || in_flight > 0 {
            while terminal_error.is_none()
                && !dispatch_exhausted
                && in_flight < DEFAULT_MAX_IN_FLIGHT
            {
                let Some(ip) = hosts.next() else {
                    dispatch_exhausted = true;
                    break;
                };

                let result_tx = result_tx.clone();
                let sequence = dispatch_index;
                dispatch_index += 1;
                in_flight += 1;

                scope.spawn(move || {
                    let result = probe.probe(&ip).map(|maybe_mac| maybe_mac.map(format_mac));
                    let _ = result_tx.send((sequence, ip, result));
                });
            }

            if in_flight == 0 {
                break;
            }

            let probe_result = match result_rx.recv() {
                Ok(probe_result) => probe_result,
                Err(_) => {
                    terminal_error.get_or_insert_with(|| {
                        io::Error::other("probe worker channel closed unexpectedly")
                    });
                    break;
                }
            };

            in_flight -= 1;

            let (sequence, ip, result) = probe_result;
            match result {
                Ok(Some(mac)) if terminal_error.is_none() => {
                    results.push((
                        sequence,
                        IpCheckResult {
                            ip,
                            mac,
                            exist: true,
                        },
                    ));
                }
                Ok(Some(_)) | Ok(None) => {}
                Err(err) => {
                    terminal_error.get_or_insert(err);
                }
            }
        }
    });

    if let Some(err) = terminal_error {
        return Err(err);
    }

    results.sort_by_key(|(sequence, _)| *sequence);
    Ok(results.into_iter().map(|(_, result)| result).collect())
}

#[derive(Clone, Debug)]
pub struct IpCheckResult {
    pub ip: String,
    pub mac: String,
    pub exist: bool,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn collect_scan_results(task: ScanTask) -> io::Result<Vec<IpCheckResult>> {
    let ScanTask {
        events,
        join_handle,
        ..
    } = task;

    let mut results = Vec::new();
    let mut terminal_error = None;

    for event in events {
        match event {
            ScanEvent::HostFound { ip, mac, .. } => {
                results.push(IpCheckResult {
                    ip,
                    mac,
                    exist: true,
                });
            }
            ScanEvent::Failed { message, .. } => {
                terminal_error = Some(io::Error::other(message));
                break;
            }
            ScanEvent::Finished { .. } | ScanEvent::Cancelled { .. } => break,
            _ => {}
        }
    }

    let _ = join_handle.join();

    if let Some(err) = terminal_error {
        return Err(err);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::collect_scan_results;
    use crate::scan_master::{
        start_scan_with_probe_and_hostname_resolver, ArpProbe, HostnameResolver, ScanEvent,
    };
    use std::io;
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[derive(Clone)]
    struct QuickProbe;

    impl ArpProbe for QuickProbe {
        fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
            if ip.ends_with(".1") {
                Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]))
            } else {
                Ok(None)
            }
        }
    }

    #[derive(Clone)]
    struct BlockingHostnameResolver {
        started_tx: mpsc::Sender<String>,
        permits: Arc<Mutex<mpsc::Receiver<()>>>,
    }

    impl HostnameResolver for BlockingHostnameResolver {
        fn resolve(&self, ip: &str) -> Option<String> {
            self.started_tx
                .send(ip.to_string())
                .expect("test should observe hostname resolution start");

            self.permits
                .lock()
                .expect("hostname permits should not be poisoned")
                .recv_timeout(Duration::from_secs(2))
                .ok()?;

            Some("printer.lan".to_string())
        }
    }

    #[test]
    fn collect_scan_results_stops_at_terminal_event_without_waiting_for_hostname_lookup() {
        let (started_tx, started_rx) = mpsc::channel();
        let (_permit_tx, permit_rx) = mpsc::channel();
        let resolver = BlockingHostnameResolver {
            started_tx,
            permits: Arc::new(Mutex::new(permit_rx)),
        };

        let task = start_scan_with_probe_and_hostname_resolver(
            "192.168.1.1/32",
            1,
            Arc::new(QuickProbe),
            Arc::new(resolver),
        )
        .expect("scan task should start");

        let (result_tx, result_rx) = mpsc::channel();
        thread::spawn(move || {
            let result = collect_scan_results(task);
            result_tx
                .send(result)
                .expect("test should observe collection completion");
        });

        assert_eq!(
            started_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("hostname lookup should have started"),
            "192.168.1.1"
        );

        let results = result_rx
            .recv_timeout(Duration::from_millis(200))
            .expect("collection should finish without waiting for reverse DNS");
        let results = results.expect("collection should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ip, "192.168.1.1");
        assert_eq!(results[0].mac, "AA:BB:CC:DD:EE:01");
        assert!(results[0].exist);
    }

    #[test]
    fn collect_scan_results_ignores_hostname_updates() {
        let (event_tx, event_rx) = mpsc::channel();
        let join_handle = thread::spawn(move || {
            let _ = event_tx.send(ScanEvent::Started {
                task_id: 11,
                total_hosts: 1,
            });
            let _ = event_tx.send(ScanEvent::HostFound {
                task_id: 11,
                ip: "192.168.1.1".into(),
                mac: "AA:BB:CC:DD:EE:01".into(),
            });
            let _ = event_tx.send(ScanEvent::Progress {
                task_id: 11,
                scanned_hosts: 1,
                total_hosts: 1,
            });
            let _ = event_tx.send(ScanEvent::Finished {
                task_id: 11,
                scanned_hosts: 1,
                found_hosts: 1,
            });
            let _ = event_tx.send(ScanEvent::HostnameResolved {
                task_id: 11,
                ip: "192.168.1.1".into(),
                hostname: "printer.lan".into(),
            });
        });

        let results = collect_scan_results(crate::scan_master::ScanTask {
            task_id: 11,
            events: event_rx,
            cancel_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            join_handle,
        })
        .expect("collection should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ip, "192.168.1.1");
    }
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
