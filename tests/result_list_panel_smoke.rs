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

    let hostname_cells =
        ElementHandle::find_by_element_id(&panel, "ResultListPanel::hostname-cell-text")
            .collect::<Vec<_>>();
    assert_eq!(
        hostname_cells.len(),
        2,
        "two rows should render two hostname cells"
    );

    let hostname_values = hostname_cells
        .iter()
        .map(|cell| cell.accessible_value().map(|value| value.to_string()))
        .collect::<Vec<_>>();
    assert_eq!(
        hostname_values,
        vec![Some("printer.local".to_string()), Some(String::new())]
    );

    let header_position = hostname_header.absolute_position();
    let header_size = hostname_header.size();
    let first_cell_position = hostname_cells[0].absolute_position();
    let first_cell_size = hostname_cells[0].size();
    let second_cell_position = hostname_cells[1].absolute_position();
    let second_cell_size = hostname_cells[1].size();
    let window_width = 480.0;

    assert!(
        (header_position.x - first_cell_position.x).abs() < f32::EPSILON,
        "hostname header and first cell should stay column-aligned: header x={}, cell x={}",
        header_position.x,
        first_cell_position.x
    );
    assert!(
        (first_cell_position.x - second_cell_position.x).abs() < f32::EPSILON,
        "hostname cells should stay aligned across rows: first x={}, second x={}",
        first_cell_position.x,
        second_cell_position.x
    );
    assert!(
        (header_size.width - first_cell_size.width).abs() < f32::EPSILON,
        "hostname header and first cell should keep the same visible column width: header={}, cell={}",
        header_size.width,
        first_cell_size.width
    );
    assert!(
        first_cell_position.x >= 0.0 && first_cell_position.x + first_cell_size.width <= window_width,
        "first hostname cell should stay inside the visible window region: left={}, right={}, window={}",
        first_cell_position.x,
        first_cell_position.x + first_cell_size.width,
        window_width
    );
    assert!(
        header_position.x >= 0.0 && header_position.x + header_size.width <= window_width,
        "hostname header should stay inside the visible window region: left={}, right={}, window={}",
        header_position.x,
        header_position.x + header_size.width,
        window_width
    );
    assert!(
        second_cell_position.x >= 0.0
            && second_cell_position.x + second_cell_size.width <= window_width,
        "second hostname cell should stay inside the visible window region: left={}, right={}, window={}",
        second_cell_position.x,
        second_cell_position.x + second_cell_size.width,
        window_width
    );
}
