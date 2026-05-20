use arp_scan_rs::scan_target::{normalize_scan_target, InputMode, ScanTargetInput};

#[test]
fn cidr_mode_preserves_valid_cidr() {
    let input = ScanTargetInput {
        mode: InputMode::Cidr,
        cidr_text: "192.168.1.0/24".into(),
        ip_text: String::new(),
        mask_text: String::new(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap(),
        "192.168.1.0/24".to_string()
    );
}

#[test]
fn ip_and_mask_mode_normalizes_host_ip_to_network_cidr() {
    let input = ScanTargetInput {
        mode: InputMode::IpAndMask,
        cidr_text: String::new(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.255.255.0".into(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap(),
        "192.168.1.0/24".to_string()
    );
}

#[test]
fn ip_and_mask_mode_rejects_non_contiguous_masks() {
    let input = ScanTargetInput {
        mode: InputMode::IpAndMask,
        cidr_text: String::new(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.0.255.0".into(),
    };

    assert_eq!(
        normalize_scan_target(&input).unwrap_err().to_string(),
        "Invalid subnet mask"
    );
}
