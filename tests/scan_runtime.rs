use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use arp_scan_rs::scan_master::{
    start_scan_with_probe, start_scan_with_probe_and_hostname_resolver, ArpProbe,
    HostnameResolver, ScanEvent, ScanTask,
};

const HOSTNAME_WORKER_COUNT: usize = 2;

fn hostname_runtime_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    let _lock = hostname_runtime_test_lock();
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

#[derive(Clone)]
struct AllHostsFoundProbe;

impl ArpProbe for AllHostsFoundProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        let suffix = ip
            .rsplit('.')
            .next()
            .expect("test IPs should contain an IPv4 suffix")
            .parse::<u8>()
            .expect("IPv4 suffix should parse");
        Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, suffix]))
    }
}

#[derive(Clone)]
struct PeakTrackingHostnameResolver {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl HostnameResolver for PeakTrackingHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(current, Ordering::SeqCst);
        self.started_tx
            .send(ip.to_string())
            .expect("test should observe hostname resolution start");

        let permit = self
            .permits
            .lock()
            .expect("hostname permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2));

        self.active.fetch_sub(1, Ordering::SeqCst);

        permit.ok()?;
        Some(format!("host-{ip}"))
    }
}

#[test]
fn runtime_bounds_hostname_resolution_concurrency() {
    let _lock = hostname_runtime_test_lock();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let resolver = PeakTrackingHostnameResolver {
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.0/29",
        2,
        Arc::new(AllHostsFoundProbe),
        Arc::new(resolver),
    )
    .unwrap();

    let first_two = vec![
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first hostname lookup should start"),
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second hostname lookup should start"),
    ];

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
    all_started.extend(first_two);
    all_started.sort();

    let hostname_updates: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ScanEvent::HostnameResolved { ip, hostname, .. } => Some((ip.clone(), hostname.clone())),
            _ => None,
        })
        .collect();

    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(peak.load(Ordering::SeqCst), 2);
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
    assert_eq!(hostname_updates.len(), 6);
}

#[derive(Clone)]
struct OneHostFoundThenBlockedProbe {
    started_tx: mpsc::Sender<String>,
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl ArpProbe for OneHostFoundThenBlockedProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        self.started_tx
            .send(ip.to_string())
            .expect("test should observe probe start");

        self.permits
            .lock()
            .expect("probe permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2))
            .expect("test should release blocked probes");

        if ip.ends_with(".1") {
            Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn runtime_suppresses_hostname_updates_after_cancellation() {
    let _lock = hostname_runtime_test_lock();
    let (probe_started_tx, probe_started_rx) = mpsc::channel();
    let (probe_permit_tx, probe_permit_rx) = mpsc::channel();
    let (hostname_started_tx, hostname_started_rx) = mpsc::channel();
    let (hostname_permit_tx, hostname_permit_rx) = mpsc::channel();
    let probe = OneHostFoundThenBlockedProbe {
        started_tx: probe_started_tx,
        permits: Arc::new(Mutex::new(probe_permit_rx)),
    };
    let resolver = BlockingHostnameResolver {
        started_tx: hostname_started_tx,
        permits: Arc::new(Mutex::new(hostname_permit_rx)),
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.0/29",
        1,
        Arc::new(probe),
        Arc::new(resolver),
    )
    .unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Started { .. })
    ));
    assert_eq!(
        probe_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first probe should start"),
        "192.168.1.1".to_string()
    );

    probe_permit_tx.send(()).unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::HostFound { ip, .. }) if ip == "192.168.1.1"
    ));
    assert!(matches!(
        task.events.recv_timeout(Duration::from_secs(1)),
        Ok(ScanEvent::Progress {
            scanned_hosts: 1,
            ..
        })
    ));
    assert_eq!(
        hostname_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("hostname lookup should have started"),
        "192.168.1.1".to_string()
    );

    task.cancel_flag.store(true, Ordering::SeqCst);

    assert_eq!(
        probe_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second probe should start before cancellation is observed"),
        "192.168.1.2".to_string()
    );
    probe_permit_tx.send(()).unwrap();

    let mut saw_cancelled = false;
    while !saw_cancelled {
        let event = task
            .events
            .recv_timeout(Duration::from_secs(1))
            .expect("scan should emit a terminal cancellation event");
        saw_cancelled = matches!(event, ScanEvent::Cancelled { .. });
    }

    hostname_permit_tx.send(()).unwrap();

    assert!(matches!(
        task.events.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected)
    ));

    task.join_handle
        .join()
        .expect("scan task should exit cleanly");
}

