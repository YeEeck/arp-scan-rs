#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use arp_scan_rs::scan_master::{start_scan, ScanEvent, ScanTask};
use arp_scan_rs::ui_state::{ScanUiState, ViewSnapshot};
use slint::{ModelRc, VecModel};

slint::include_modules!();

const MAX_IN_FLIGHT: usize = 256;

#[derive(Clone)]
struct ActiveScan {
    task_id: u64,
    cancel_flag: Arc<AtomicBool>,
}

fn main() {
    let window = MainWindow::new().unwrap();
    let ui_state = Arc::new(Mutex::new(ScanUiState::default()));
    let active_scan = Arc::new(Mutex::new(None::<ActiveScan>));

    window.set_result_list_data_model(ModelRc::from(Rc::new(VecModel::default())));
    apply_snapshot(&window, ui_state.lock().unwrap().snapshot());

    let weak_window = window.as_weak();
    let ui_state_for_start = Arc::clone(&ui_state);
    let active_scan_for_start = Arc::clone(&active_scan);
    window.on_do_scan(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };

        let cidr = window.get_cidr().to_string();
        let task = match start_scan(&cidr, MAX_IN_FLIGHT) {
            Ok(task) => task,
            Err(err) => {
                let mut state = ui_state_for_start.lock().unwrap();
                state.fail_to_start(err.to_string());
                apply_snapshot(&window, state.snapshot());
                return;
            }
        };

        {
            let mut state = ui_state_for_start.lock().unwrap();
            state.begin_scan(task.task_id);
            apply_snapshot(&window, state.snapshot());
        }

        {
            let mut active = active_scan_for_start.lock().unwrap();
            *active = Some(ActiveScan {
                task_id: task.task_id,
                cancel_flag: Arc::clone(&task.cancel_flag),
            });
        }

        spawn_event_forwarder(
            task,
            window.as_weak(),
            Arc::clone(&ui_state_for_start),
            Arc::clone(&active_scan_for_start),
        );
    });

    let active_scan_for_cancel = Arc::clone(&active_scan);
    window.on_do_cancel(move || {
        if let Some(active) = active_scan_for_cancel.lock().unwrap().as_ref() {
            active.cancel_flag.store(true, Ordering::Relaxed);
        }
    });

    window.run().unwrap();
}

fn spawn_event_forwarder(
    task: ScanTask,
    weak_window: slint::Weak<MainWindow>,
    ui_state: Arc<Mutex<ScanUiState>>,
    active_scan: Arc<Mutex<Option<ActiveScan>>>,
) {
    let ScanTask {
        task_id,
        events,
        join_handle,
        ..
    } = task;

    thread::spawn(move || {
        for event in events {
            let is_terminal = matches!(
                &event,
                ScanEvent::Finished { .. } | ScanEvent::Cancelled { .. } | ScanEvent::Failed { .. }
            );
            let weak_window = weak_window.clone();
            let ui_state = Arc::clone(&ui_state);
            let active_scan = Arc::clone(&active_scan);

            let dispatch_result = slint::invoke_from_event_loop(move || {
                let Some(window) = weak_window.upgrade() else {
                    return;
                };

                {
                    let mut state = ui_state.lock().unwrap();
                    state.apply_event(event);
                    apply_snapshot(&window, state.snapshot());
                }

                if is_terminal {
                    let mut active = active_scan.lock().unwrap();
                    if active.as_ref().is_some_and(|scan| scan.task_id == task_id) {
                        *active = None;
                    }
                }
            });

            if dispatch_result.is_err() {
                break;
            }
        }

        let _ = join_handle.join();
    });
}

fn apply_snapshot(
    window: &MainWindow,
    snapshot: ViewSnapshot,
) {
    window.set_status_text(snapshot.status_text.into());
    window.set_progress_text(snapshot.progress_text.into());
    window.set_scan_enabled(!snapshot.is_scanning);
    window.set_cancel_enabled(snapshot.can_cancel);

    let rows = snapshot
        .rows
        .into_iter()
        .map(|row| ResultListData {
            ip: row.ip.into(),
            mac: row.mac.into(),
        })
        .collect::<Vec<_>>();
    window.set_result_list_data_model(ModelRc::from(Rc::new(VecModel::from(rows))));
}
