use std::thread::JoinHandle;
use std::{io, thread};

use crate::ip_box::next_ip;

mod arp_core;
mod ip_box;

// 使用示例
fn main() {
    let cidr = "192.168.1.0/24";
    let first_ip_addr_str = ip_box::first_ip(cidr).expect("No avalivable addr.");
    let mut ip_string = first_ip_addr_str;
    let mut handle_vec: Vec<JoinHandle<()>> = Vec::new();
    loop {
        //println!("current_ip: {}", ip_string);
        let ip_string_cur_temp = ip_string.clone();
        let handle = thread::spawn(
            move || match check_ip_exist(&(ip_string_cur_temp.clone())) {
                Ok(result) => {
                    if result.exist {
                        println!("{} --- {}", result.ip, result.mac);
                    }
                }
                Err(e) => {
                    println!("Error: {e}");
                }
            },
        );
        handle_vec.push(handle);

        match next_ip(cidr, &ip_string) {
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
}

struct IpCheckResult {
    ip: String,
    mac: String,
    exist: bool,
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
