mod cmd;

use markadd_core::{doctor_stub, version};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("doctor") => {
            cmd::doctor::run(args.collect());
        }
        Some("list-templates") => {
            cmd::list_templates::run(args.collect());
        }
        Some("--version") | Some("-V") => {
            println!("markadd v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd}");
            eprintln!("try: `markadd doctor`, `markadd list-templates`, or `markadd --version`");
            std::process::exit(2);
        }
        None => {
            println!("markadd v{}", env!("CARGO_PKG_VERSION"));
            println!("{}", doctor_stub());
            println!("usage:");
            println!("  markadd doctor [--config <path>] [--profile <name>]");
            println!("  markadd list-templates [--config <path>] [--profile <name>]");
        }
    }
}
