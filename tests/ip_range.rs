use arp_scan_rs::scan_master::{host_count, host_iter, hosts, HostMaterializationError};

#[test]
fn hosts_and_counts_follow_cidr_rules() {
    assert_eq!(host_count("192.168.1.0/24"), Some(254));
    assert_eq!(
        hosts("192.168.1.0/30").unwrap(),
        vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
    );
    assert_eq!(
        hosts("192.168.1.0/31").unwrap(),
        vec!["192.168.1.0".to_string(), "192.168.1.1".to_string()]
    );
    assert_eq!(hosts("192.168.1.5/32").unwrap(), vec!["192.168.1.5".to_string()]);
}

#[test]
fn invalid_cidr_returns_invalid_cidr_error() {
    assert_eq!(host_count("bad"), None);
    assert!(matches!(
        hosts("192.168.1.0/33"),
        Err(HostMaterializationError::InvalidCidr)
    ));
}

#[test]
fn large_cidr_returns_too_large_error_but_still_supports_lazy_iteration() {
    assert_eq!(host_count("10.0.0.0/8"), Some(16_777_214));
    assert_eq!(
        hosts("10.0.0.0/8"),
        Err(HostMaterializationError::TooLarge {
            total_hosts: 16_777_214,
            max_hosts: 4_096,
        })
    );

    let mut iter = host_iter("10.0.0.0/8").expect("expected lazy iterator for valid cidr");
    assert_eq!(iter.next().as_deref(), Some("10.0.0.1"));
    assert_eq!(iter.next().as_deref(), Some("10.0.0.2"));
    assert_eq!(iter.next().as_deref(), Some("10.0.0.3"));
}
