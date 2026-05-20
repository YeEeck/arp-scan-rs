use arp_scan_rs::network_interface::{
    NetworkInterface, NetworkInterfaceAvailability, NetworkInterfaceIpv4, format_interface_label,
    load_network_interfaces,
};

#[test]
fn available_interface_keeps_plain_display_label() {
    let item = NetworkInterface {
        id: "ethernet-1".into(),
        name: "Ethernet".into(),
        availability: NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
            ip: "192.168.1.23".into(),
            mask: "255.255.255.0".into(),
            cidr: "192.168.1.0/24".into(),
        }),
    };

    assert_eq!(format_interface_label(&item), "Ethernet");
}

#[test]
fn unavailable_interface_appends_reason_to_display_label() {
    let item = NetworkInterface {
        id: "loopback".into(),
        name: "Loopback Pseudo-Interface".into(),
        availability: NetworkInterfaceAvailability::Unavailable {
            reason: "Loopback".into(),
        },
    };

    assert_eq!(
        format_interface_label(&item),
        "Loopback Pseudo-Interface (Loopback)"
    );
}

#[test]
fn load_network_interfaces_returns_at_least_placeholder_entries_that_have_labels() {
    let interfaces = load_network_interfaces().expect("interface load should not fail hard");

    assert!(
        !interfaces.is_empty(),
        "the UI needs either real interfaces or a placeholder entry"
    );
    assert!(
        interfaces
            .iter()
            .all(|item| !format_interface_label(item).trim().is_empty()),
        "every interface item should have a visible label"
    );
}
