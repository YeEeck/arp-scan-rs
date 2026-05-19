use std::io;

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

#[cfg(target_os = "windows")]
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
    P: ArpProbe,
{
    let mut avaliable_node_list: Vec<IpCheckResult> = Vec::new();
    let first_ip_addr_str = ip_box::first_ip(cidr).ok_or(io::Error::new(io::ErrorKind::InvalidInput, "No avaliable ip addr."))?;
    let mut ip_string = first_ip_addr_str;
    loop {
        let result = check_ip_exist_with_probe(&ip_string, &probe)?;
        if result.exist {
            avaliable_node_list.push(result);
        }

        match ip_box::next_ip(cidr, &ip_string) {
            Some(ip) => {
                ip_string = ip;
            }
            None => break,
        }
    }

    Ok(avaliable_node_list)
}

#[derive(Clone, Debug)]
pub struct IpCheckResult {
    pub ip: String,
    pub mac: String,
    pub exist: bool,
}

fn check_ip_exist_with_probe<P: ArpProbe + ?Sized>(ip_str: &str, probe: &P) -> io::Result<IpCheckResult> {
    let mut result = IpCheckResult {
        ip: ip_str.to_string(),
        mac: String::new(),
        exist: false,
    };
    if let Some(mac) = probe.probe(ip_str)? {
        result.mac = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
        result.exist = true;
    }

    Ok(result)
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
