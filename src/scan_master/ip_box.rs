use std::net::Ipv4Addr;

/// 解析CIDR字符串，返回网络地址、广播地址和前缀长度
fn parse_cidr(cidr: &str) -> Option<(u32, u32, u8)> {
    // 分割CIDR为IP和前缀长度
    let (net_str, prefix_len_str) = cidr.split_once('/')?;
    let net_ip: Ipv4Addr = net_str.parse().ok()?;
    let prefix_len: u8 = prefix_len_str.parse().ok()?;
    
    // 验证前缀长度有效范围
    if prefix_len > 32 {
        return None;
    }
    
    // 将IPv4地址转换为32位整数
    let net_u32 = u32::from(net_ip);
    
    // 计算子网掩码
    let mask = if prefix_len == 0 {
        0
    } else {
        (!0u32) << (32 - prefix_len)
    };
    
    // 计算网络地址和广播地址
    let network = net_u32 & mask;
    let broadcast = network | !mask;
    
    Some((network, broadcast, prefix_len))
}

/// 获取指定CIDR网段中的第一个可用IP地址
///
/// 根据CIDR前缀长度确定可用IP范围：
/// - `/0` 到 `/30`: 排除网络地址，返回网络地址+1
/// - `/31`: 点对点网络，两个IP都可用，返回第一个IP（网络地址）
/// - `/32`: 主机路由，返回唯一的IP地址
///
/// # 参数
/// - `cidr`: CIDR格式的网段字符串，如 "192.168.1.0/24"
///
/// # 返回值
/// - `Some(String)`: 第一个可用IP地址的字符串表示
/// - `None`: 如果CIDR格式无效或前缀长度超出范围
///
/// # 示例
/// ```
/// // 标准/24网络，跳过网络地址
/// assert_eq!(first_ip("192.168.1.0/24"), Some("192.168.1.1".to_string()));
///
/// // /31点对点网络，网络地址可用
/// assert_eq!(first_ip("192.168.1.0/31"), Some("192.168.1.0".to_string()));
///
/// // /32主机路由，只有一个IP
/// assert_eq!(first_ip("192.168.1.5/32"), Some("192.168.1.5".to_string()));
/// ```
pub fn first_ip(cidr: &str) -> Option<String> {
    let (network, _broadcast, prefix_len) = parse_cidr(cidr)?;
    
    // 根据前缀长度确定第一个可用IP
    let first_usable = match prefix_len {
        0..=30 => network + 1,      // 传统网络，跳过网络地址
        31 => network,              // /31点对点网络，两个IP都可用
        32 => network,              // /32主机路由，只有这一个IP
        _ => return None,
    };
    
    Some(Ipv4Addr::from(first_usable).to_string())
}

