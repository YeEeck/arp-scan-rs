use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use arp_scan_rs::scan_master::{
    start_scan_with_probe, start_scan_with_probe_and_hostname_resolver, ArpProbe,
    HostnameResolver, ScanEvent, ScanTask,
};

#[derive(Clone)]
struct ControlledProbe {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl ArpProbe for ControlledProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(current, Ordering::SeqCst);
        self.started_tx
            .send(ip.to_string())
            .expect("test should receive probe start notifications");

        let permit_result = self
            .permits
            .lock()
            .expect("probe permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2));

        self.active.fetch_sub(1, Ordering::SeqCst);

        match permit_result {
            Ok(()) => {
                if ip.ends_with(".1") {
                    Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]))
                } else if ip.ends_with(".2") {
                    Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x02]))
                } else {
                    Ok(None)
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "probe release timed out",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "probe release channel disconnected",
            )),
        }
    }
}

fn drain_task(task: ScanTask) -> Vec<ScanEvent> {
    let ScanTask {
        events,
        join_handle,
        ..
    } = task;

    join_handle.join().expect("scan task should join cleanly");
    events.iter().collect()
}

#[test]
fn runtime_rejects_zero_max_in_flight() {
    let (started_tx, _started_rx) = mpsc::channel();
    let (_permit_tx, permit_rx) = mpsc::channel();
    let probe = ControlledProbe {
        active: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let err = start_scan_with_probe("192.168.1.0/30", 0, Arc::new(probe)).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn runtime_respects_max_in_flight_and_streams_terminal_event() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let probe = ControlledProbe {
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe("192.168.1.0/29", 2, Arc::new(probe)).unwrap();

    let mut started = vec![
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first probe should start"),
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second probe should start"),
    ];
    started.sort();

    assert_eq!(
        started,
        vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
    );
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(
        started_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    for _ in 0..6 {
        permit_tx.send(()).unwrap();
    }

    let events = drain_task(task);
    let mut all_started: Vec<_> = started_rx.try_iter().collect();
    all_started.extend(started);
    all_started.sort();

    let progress_updates: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ScanEvent::Progress { scanned_hosts, .. } => Some(*scanned_hosts),
            _ => None,
        })
        .collect();
    let found_hosts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ScanEvent::HostFound { ip, mac, .. } => Some((ip.clone(), mac.clone())),
            _ => None,
        })
        .collect();

    assert_eq!(
        all_started,
        vec![
            "192.168.1.1".to_string(),
            "192.168.1.2".to_string(),
            "192.168.1.3".to_string(),
            "192.168.1.4".to_string(),
            "192.168.1.5".to_string(),
            "192.168.1.6".to_string(),
        ]
    );
    assert!(matches!(
        events.first(),
        Some(ScanEvent::Started {
            total_hosts: 6,
            ..
        })
    ));
    let mut found_hosts = found_hosts;
    found_hosts.sort();
    assert_eq!(
        found_hosts,
        vec![
            ("192.168.1.1".to_string(), "AA:BB:CC:DD:EE:01".to_string()),
            ("192.168.1.2".to_string(), "AA:BB:CC:DD:EE:02".to_string()),
        ]
    );
    assert_eq!(progress_updates, vec![1, 2, 3, 4, 5, 6]);
    assert!(matches!(
        events.last(),
        Some(ScanEvent::Finished {
            scanned_hosts: 6,
            found_hosts: 2,
            ..
        })
    ));
}

#[test]
fn runtime_cancellation_stops_dispatch_and_preserves_completed_results() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let probe = ControlledProbe {
        active,
        peak,
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe("192.168.1.0/29", 1, Arc::new(probe)).unwrap();

    assert_eq!(
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first probe should start"),
        "192.168.1.1".to_string()
    );

    task.cancel_flag.store(true, Ordering::SeqCst);
    permit_tx.send(()).unwrap();

    let events = drain_task(task);
    let remaining_starts: Vec<_> = started_rx.try_iter().collect();

    assert!(remaining_starts.is_empty());
    assert!(events.iter().any(|event| matches!(
        event,
        ScanEvent::HostFound { ip, mac, .. }
            if ip == "192.168.1.1" && mac == "AA:BB:CC:DD:EE:01"
    )));
    assert!(matches!(
        events.last(),
        Some(ScanEvent::Cancelled {
            scanned_hosts: 1,
            found_hosts: 1,
            ..
        })
    ));
}

