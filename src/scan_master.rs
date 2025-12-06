use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

mod arp_core;
mod ip_box;

pub fn scan_by_arp(cidr: &str) -> io::Result<Vec<IpCheckResult>> {
    let avaliable_node_list: Arc<Mutex<Vec<IpCheckResult>>> = Arc::new(Mutex::new(Vec::new()));
    let first_ip_addr_str = ip_box::first_ip(cidr).ok_or(io::Error::new(io::ErrorKind::InvalidInput, "No avaliable ip addr."))?;
    let mut ip_string = first_ip_addr_str;
    let mut handle_vec: Vec<JoinHandle<()>> = Vec::new();
    loop {
        //println!("current_ip: {}", ip_string);
        let avaliable_node_list_shared_clone = Arc::clone(&avaliable_node_list);
        let ip_string_cur_temp = ip_string.clone();
        let handle = thread::spawn(
            move || match check_ip_exist(&(ip_string_cur_temp.clone())) {
                Ok(result) => {
                    if result.exist {
                        let mut avaliable_node_list_guard =
                            avaliable_node_list_shared_clone.lock().unwrap();
                        avaliable_node_list_guard.push(result);
                        //println!("{} --- {}", result.ip, result.mac);
                    }
                }
                Err(e) => {
                    println!("Error: {e}");
                }
            },
        );
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

#[derive(Clone)]
pub struct IpCheckResult {
    pub ip: String,
    pub mac: String,
    pub exist: bool,
}

fn check_ip_exist(ip_str: &str) -> io::Result<IpCheckResult> {
    let mut result = IpCheckResult {
        ip: ip_str.to_string(),
        mac: String::new(),
        exist: false,
    };
    let ip_addr = arp_core::parse_ip(ip_str)?;
    //println!("目标 IP: {} (0x{:08X})", ip_str, ip_addr);

    if let Ok(mac) = arp_core::get_mac_address(ip_addr) {
        result.mac = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        )
        .to_string();
        result.exist = true;
    }

    Ok(result)
}