/// 获取指定网段中当前IP的下一个可用IP地址
///
/// 根据CIDR规则：
/// - `/0` 到 `/30`: 排除网络地址和广播地址
/// - `/31`: 点对点网络，两个IP都可用
/// - `/32`: 主机路由，只有一个IP
///
/// # 参数
/// - `cidr`: CIDR格式的网段，如 "192.168.1.0/24"
/// - `current_ip`: 当前IP地址，如 "192.168.1.123"
///
/// # 返回值
/// - `Some(String)`: 下一个IP地址字符串
/// - `None`: 如果没有下一个可用IP（当前IP已是最后一个、不在网段内或输入无效）
///
/// # 示例
/// ```
/// // 基本用例
/// assert_eq!(
///     next_ip("192.168.1.0/24", "192.168.1.123"),
///     Some("192.168.1.124".to_string())
/// );
///
/// // 最后一个可用IP
/// assert_eq!(
///     next_ip("192.168.1.0/24", "192.168.1.254"),
///     None
/// );
///
/// // /31点对点网络
/// assert_eq!(
///     next_ip("192.168.1.0/31", "192.168.1.0"),
///     Some("192.168.1.1".to_string())
/// );
/// ```
pub fn next_ip(cidr: &str, current_ip: &str) -> Option<String> {
    let (network, broadcast, prefix_len) = parse_cidr(cidr)?;
    
    // 根据前缀长度确定可用IP范围
    let (first_usable, last_usable) = match prefix_len {
        0..=30 => (network + 1, broadcast - 1), // 传统网络
        31 => (network, broadcast),             // /31点对点
        32 => (network, network),               // /32主机路由
        _ => return None,
    };
    
    // 解析并验证当前IP
    let current: Ipv4Addr = current_ip.parse().ok()?;
    let current_u32 = u32::from(current);
    
    // 检查当前IP是否在网段范围内
    if current_u32 < network || current_u32 > broadcast {
        return None;
    }
    
    // 计算下一个IP
    let next_u32 = current_u32.checked_add(1)?;
    
    // 检查下一个IP是否在可用范围内
    if next_u32 >= first_usable && next_u32 <= last_usable {
        Some(Ipv4Addr::from(next_u32).to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_first_ip_basic() {
        assert_eq!(first_ip("192.168.1.0/24"), Some("192.168.1.1".to_string()));
        assert_eq!(first_ip("192.168.1.0/16"), Some("192.168.0.1".to_string()));
        assert_eq!(first_ip("192.168.1.0/8"), Some("192.0.0.1".to_string()));
    }
    
    #[test]
    fn test_first_ip_31() {
        assert_eq!(first_ip("192.168.1.0/31"), Some("192.168.1.0".to_string()));
        assert_eq!(first_ip("192.168.1.2/31"), Some("192.168.1.2".to_string()));
    }
    
    #[test]
    fn test_first_ip_32() {
        assert_eq!(first_ip("192.168.1.5/32"), Some("192.168.1.5".to_string()));
        assert_eq!(first_ip("10.0.0.1/32"), Some("10.0.0.1".to_string()));
    }
    
    #[test]
    fn test_first_ip_edge_cases() {
        assert_eq!(first_ip("0.0.0.0/0"), Some("0.0.0.1".to_string()));
        assert_eq!(first_ip("255.255.255.248/29"), Some("255.255.255.249".to_string()));
    }
    
    #[test]
    fn test_first_ip_invalid() {
        assert_eq!(first_ip("invalid"), None);
        assert_eq!(first_ip("192.168.1.0/33"), None);
        assert_eq!(first_ip("192.168.1.0/abc"), None);
        assert_eq!(first_ip("256.1.1.0/24"), None);
        assert_eq!(first_ip("192.168.1.0/"), None);
        assert_eq!(first_ip(""), None);
    }
    
    #[test]
    fn test_next_ip_basic() {
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.123"),
            Some("192.168.1.124".to_string())
        );
    }
    
    #[test]
    fn test_next_ip_first_usable() {
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.1"),
            Some("192.168.1.2".to_string())
        );
    }
    
    #[test]
    fn test_next_ip_last_usable() {
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.254"),
            None
        );
    }
    
    #[test]
    fn test_next_ip_network_address() {
        // 从网络地址下一个开始
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.0"),
            Some("192.168.1.1".to_string())
        );
    }
    
    #[test]
    fn test_next_ip_broadcast() {
        // 广播地址没有下一个
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.255"),
            None
        );
    }
    
    #[test]
    fn test_next_ip_31() {
        // /31点对点网络，两个IP都可用
        assert_eq!(
            next_ip("192.168.1.0/31", "192.168.1.0"),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(
            next_ip("192.168.1.0/31", "192.168.1.1"),
            None // 已是最后一个
        );
    }
    
    #[test]
    fn test_next_ip_32() {
        // /32主机路由，只有一个IP
        assert_eq!(
            next_ip("192.168.1.5/32", "192.168.1.5"),
            None
        );
    }
    
    #[test]
    fn test_next_ip_large_prefix() {
        // /8大网段，跨段
        assert_eq!(
            next_ip("10.0.0.0/8", "10.0.0.255"),
            Some("10.0.1.0".to_string())
        );
        assert_eq!(
            next_ip("10.0.0.0/8", "10.255.255.254"),
            None
        );
    }
    
    #[test]
    fn test_next_ip_out_of_range() {
        // 当前IP不在网段内
        assert_eq!(
            next_ip("192.168.1.0/24", "10.0.0.1"),
            None
        );
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.2.1"),
            None
        );
    }
    
    #[test]
    fn test_next_ip_invalid_input() {
        // 无效输入
        assert_eq!(next_ip("invalid", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/33", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/abc", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/24", "invalid"), None);
        assert_eq!(next_ip("192.168.1.0/24", "256.1.1.1"), None);
    }
    
    #[test]
    fn test_both_functions_together() {
        // 测试两个函数配合使用
        let cidr = "172.16.0.0/22";
        let first = first_ip(cidr).unwrap();
        assert_eq!(first, "172.16.0.1");
        
        let second = next_ip(cidr, &first).unwrap();
        assert_eq!(second, "172.16.0.2");
        
        let third = next_ip(cidr, &second).unwrap();
        assert_eq!(third, "172.16.0.3");
    }
}
