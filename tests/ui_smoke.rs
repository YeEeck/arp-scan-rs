use arp_scan_rs::ui::{MainWindow, ResultListData};
use slint::{Model, ModelRc, VecModel};

#[test]
fn main_window_smoke_test_exposes_generated_bindings() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let window = MainWindow::new().unwrap();
    let rows = VecModel::from(vec![
        ResultListData {
            ip: "192.168.1.2".into(),
            mac: "AA:AA:AA:AA:AA:02".into(),
            hostname: "printer.local".into(),
        },
        ResultListData {
            ip: "192.168.1.20".into(),
            mac: "AA:AA:AA:AA:AA:14".into(),
            hostname: "".into(),
        },
    ]);

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(rows)));
    window.set_input_mode(0);
    window.set_cidr_text("192.168.1.0/24".into());
    window.set_ip_text("192.168.1.23".into());
    window.set_mask_text("255.255.255.0".into());
    window.set_status_text("Scanning".into());
    window.set_progress_text("2 / 254".into());
    window.set_scan_enabled(false);
    window.set_cancel_enabled(true);

    assert_eq!(window.get_input_mode(), 0);
    assert_eq!(window.get_cidr_text(), "192.168.1.0/24");
    assert_eq!(window.get_ip_text(), "192.168.1.23");
    assert_eq!(window.get_mask_text(), "255.255.255.0");
    assert_eq!(window.get_status_text(), "Scanning");
    assert_eq!(window.get_progress_text(), "2 / 254");
    assert!(!window.get_scan_enabled());
    assert!(window.get_cancel_enabled());
    let rows_model = window.get_result_list_data_model();
    let rows = rows_model
        .as_any()
        .downcast_ref::<VecModel<ResultListData>>()
        .unwrap();

    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().hostname, "printer.local");
    assert_eq!(rows.row_data(1).unwrap().hostname, "");
}
