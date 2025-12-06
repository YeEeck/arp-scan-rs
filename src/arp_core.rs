use std::ffi::CString;
use std::io;

// Windows 错误码常量
const NO_ERROR: u32 = 0;

// 链接到 Iphlpapi.dll
#[link(name = "Iphlpapi")]
unsafe extern "system" {
    /// 发送 ARP 请求获取指定 IP 的物理地址
    /// 
    /// # 参数
    /// - `destIp`: 目标 IPv4 地址（网络字节序）
    /// - `srcIp`: 源 IPv4 地址（可为 0）
    /// - `macAddr`: 指向至少 6 字节的缓冲区接收 MAC 地址
    /// - `phyAddrLen`: 输入缓冲区长度，输出实际地址长度
    /// 
    /// # 返回值
    /// 成功返回 NO_ERROR (0)，失败返回错误码
    fn SendARP(destIp: u32, srcIp: u32, macAddr: *mut u8, phyAddrLen: *mut u32) -> u32;
}

// 链接到 Ws2_32.dll
#[link(name = "Ws2_32")]
unsafe extern "system" {
    /// 将点分十进制 IP 字符串转换为网络字节序的 u32
    /// 
    /// # 返回值
    /// 成功返回 IP 地址，失败返回 INADDR_NONE (0xFFFFFFFF)
    fn inet_addr(ip: *const i8) -> u32;
}

/// 将 IP 字符串转换为 u32（网络字节序）
pub fn parse_ip(ip_str: &str) -> io::Result<u32> {
    let c_str = CString::new(ip_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    
    let addr = unsafe { inet_addr(c_str.as_ptr()) };
    
    // inet_addr 返回 INADDR_NONE 表示失败
    if addr == u32::MAX {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid IP address format"
        ))
    } else {
        Ok(addr)
    }
}

/// 获取指定 IP 的 MAC 地址
pub fn get_mac_address(ip: u32) -> io::Result<[u8; 6]> {
    let mut mac_addr = [0u8; 6];
    let mut phy_addr_len = 6u32;
    
    let result = unsafe {
        SendARP(
            ip,
            0,  // srcIp 参数可为 0
            mac_addr.as_mut_ptr(),
            &mut phy_addr_len,
        )
    };
    
    if result == NO_ERROR {
        Ok(mac_addr)
    } else {
        Err(io::Error::from_raw_os_error(result as i32))
    }
}