use arp_scan_rs::ui::{ResultListData, ResultListPanel};
use i_slint_backend_testing::ElementHandle;
use slint::{ComponentHandle, LogicalSize, Model, ModelRc, VecModel};

#[test]
fn result_list_panel_smoke_test_exposes_generated_bindings() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
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

    panel.set_rows(ModelRc::from(std::rc::Rc::new(rows)));

    let rows_model = panel.get_rows();
    let rows = rows_model
        .as_any()
        .downcast_ref::<VecModel<ResultListData>>()
        .unwrap();

    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().ip, "192.168.1.2");
    assert_eq!(rows.row_data(0).unwrap().mac, "AA:AA:AA:AA:AA:02");
    assert_eq!(rows.row_data(0).unwrap().hostname, "printer.local");
    assert_eq!(rows.row_data(1).unwrap().ip, "192.168.1.20");
    assert_eq!(rows.row_data(1).unwrap().mac, "AA:AA:AA:AA:AA:14");
    assert_eq!(rows.row_data(1).unwrap().hostname, "");
    assert!(rows.row_data(2).is_none());
}

#[test]
fn result_list_panel_renders_hostname_text_in_hostname_column() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
    let rows = VecModel::from(vec![ResultListData {
        ip: "192.168.1.2".into(),
        mac: "AA:AA:AA:AA:AA:02".into(),
        hostname: "printer.local".into(),
    }]);

    panel.set_rows(ModelRc::from(std::rc::Rc::new(rows)));

    let hostname_header = ElementHandle::find_by_element_id(
        &panel,
        "ResultListPanel::hostname-column-header",
    )
    .next()
    .expect("hostname column header should be present");
    assert_eq!(hostname_header.accessible_value().as_deref(), Some("Hostname"));

    let hostname_cell =
        ElementHandle::find_by_element_id(&panel, "ResultListPanel::hostname-cell-text")
            .next()
            .expect("hostname cell text should be present");
    assert_eq!(
        hostname_cell.accessible_value().as_deref(),
        Some("printer.local")
    );
}

#[test]
fn result_list_panel_keeps_hostname_column_reachable_with_empty_and_filled_rows() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
    panel.window().set_size(LogicalSize::new(480.0, 220.0));
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

    panel.set_rows(ModelRc::from(std::rc::Rc::new(rows)));

    let hostname_header = ElementHandle::find_by_element_id(
        &panel,
        "ResultListPanel::hostname-column-header",
    )
    .next()
    .expect("hostname column header should stay present");
    assert_eq!(hostname_header.accessible_value().as_deref(), Some("Hostname"));
    assert!(
        hostname_header.size().width >= 90.0,
        "hostname header should keep enough visible width for a narrow panel, got {}",
        hostname_header.size().width
    );

    let hostname_cell =
        ElementHandle::find_by_element_id(&panel, "ResultListPanel::hostname-cell-text")
            .next()
            .expect("hostname cell text should stay present");
    assert_eq!(
        hostname_cell.accessible_value().as_deref(),
        Some("printer.local")
    );
    assert!(
        hostname_cell.size().width >= 90.0,
        "hostname cell should keep enough visible width for a narrow panel, got {}",
        hostname_cell.size().width
    );
}
