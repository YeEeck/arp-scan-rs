use std::io;
use std::sync::mpsc;
use std::thread;

mod arp_core;
mod ip_box;
pub mod probe;
pub mod runtime;

pub use ip_box::{
    first_ip, host_count, host_iter, hosts, next_ip, HostIter, HostMaterializationError,
    HostMaterializeError,
};
pub use probe::{ArpProbe, SystemArpProbe};
pub use runtime::{next_task_id, start_scan, start_scan_with_probe, ScanEvent, ScanTask};

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

#[cfg(target_os = "windows")]
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
            }
            _ => {}
        }
    }

    let _ = join_handle.join();

    if let Some(err) = terminal_error {
        return Err(err);
    }

    Ok(results)
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