#[derive(Clone)]
struct PanickingProbe;

impl ArpProbe for PanickingProbe {
    fn probe(&self, _ip: &str) -> io::Result<Option<[u8; 6]>> {
        panic!("probe worker panicked before sending a result");
    }
}

#[test]
fn runtime_emits_failed_when_probe_worker_exits_before_send() {
    let task = start_scan_with_probe("192.168.1.10/32", 1, Arc::new(PanickingProbe)).unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Started {
            total_hosts: 1,
            ..
        })
    ));

    let failure = task
        .events
        .recv_timeout(Duration::from_millis(300))
        .expect("expected a failure event instead of a blocked scan");

    assert!(matches!(
        failure,
        ScanEvent::Failed { ref message, .. }
            if message.contains("probe worker panicked before sending a result")
    ));

    task.join_handle
        .join()
        .expect("scan task should exit cleanly after reporting failure");
}

#[derive(Clone)]
struct FatalThenBlockedProbe {
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl ArpProbe for FatalThenBlockedProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        self.started_tx
            .send(ip.to_string())
            .expect("test should receive probe start notifications");

        if ip.ends_with(".1") {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "fatal unsupported probe failure",
            ))
        } else {
            self.permits
                .lock()
                .expect("probe permits should not be poisoned")
                .recv_timeout(Duration::from_secs(2))
                .expect("blocked worker should be released by the test");
            Ok(None)
        }
    }
}

#[test]
fn runtime_drains_in_flight_workers_before_terminal_failed() {
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let probe = FatalThenBlockedProbe {
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe("192.168.1.0/30", 2, Arc::new(probe)).unwrap();
    let ScanTask {
        events,
        join_handle,
        ..
    } = task;

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Started {
            total_hosts: 2,
            ..
        })
    ));

    let mut started = vec![
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first probe should start"),
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second probe should start"),
    ];
    started.sort();
    assert_eq!(
        started,
        vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
    );

    let (join_done_tx, join_done_rx) = mpsc::channel();
    thread::spawn(move || {
        join_handle
            .join()
            .expect("scan task should join cleanly after draining workers");
        join_done_tx
            .send(())
            .expect("test should observe the join completion");
    });

    assert_eq!(
        join_done_rx.recv_timeout(Duration::from_millis(200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    assert_eq!(
        events.recv_timeout(Duration::from_millis(200)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    permit_tx.send(()).unwrap();

    let failure = events
        .recv_timeout(Duration::from_secs(1))
        .expect("expected a terminal failure after in-flight drain");
    assert!(matches!(
        failure,
        ScanEvent::Failed { ref message, .. } if message == "fatal unsupported probe failure"
    ));
    assert_eq!(
        join_done_rx.recv_timeout(Duration::from_secs(1)),
        Ok(())
    );
}

#[derive(Clone)]
struct QuickProbe;

impl ArpProbe for QuickProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        if ip.ends_with(".1") {
            Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone)]
struct BlockingHostnameResolver {
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl HostnameResolver for BlockingHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        self.started_tx
            .send(ip.to_string())
            .expect("test should observe hostname resolution start");

        self.permits
            .lock()
            .expect("hostname permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2))
            .ok()?;

        Some("printer.lan".to_string())
    }
}

#[test]
fn runtime_streams_hostname_updates_without_waiting_for_lookup() {
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let resolver = BlockingHostnameResolver {
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.1/32",
        1,
        Arc::new(QuickProbe),
        Arc::new(resolver),
    )
    .unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Started {
            total_hosts: 1,
            ..
        })
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::HostFound {
            ip,
            mac,
            ..
        }) if ip == "192.168.1.1" && mac == "AA:BB:CC:DD:EE:01"
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Progress {
            scanned_hosts: 1,
            total_hosts: 1,
            ..
        })
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Finished {
            scanned_hosts: 1,
            found_hosts: 1,
            ..
        })
    ));

    assert_eq!(
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("hostname lookup should have started"),
        "192.168.1.1"
    );
    assert_eq!(
        task.events.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    permit_tx.send(()).unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::HostnameResolved {
            ip,
            hostname,
            ..
        }) if ip == "192.168.1.1" && hostname == "printer.lan"
    ));

    task.join_handle
        .join()
        .expect("scan task should exit cleanly");
}
