use std::io;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// 将 IP 字符串转换为 u32（网络字节序）
pub fn parse_ip(ip_str: &str) -> io::Result<u32> {
    let addr = Ipv4Addr::from_str(ip_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    Ok(u32::from(addr))
}

/// 获取指定 IP 的 MAC 地址
#[cfg(target_os = "windows")]
pub fn get_mac_address(ip: u32) -> io::Result<[u8; 6]> {
    const NO_ERROR: u32 = 0;

    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        fn SendARP(destIp: u32, srcIp: u32, macAddr: *mut u8, phyAddrLen: *mut u32) -> u32;
    }

    let mut mac_addr = [0u8; 6];
    let mut phy_addr_len = 6u32;

    let result = unsafe { SendARP(ip, 0, mac_addr.as_mut_ptr(), &mut phy_addr_len) };

    if result == NO_ERROR {
        Ok(mac_addr)
    } else {
        Err(io::Error::from_raw_os_error(result as i32))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_mac_address(_ip: u32) -> io::Result<[u8; 6]> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "ARP probing is only supported on Windows",
    ))
}
