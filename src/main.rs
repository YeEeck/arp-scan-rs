use std::io;

mod arp_core;

// 使用示例
fn main() {
    let ip_str = "192.168.1.195";
    match check_ip_exist(ip_str) {
        Ok(result) => {
            if result.exist {
                print!("{} --- {}", result.ip, result.mac);
            }
        }
        Err(e) => {
            print!("Error: {e}");
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
