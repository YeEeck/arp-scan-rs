use clap::Parser;

mod scan_master;

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
    MainWindow::new().unwrap().run().unwrap();
}