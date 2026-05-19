use std::io;

#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::thread;
#[cfg(target_os = "windows")]
use std::thread::JoinHandle;

mod arp_core;
mod ip_box;
pub mod runtime;
pub mod probe;

pub use ip_box::{first_ip, host_count, host_iter, hosts, next_ip, HostIter, HostMaterializeError};
pub use runtime::{next_task_id, ScanEvent, ScanTask};
pub use probe::{ArpProbe, SystemArpProbe};

#[cfg(target_os = "windows")]
pub fn scan_by_arp(cidr: &str) -> io::Result<Vec<IpCheckResult>> {
    scan_by_arp_threaded(cidr, SystemArpProbe)
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

#[cfg(target_os = "windows")]
fn scan_by_arp_threaded<P>(cidr: &str, probe: P) -> io::Result<Vec<IpCheckResult>>
where
    P: ArpProbe + Send + Sync + 'static,
{
    let avaliable_node_list: Arc<Mutex<Vec<IpCheckResult>>> = Arc::new(Mutex::new(Vec::new()));
    let first_ip_addr_str = ip_box::first_ip(cidr).ok_or(io::Error::new(io::ErrorKind::InvalidInput, "No avaliable ip addr."))?;
    let mut ip_string = first_ip_addr_str;
    let mut handle_vec: Vec<JoinHandle<()>> = Vec::new();
    let probe = Arc::new(probe);
    loop {
        let avaliable_node_list_shared_clone = Arc::clone(&avaliable_node_list);
        let probe_shared_clone = Arc::clone(&probe);
        let ip_string_cur_temp = ip_string.clone();
        let handle = thread::spawn(move || {
            match check_ip_exist_with_probe(&(ip_string_cur_temp.clone()), probe_shared_clone.as_ref()) {
                Ok(result) => {
                    if result.exist {
                        let mut avaliable_node_list_guard =
                            avaliable_node_list_shared_clone.lock().unwrap();
                        avaliable_node_list_guard.push(result);
                    }
                }
                Err(e) => {
                    println!("Error: {e}");
                }
            }
        });
        handle_vec.push(handle);

        match ip_box::next_ip(cidr, &ip_string) {
            Some(ip) => {
                ip_string = ip;
            }
            None => {
                break;
            }
        }
    }
    for handle in handle_vec {
        if let Err(e) = handle.join() {
            println!("Thread handle error: {:?}", e);
        }
    }

    Ok(avaliable_node_list.lock().unwrap().to_vec())
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
