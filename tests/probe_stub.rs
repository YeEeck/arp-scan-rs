use arp_scan_rs::scan_master::{ArpProbe, SystemArpProbe};

#[cfg(not(target_os = "windows"))]
#[test]
fn system_probe_reports_unsupported_on_non_windows() {
    let err = SystemArpProbe::default()
        .probe("192.168.1.10")
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}
