use arp_scan_rs::network_interface::{
    format_interface_label, load_network_interfaces, resolve_selected_interface,
    is_interface_usable, NetworkInterface, NetworkInterfaceAvailability, NetworkInterfaceIpv4,
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
fn usability_reflects_availability_state() {
    let available = NetworkInterface {
        id: "ethernet-1".into(),
        name: "Ethernet".into(),
        availability: NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
            ip: "192.168.1.23".into(),
            mask: "255.255.255.0".into(),
            cidr: "192.168.1.0/24".into(),
        }),
    };
    let unavailable = NetworkInterface {
        id: "loopback".into(),
        name: "Loopback Pseudo-Interface".into(),
        availability: NetworkInterfaceAvailability::Unavailable {
            reason: "Loopback".into(),
        },
    };

    assert!(is_interface_usable(&available));
    assert!(!is_interface_usable(&unavailable));
}

#[test]
fn unavailable_interface_cannot_be_resolved_from_selection_index() {
    let interfaces = vec![
        NetworkInterface {
            id: "ethernet-1".into(),
            name: "Ethernet".into(),
            availability: NetworkInterfaceAvailability::Available(NetworkInterfaceIpv4 {
                ip: "192.168.1.23".into(),
                mask: "255.255.255.0".into(),
                cidr: "192.168.1.0/24".into(),
            }),
        },
        NetworkInterface {
            id: "loopback".into(),
            name: "Loopback".into(),
            availability: NetworkInterfaceAvailability::Unavailable {
                reason: "Loopback".into(),
            },
        },
    ];

    assert!(resolve_selected_interface(&interfaces, 1).is_some());
    assert!(resolve_selected_interface(&interfaces, 2).is_none());
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
