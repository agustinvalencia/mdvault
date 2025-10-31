#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]
#![allow(
    clippy::print_stdout,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use markadd_core::doctor_stub;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("doctor") => {
            println!("markadd v{} | doctor", env!("CARGO_PKG_VERSION"));
            println!("{}", doctor_stub());
        }
        Some("--version" | "-V") => {
            println!("markadd v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd}");
            eprintln!("try: `markadd doctor` or `markadd --version`");
            std::process::exit(2);
        }
        None => {
            println!("markadd v{}", env!("CARGO_PKG_VERSION"));
            println!("usage:");
            println!("  markadd doctor       # print build & health info");
            println!("  markadd --version    # print version");
        }
    }
}
