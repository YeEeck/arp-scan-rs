use arp_scan_rs::ui::{ResultListData, ResultListPanel};
use slint::{Model, ModelRc, VecModel};

#[test]
fn result_list_panel_smoke_test_exposes_generated_bindings() {
    let _backend = i_slint_backend_testing::init_no_event_loop();

    let panel = ResultListPanel::new().unwrap();
    let rows = VecModel::from(vec![
        ResultListData {
            ip: "192.168.1.2".into(),
            mac: "AA:AA:AA:AA:AA:02".into(),
        },
        ResultListData {
            ip: "192.168.1.20".into(),
            mac: "AA:AA:AA:AA:AA:14".into(),
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
    assert_eq!(rows.row_data(1).unwrap().ip, "192.168.1.20");
    assert_eq!(rows.row_data(1).unwrap().mac, "AA:AA:AA:AA:AA:14");
    assert!(rows.row_data(2).is_none());
}