#[derive(Clone)]
struct HostFoundThenBlockedProbe {
    permits: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl ArpProbe for HostFoundThenBlockedProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        if ip.ends_with(".1") {
            return Ok(Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]));
        }

        self.permits
            .lock()
            .expect("probe permits should not be poisoned")
            .recv_timeout(Duration::from_secs(2))
            .expect("test should release blocked probes");

        Ok(None)
    }
}

#[test]
fn runtime_caps_hostname_workers_across_cancelled_scan_cycles() {
    let _lock = hostname_runtime_test_lock();
    let scan_count = 6;
    let (hostname_started_tx, hostname_started_rx) = mpsc::channel();
    let (hostname_permit_tx, hostname_permit_rx) = mpsc::channel();
    let resolver = BlockingHostnameResolver {
        started_tx: hostname_started_tx,
        permits: Arc::new(Mutex::new(hostname_permit_rx)),
    };
    let resolver = Arc::new(resolver);

    let (wave1_probe_permit_tx, wave1_probe_permit_rx) = mpsc::channel();
    let wave1_probe = Arc::new(HostFoundThenBlockedProbe {
        permits: Arc::new(Mutex::new(wave1_probe_permit_rx)),
    });

    let wave1_tasks: Vec<_> = (0..scan_count)
        .map(|_| {
            start_scan_with_probe_and_hostname_resolver(
                "192.168.1.0/30",
                1,
                Arc::clone(&wave1_probe),
                Arc::clone(&resolver),
            )
            .unwrap()
        })
        .collect();

    let mut wave1_hostname_starts = Vec::new();
    while let Ok(ip) = hostname_started_rx.recv_timeout(Duration::from_millis(150)) {
        wave1_hostname_starts.push(ip);
    }

    assert!(
        !wave1_hostname_starts.is_empty(),
        "first wave should start at least one hostname lookup"
    );
    assert!(
        wave1_hostname_starts.len() < scan_count,
        "hostname concurrency should not scale to one worker per scan"
    );

    for task in &wave1_tasks {
        task.cancel_flag.store(true, Ordering::SeqCst);
    }
    for _ in 0..scan_count {
        wave1_probe_permit_tx.send(()).unwrap();
    }
    for task in wave1_tasks {
        task.join_handle
            .join()
            .expect("first-wave scan task should exit cleanly");
    }

    let (wave2_probe_permit_tx, wave2_probe_permit_rx) = mpsc::channel();
    let wave2_probe = Arc::new(HostFoundThenBlockedProbe {
        permits: Arc::new(Mutex::new(wave2_probe_permit_rx)),
    });

    let wave2_tasks: Vec<_> = (0..scan_count)
        .map(|_| {
            start_scan_with_probe_and_hostname_resolver(
                "192.168.1.0/30",
                1,
                Arc::clone(&wave2_probe),
                Arc::clone(&resolver),
            )
            .unwrap()
        })
        .collect();

    assert_eq!(
        hostname_started_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "second wave should stay queued while first-wave hostname lookups occupy the shared pool"
    );

    for task in &wave2_tasks {
        task.cancel_flag.store(true, Ordering::SeqCst);
    }
    for _ in 0..scan_count {
        wave2_probe_permit_tx.send(()).unwrap();
    }
    for _ in 0..wave1_hostname_starts.len() {
        hostname_permit_tx.send(()).unwrap();
    }
    for task in wave2_tasks {
        task.join_handle
            .join()
            .expect("second-wave scan task should exit cleanly");
    }
}

