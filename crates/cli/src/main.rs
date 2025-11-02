mod cmd;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("doctor") => {
            cmd::doctor::run(args.collect());
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
