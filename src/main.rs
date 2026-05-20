#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use arp_scan_rs::network_interface::{
    format_interface_label, is_interface_usable, load_network_interfaces, resolve_selected_interface,
};
use arp_scan_rs::scan_master::{start_scan, ScanEvent, ScanTask};
use arp_scan_rs::scan_target::{
    InputMode, UiInputState, apply_interface_to_input_state, prepare_scan_target_from_ui_fields,
};
use arp_scan_rs::ui::{MainWindow, NetworkInterfaceItem, ResultListData};
use arp_scan_rs::ui_state::{RowMutation, ScanUiState, ViewState};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

const MAX_IN_FLIGHT: usize = 256;

#[derive(Clone)]
struct ActiveScan {
    task_id: u64,
    cancel_flag: Arc<AtomicBool>,
}

fn main() {
    let window = MainWindow::new().unwrap();
    let interfaces = load_network_interfaces().unwrap_or_else(|_| Vec::new());
    let interface_items = std::iter::once(NetworkInterfaceItem {
        label: "Select network interface".into(),
        enabled: true,
    })
    .chain(interfaces.iter().map(|item| NetworkInterfaceItem {
        label: format_interface_label(item).into(),
        enabled: is_interface_usable(item),
    }))
        .collect::<Vec<_>>();

    window.set_input_mode(0);
    window.set_ip_text("192.168.1.23".into());
    window.set_mask_text("255.255.255.0".into());
    window.set_cidr_text("192.168.1.0/24".into());
    window.set_network_interface_model(ModelRc::from(std::rc::Rc::new(VecModel::from(
        interface_items,
    ))));
    window.set_selected_network_interface_index(0);

    let ui_state = Arc::new(Mutex::new(ScanUiState::default()));
    let active_scan = Arc::new(Mutex::new(None::<ActiveScan>));

    window.set_result_list_data_model(ModelRc::from(std::rc::Rc::new(VecModel::default())));
    apply_view_state(&window, ui_state.lock().unwrap().view_state());

    let weak_window = window.as_weak();
    let ui_state_for_start = Arc::clone(&ui_state);
    let active_scan_for_start = Arc::clone(&active_scan);
    window.on_do_scan(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };

        let cidr = match prepare_scan_target_from_ui_fields(
            window.get_input_mode(),
            &window.get_cidr_text().to_string(),
            &window.get_ip_text().to_string(),
            &window.get_mask_text().to_string(),
        ) {
            Ok(cidr) => cidr,
            Err(err) => {
                let mut state = ui_state_for_start.lock().unwrap();
                let row_mutation = state.fail_to_start(err.to_string());
                let view_state = state.view_state();
                drop(state);
                apply_view_state(&window, view_state);
                apply_row_mutation(&window, row_mutation);
                return;
            }
        };
        let task = match start_scan(&cidr, MAX_IN_FLIGHT) {
            Ok(task) => task,
            Err(err) => {
                let mut state = ui_state_for_start.lock().unwrap();
                let row_mutation = state.fail_to_start(err.to_string());
                let view_state = state.view_state();
                drop(state);
                apply_view_state(&window, view_state);
                apply_row_mutation(&window, row_mutation);
                return;
            }
        };

        {
            let mut state = ui_state_for_start.lock().unwrap();
            let row_mutation = state.begin_scan(task.task_id);
            let view_state = state.view_state();
            drop(state);
            apply_view_state(&window, view_state);
            apply_row_mutation(&window, row_mutation);
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

    let interfaces_for_selection = Arc::new(interfaces);
    let weak_window_for_interface = window.as_weak();
    window.on_network_interface_changed(move |index| {
        let Some(interface) = resolve_selected_interface(interfaces_for_selection.as_ref(), index) else {
            return;
        };
        let Some(window) = weak_window_for_interface.upgrade() else {
            return;
        };

        let input_state = current_ui_input_state(&window);
        let next = apply_interface_to_input_state(&input_state, &interface.availability);

        window.set_cidr_text(next.cidr_text.into());
        window.set_ip_text(next.ip_text.into());
        window.set_mask_text(next.mask_text.into());
    });

    window.run().unwrap();
}

fn current_ui_input_state(window: &MainWindow) -> UiInputState {
    UiInputState {
        mode: match window.get_input_mode() {
            1 => InputMode::Cidr,
            _ => InputMode::IpAndMask,
        },
        cidr_text: window.get_cidr_text().to_string(),
        ip_text: window.get_ip_text().to_string(),
        mask_text: window.get_mask_text().to_string(),
    }
}

fn spawn_event_forwarder(
    task: ScanTask,
    weak_window: slint::Weak<MainWindow>,
    ui_state: Arc<Mutex<ScanUiState>>,
    active_scan: Arc<Mutex<Option<ActiveScan>>>,
) {
    let ScanTask {
        task_id,
        cancel_flag,
        events,
        join_handle,
        ..
    } = task;

    thread::spawn(move || {
        let mut ui_dispatch_available = true;

        for event in events {
            if !ui_dispatch_available {
                continue;
            }

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
                    let outcome = state.apply_event(event);
                    if !outcome.applied {
                        return;
                    }
                    let view_state = state.view_state();
                    let row_mutation = outcome.row_mutation;
                    drop(state);
                    apply_view_state(&window, view_state);
                    apply_row_mutation(&window, row_mutation);
                }

                if is_terminal {
                    let mut active = active_scan.lock().unwrap();
                    if active.as_ref().is_some_and(|scan| scan.task_id == task_id) {
                        *active = None;
                    }
                }
            });

            if dispatch_result.is_err() {
                cancel_flag.store(true, Ordering::Relaxed);
                ui_dispatch_available = false;
            }
        }

        if ui_dispatch_available {
            let _ = join_handle.join();
        }
    });
}

fn apply_view_state(window: &MainWindow, view_state: ViewState) {
    window.set_status_text(view_state.status_text.into());
    window.set_progress_text(view_state.progress_text.into());
    window.set_scan_enabled(!view_state.is_scanning);
    window.set_cancel_enabled(view_state.can_cancel);
}

fn apply_row_mutation(window: &MainWindow, row_mutation: RowMutation) {
    if matches!(row_mutation, RowMutation::None) {
        return;
    }

    let model_rc = window.get_result_list_data_model();
    let model = model_rc
        .as_any()
        .downcast_ref::<VecModel<ResultListData>>()
        .expect("result model should always be a VecModel<ResultListData>");

    match row_mutation {
        RowMutation::None => {}
        RowMutation::Clear => model.clear(),
        RowMutation::Insert { index, row } => model.insert(index, map_row(row)),
        RowMutation::Update { index, row } => {
            if index < model.row_count() {
                model.set_row_data(index, map_row(row));
            }
        }
    }
}

fn map_row(row: arp_scan_rs::ui_state::ResultRow) -> ResultListData {
    ResultListData {
        ip: row.ip.into(),
        mac: row.mac.into(),
        hostname: row.hostname.into(),
    }
}
