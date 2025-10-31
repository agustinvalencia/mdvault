#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub rustc_version() -> String {
    std::env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".into())
}

pub fn doctor_stub() -> String {
    format!(
        "markadd-core v{} | rustc {} on {} ",
        version(),
        rustc_version(),
        std::env::consts::OS
    )
}

