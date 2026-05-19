use arp_scan_rs::scan_master::ScanEvent;
use arp_scan_rs::ui_state::ScanUiState;

#[test]
fn ui_state_sorts_deduplicates_and_ignores_stale_events() {
    let mut state = ScanUiState::default();

    state.begin_scan(7);
    state.apply_event(ScanEvent::Started {
        task_id: 7,
        total_hosts: 4,
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.20".into(),
        mac: "AA:AA:AA:AA:AA:20".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.2".into(),
        mac: "AA:AA:AA:AA:AA:02".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 7,
        ip: "192.168.1.20".into(),
        mac: "AA:AA:AA:AA:AA:20".into(),
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 6,
        ip: "192.168.1.1".into(),
        mac: "STALE".into(),
    });
    state.apply_event(ScanEvent::Progress {
        task_id: 7,
        scanned_hosts: 2,
        total_hosts: 4,
    });

    let snapshot = state.snapshot();

    assert!(snapshot.is_scanning);
    assert!(snapshot.can_cancel);
    assert_eq!(snapshot.status_text, "Scanning");
    assert_eq!(snapshot.progress_text, "2 / 4");
    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(snapshot.rows[0].ip, "192.168.1.2");
    assert_eq!(snapshot.rows[0].mac, "AA:AA:AA:AA:AA:02");
    assert_eq!(snapshot.rows[1].ip, "192.168.1.20");
    assert_eq!(snapshot.rows[1].mac, "AA:AA:AA:AA:AA:20");
}

#[test]
fn ui_state_cancel_preserves_results_and_marks_status_cancelled() {
    let mut state = ScanUiState::default();

    state.begin_scan(9);
    state.apply_event(ScanEvent::Started {
        task_id: 9,
        total_hosts: 3,
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 9,
        ip: "192.168.1.8".into(),
        mac: "AA:AA:AA:AA:AA:08".into(),
    });
    state.apply_event(ScanEvent::Progress {
        task_id: 9,
        scanned_hosts: 1,
        total_hosts: 3,
    });
    state.apply_event(ScanEvent::Cancelled {
        task_id: 9,
        scanned_hosts: 1,
        found_hosts: 1,
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 8,
        ip: "192.168.1.2".into(),
        mac: "STALE".into(),
    });

    let snapshot = state.snapshot();

    assert!(!snapshot.is_scanning);
    assert!(!snapshot.can_cancel);
    assert_eq!(snapshot.status_text, "Cancelled (1 hosts)");
    assert_eq!(snapshot.progress_text, "1 / 3");
    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.rows[0].ip, "192.168.1.8");
    assert_eq!(snapshot.rows[0].mac, "AA:AA:AA:AA:AA:08");
}

#[test]
fn begin_scan_clears_previous_results_for_a_new_task() {
    let mut state = ScanUiState::default();

    state.begin_scan(10);
    state.apply_event(ScanEvent::Started {
        task_id: 10,
        total_hosts: 2,
    });
    state.apply_event(ScanEvent::HostFound {
        task_id: 10,
        ip: "192.168.1.3".into(),
        mac: "AA:AA:AA:AA:AA:03".into(),
    });
    state.apply_event(ScanEvent::Finished {
        task_id: 10,
        scanned_hosts: 2,
        found_hosts: 1,
    });

    state.begin_scan(11);

    let snapshot = state.snapshot();

    assert!(snapshot.is_scanning);
    assert!(snapshot.can_cancel);
    assert_eq!(snapshot.status_text, "Scanning");
    assert_eq!(snapshot.progress_text, "0 / 0");
    assert!(snapshot.rows.is_empty());

    state.apply_event(ScanEvent::Started {
        task_id: 10,
        total_hosts: 99,
    });
    state.apply_event(ScanEvent::Progress {
        task_id: 10,
        scanned_hosts: 99,
        total_hosts: 99,
    });

    let snapshot = state.snapshot();
    assert_eq!(snapshot.progress_text, "0 / 0");
    assert!(snapshot.rows.is_empty());
}
