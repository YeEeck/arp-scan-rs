use arp_scan_rs::scan_target::{
    convert_cidr_to_ip_and_mask, convert_ip_and_mask_to_cidr, normalize_scan_target, InputMode,
    ScanTargetInput, UiInputState,
};

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

#[test]
fn cidr_converts_to_ip_and_mask_for_mode_switch() {
    assert_eq!(
        convert_cidr_to_ip_and_mask("192.168.1.0/24").unwrap(),
        ("192.168.1.0".to_string(), "255.255.255.0".to_string())
    );
}

#[test]
fn ip_and_mask_convert_to_cidr_for_mode_switch() {
    assert_eq!(
        convert_ip_and_mask_to_cidr("192.168.1.23", "255.255.255.128").unwrap(),
        "192.168.1.0/25".to_string()
    );
}

#[test]
fn successful_mode_switch_updates_ip_and_mask_fields() {
    let state = UiInputState {
        mode: InputMode::Cidr,
        cidr_text: "192.168.1.0/24".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.255.0.0".into(),
    };

    let switched = state.switch_mode(InputMode::IpAndMask);

    assert_eq!(switched.mode, InputMode::IpAndMask);
    assert_eq!(switched.cidr_text, "192.168.1.0/24");
    assert_eq!(switched.ip_text, "192.168.1.0");
    assert_eq!(switched.mask_text, "255.255.255.0");
}

#[test]
fn successful_mode_switch_updates_cidr_field() {
    let state = UiInputState {
        mode: InputMode::IpAndMask,
        cidr_text: "10.0.0.0/8".into(),
        ip_text: "192.168.1.23".into(),
        mask_text: "255.255.255.128".into(),
    };

    let switched = state.switch_mode(InputMode::Cidr);

    assert_eq!(switched.mode, InputMode::Cidr);
    assert_eq!(switched.cidr_text, "192.168.1.0/25");
    assert_eq!(switched.ip_text, "192.168.1.23");
    assert_eq!(switched.mask_text, "255.255.255.128");
}

#[test]
fn invalid_mode_switch_keeps_last_known_destination_values() {
    let state = UiInputState {
        mode: InputMode::Cidr,
        cidr_text: "not-a-cidr".into(),
        ip_text: "10.0.0.8".into(),
        mask_text: "255.255.255.0".into(),
    };

    let switched = state.switch_mode(InputMode::IpAndMask);

    assert_eq!(switched.mode, InputMode::IpAndMask);
    assert_eq!(switched.ip_text, "10.0.0.8");
    assert_eq!(switched.mask_text, "255.255.255.0");
}
