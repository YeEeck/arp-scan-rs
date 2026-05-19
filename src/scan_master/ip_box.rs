use std::net::Ipv4Addr;

const MAX_MATERIALIZED_HOSTS: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostMaterializeError {
    InvalidCidr,
    TooLarge { total_hosts: usize, max_hosts: usize },
}

fn parse_cidr(cidr: &str) -> Option<(u32, u8)> {
    let (net_str, prefix_len_str) = cidr.split_once('/')?;
    let net_ip: Ipv4Addr = net_str.parse().ok()?;
    let prefix_len: u8 = prefix_len_str.parse().ok()?;

    if prefix_len > 32 {
        return None;
    }

    Some((u32::from(net_ip), prefix_len))
}

fn cidr_bounds(cidr: &str) -> Option<(u32, u32, u8)> {
    let (net_u32, prefix_len) = parse_cidr(cidr)?;

    let mask = if prefix_len == 0 {
        0
    } else {
        (!0u32) << (32 - prefix_len)
    };

    let network = net_u32 & mask;
    let broadcast = network | !mask;

    Some((network, broadcast, prefix_len))
}

fn usable_range(cidr: &str) -> Option<(u32, u32)> {
    let (network, broadcast, prefix_len) = cidr_bounds(cidr)?;

    match prefix_len {
        0..=30 => Some((network + 1, broadcast - 1)),
        31 => Some((network, broadcast)),
        32 => Some((network, network)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct HostIter {
    next: u32,
    end: u32,
    done: bool,
}

impl Iterator for HostIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let current = self.next;
        if current > self.end {
            self.done = true;
            return None;
        }

        if current == self.end {
            self.done = true;
        } else {
            self.next = current.checked_add(1)?;
        }

        Some(Ipv4Addr::from(current).to_string())
    }
}

/// Returns the number of usable hosts in a CIDR range.
///
/// This follows the standard `/31` and `/32` rules used elsewhere in this
/// module.
pub fn host_count(cidr: &str) -> Option<usize> {
    let (first_usable, last_usable) = usable_range(cidr)?;
    Some((last_usable - first_usable + 1) as usize)
}

/// Materializes all usable hosts for a CIDR range when the range is small enough.
///
/// Large valid CIDRs are intentionally bounded; for streaming access use
/// [`host_iter()`] instead.
pub fn hosts(cidr: &str) -> Result<Vec<String>, HostMaterializeError> {
    let total_hosts = host_count(cidr).ok_or(HostMaterializeError::InvalidCidr)?;
    if total_hosts > MAX_MATERIALIZED_HOSTS {
        return Err(HostMaterializeError::TooLarge {
            total_hosts,
            max_hosts: MAX_MATERIALIZED_HOSTS,
        });
    }

    host_iter(cidr)
        .map(|iter| iter.collect())
        .ok_or(HostMaterializeError::InvalidCidr)
}

/// Returns a lazy iterator over all usable hosts in a CIDR range.
pub fn host_iter(cidr: &str) -> Option<HostIter> {
    let (first_usable, last_usable) = usable_range(cidr)?;
    Some(HostIter {
        next: first_usable,
        end: last_usable,
        done: false,
    })
}

pub fn first_ip(cidr: &str) -> Option<String> {
    let (first_usable, _) = usable_range(cidr)?;
    Some(Ipv4Addr::from(first_usable).to_string())
}

pub fn next_ip(cidr: &str, current_ip: &str) -> Option<String> {
    let (network, broadcast, _) = cidr_bounds(cidr)?;
    let (first_usable, last_usable) = usable_range(cidr)?;

    let current: Ipv4Addr = current_ip.parse().ok()?;
    let current_u32 = u32::from(current);

    if current_u32 < network || current_u32 > broadcast {
        return None;
    }

    let next_u32 = current_u32.checked_add(1)?;

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
        assert_eq!(next_ip("192.168.1.0/24", "192.168.1.254"), None);
    }

    #[test]
    fn test_next_ip_network_address() {
        assert_eq!(
            next_ip("192.168.1.0/24", "192.168.1.0"),
            Some("192.168.1.1".to_string())
        );
    }

    #[test]
    fn test_next_ip_broadcast() {
        assert_eq!(next_ip("192.168.1.0/24", "192.168.1.255"), None);
    }

    #[test]
    fn test_next_ip_31() {
        assert_eq!(
            next_ip("192.168.1.0/31", "192.168.1.0"),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(next_ip("192.168.1.0/31", "192.168.1.1"), None);
    }

    #[test]
    fn test_host_iter_31_upper_edge() {
        let mut iter = host_iter("255.255.255.254/31").unwrap();
        assert_eq!(iter.next(), Some("255.255.255.254".to_string()));
        assert_eq!(iter.next(), Some("255.255.255.255".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_next_ip_32() {
        assert_eq!(next_ip("192.168.1.5/32", "192.168.1.5"), None);
    }

    #[test]
    fn test_host_iter_32_upper_edge() {
        let mut iter = host_iter("255.255.255.255/32").unwrap();
        assert_eq!(iter.next(), Some("255.255.255.255".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_next_ip_large_prefix() {
        assert_eq!(
            next_ip("10.0.0.0/8", "10.0.0.255"),
            Some("10.0.1.0".to_string())
        );
        assert_eq!(next_ip("10.0.0.0/8", "10.255.255.254"), None);
    }

    #[test]
    fn test_next_ip_out_of_range() {
        assert_eq!(next_ip("192.168.1.0/24", "10.0.0.1"), None);
        assert_eq!(next_ip("192.168.1.0/24", "192.168.2.1"), None);
    }

    #[test]
    fn test_next_ip_invalid_input() {
        assert_eq!(next_ip("invalid", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/33", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/abc", "192.168.1.1"), None);
        assert_eq!(next_ip("192.168.1.0/24", "invalid"), None);
        assert_eq!(next_ip("192.168.1.0/24", "256.1.1.1"), None);
    }

    #[test]
    fn test_cidr_sequence_iteration() {
        let cidr = "192.168.1.0/24";
        let first = first_ip(cidr).unwrap();
        assert_eq!(first, "192.168.1.1");

        let second = next_ip(cidr, &first).unwrap();
        assert_eq!(second, "192.168.1.2");

        let third = next_ip(cidr, &second).unwrap();
        assert_eq!(third, "192.168.1.3");
    }

    #[test]
    fn test_hosts_refuses_large_materialization() {
        assert!(matches!(
            hosts("10.0.0.0/8"),
            Err(HostMaterializeError::TooLarge { .. })
        ));
    }
}
