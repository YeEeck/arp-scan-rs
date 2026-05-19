use arp_scan_rs::scan_master::parse_ip;

#[test]
fn parse_ip_matches_windows_ipaddr_representation() {
    let parsed = parse_ip("192.168.80.236").expect("valid IPv4 should parse");

    assert_eq!(parsed, u32::from_ne_bytes([192, 168, 80, 236]));
}
