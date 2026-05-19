#[test]
fn windows_ip_helper_link_name_uses_mingw_compatible_casing() {
    let source = include_str!("../src/scan_master/arp_core.rs");

    assert!(
        source.contains(r#"#[link(name = "iphlpapi")]"#),
        "expected arp_core.rs to link against lowercase iphlpapi for GNU toolchains"
    );
}
