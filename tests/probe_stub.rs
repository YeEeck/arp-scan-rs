use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use arp_scan_rs::scan_master::{scan_by_arp, scan_by_arp_with_probe, ArpProbe, SystemArpProbe};

struct FakeProbe;

impl ArpProbe for FakeProbe {
    fn probe(&self, ip: &str) -> std::io::Result<Option<[u8; 6]>> {
        if ip == "192.168.1.10" {
            Ok(Some([1, 2, 3, 4, 5, 6]))
        } else {
            Ok(None)
        }
    }
}

struct FailingProbe {
    calls: Arc<AtomicUsize>,
}

impl FailingProbe {
    fn new(calls: Arc<AtomicUsize>) -> Self {
        Self { calls }
    }
}

impl ArpProbe for FailingProbe {
    fn probe(&self, _ip: &str) -> std::io::Result<Option<[u8; 6]>> {
        let next_call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;

        if next_call == 2 {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "boom"))
        } else {
            Ok(None)
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn system_probe_reports_unsupported_on_non_windows() {
    let err = SystemArpProbe::default()
        .probe("192.168.1.10")
        .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn scan_core_accepts_an_injected_probe() {
    let results = scan_by_arp_with_probe("192.168.1.10/32", FakeProbe).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].ip, "192.168.1.10");
    assert_eq!(results[0].mac, "01:02:03:04:05:06");
    assert!(results[0].exist);
}

#[test]
fn injected_probe_error_is_returned() {
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = FailingProbe::new(Arc::clone(&calls));
    let err = scan_by_arp_with_probe("192.168.1.0/30", &probe).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[cfg(not(target_os = "windows"))]
#[test]
fn public_scan_surfaces_unsupported_on_non_windows() {
    match scan_by_arp("192.168.1.10/32") {
        Ok(results) => panic!("expected unsupported error, got results: {:?}", results.len()),
        Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::Unsupported),
    }
}