#[test]
fn runtime_eventually_processes_hostname_jobs_when_executor_queue_is_full() {
    let _lock = hostname_runtime_test_lock();
    let host_count = 6;
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = mpsc::channel();
    let (permit_tx, permit_rx) = mpsc::channel();
    let resolver = PeakTrackingHostnameResolver {
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        started_tx,
        permits: Arc::new(Mutex::new(permit_rx)),
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.0/29",
        host_count,
        Arc::new(AllHostsFoundProbe),
        Arc::new(resolver),
    )
    .unwrap();

    let ScanTask {
        events,
        join_handle,
        ..
    } = task;

    let mut runtime_events = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(1))
            .expect("scan should emit events until it finishes");
        let finished = matches!(
            event,
            ScanEvent::Finished {
                scanned_hosts: 6,
                found_hosts: 6,
                ..
            }
        );
        runtime_events.push(event);
        if finished {
            break;
        }
    }

    let started_before_release: Vec<_> = started_rx.try_iter().collect();

    for _ in 0..host_count {
        permit_tx.send(()).unwrap();
    }

    let mut hostname_updates = Vec::new();
    while hostname_updates.len() < host_count {
        match events
            .recv_timeout(Duration::from_secs(1))
            .expect("hostname jobs should continue after terminal scan event")
        {
            ScanEvent::HostnameResolved { ip, hostname, .. } => {
                hostname_updates.push((ip, hostname));
            }
            other => panic!("unexpected trailing event after scan finish: {other:?}"),
        }
    }

    join_handle
        .join()
        .expect("scan task should join cleanly");

    let mut all_started: Vec<_> = started_rx.try_iter().collect();
    all_started.extend(started_before_release.iter().cloned());
    all_started.sort();
    hostname_updates.sort();

    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert!(runtime_events.iter().any(|event| matches!(
        event,
        ScanEvent::Finished {
            scanned_hosts: 6,
            found_hosts: 6,
            ..
        }
    )));
    assert!(!started_before_release.is_empty());
    assert!(started_before_release.len() <= HOSTNAME_WORKER_COUNT);
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
    assert_eq!(
        hostname_updates,
        vec![
            ("192.168.1.1".to_string(), "host-192.168.1.1".to_string()),
            ("192.168.1.2".to_string(), "host-192.168.1.2".to_string()),
            ("192.168.1.3".to_string(), "host-192.168.1.3".to_string()),
            ("192.168.1.4".to_string(), "host-192.168.1.4".to_string()),
            ("192.168.1.5".to_string(), "host-192.168.1.5".to_string()),
            ("192.168.1.6".to_string(), "host-192.168.1.6".to_string()),
        ]
    );
}

#[derive(Clone)]
struct PanickingThenWorkingHostnameResolver {
    panics_remaining: Arc<AtomicUsize>,
    started_tx: mpsc::Sender<String>,
}

impl HostnameResolver for PanickingThenWorkingHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        self.started_tx
            .send(ip.to_string())
            .expect("test should observe hostname resolution start");

        if self.panics_remaining.fetch_update(
            Ordering::SeqCst,
            Ordering::SeqCst,
            |remaining| remaining.checked_sub(1),
        )
        .is_ok()
        {
            panic!("resolver panic should not kill shared worker");
        }

        Some(format!("host-{ip}"))
    }
}

#[test]
fn runtime_recovers_from_hostname_resolver_panics() {
    let _lock = hostname_runtime_test_lock();
    let (started_tx, started_rx) = mpsc::channel();
    let resolver = PanickingThenWorkingHostnameResolver {
        panics_remaining: Arc::new(AtomicUsize::new(HOSTNAME_WORKER_COUNT)),
        started_tx,
    };

    let task = start_scan_with_probe_and_hostname_resolver(
        "192.168.1.0/29",
        1,
        Arc::new(AllHostsFoundProbe),
        Arc::new(resolver),
    )
    .unwrap();

    let events = drain_task(task);
    thread::sleep(Duration::from_millis(150));

    let started: Vec<_> = started_rx.try_iter().collect();
    let hostname_updates: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ScanEvent::HostnameResolved { ip, hostname, .. } => Some((ip.clone(), hostname.clone())),
            _ => None,
        })
        .collect();

    assert!(events.iter().any(|event| matches!(
        event,
        ScanEvent::Finished {
            scanned_hosts: 6,
            found_hosts: 6,
            ..
        }
    )));
    assert!(started.len() >= HOSTNAME_WORKER_COUNT);
    assert!(
        !hostname_updates.is_empty(),
        "workers should continue processing jobs after resolver panics"
    );
}
