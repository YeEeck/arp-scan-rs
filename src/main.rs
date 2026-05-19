#![windows_subsystem = "windows"]

use clap::Parser;
use slint::{Model, ModelRc, VecModel};
use std::rc::Rc;
use arp_scan_rs::scan_master;

#[derive(Parser)]
#[command(version, author, about, long_about = None)]
struct Cli {
    #[arg(long)]
    cidr: String,
}

slint::include_modules!();
fn main() {
    /*
    let cli = Cli::parse();
    let cidr = &cli.cidr;
    println!("Use CIDR: {cidr}");

    let res_vec = scan_master::scan_by_arp(cidr).unwrap();
    for res_elem in res_vec {
        if res_elem.exist {
            println!("{} --- {}", res_elem.ip, res_elem.mac);
        }
    }
    */
    //MainWindow::new().unwrap().run().unwrap();
    let main_window = MainWindow::new().unwrap();

    let init_result_data: Vec<ResultListData> = vec![
        ResultListData { data: "Waiting for scan.".into() },
        ResultListData { data: "...".into() },
    ];

    let result_list_data_model = Rc::new(VecModel::from(init_result_data));
    main_window.set_result_list_data_model(ModelRc::from(result_list_data_model.clone()));

    let weak_window = main_window.as_weak();
    main_window.on_do_scan(move || {
        let window = weak_window.upgrade().unwrap();
        let current_cidr = window.get_cidr();
        let result_model_rc = window.get_result_list_data_model();
        let model = result_model_rc
            .as_any()
            .downcast_ref::<VecModel<ResultListData>>()
            .unwrap();
        match scan_master::scan_by_arp(&current_cidr) {
            Ok(scan_res) => {
                model.clear();
                for scan_res_elem in scan_res {
                    let new_data = ResultListData {
                        data: format!("{} -- {}", scan_res_elem.ip, scan_res_elem.mac).into(),
                    };
                    model.push(new_data);
                }
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
    });

    main_window.run().unwrap();
}
