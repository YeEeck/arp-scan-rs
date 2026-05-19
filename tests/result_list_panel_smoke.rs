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

    assert_eq!(
        panel
            .get_rows()
            .as_any()
            .downcast_ref::<VecModel<ResultListData>>()
            .unwrap()
            .row_count(),
        2
    );
}
