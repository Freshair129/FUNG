//! Headless LM-01 smoke: real capture devices → chunks → whisper → summary.
//! Usage: live_smoke <work-dir> <capture-seconds> [language]

fn main() {
    let mut args = std::env::args().skip(1);
    let work_dir = args
        .next()
        .expect("usage: live_smoke <work-dir> <capture-seconds> [language]");
    let secs: u64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);
    let language = args.next();
    match fung_lib::__debug_live_smoke(&work_dir, secs, language) {
        Ok(report) => println!("=== LIVE SMOKE OK ===\n{report}"),
        Err(error) => {
            println!("=== LIVE SMOKE FAILED ===\n{error}");
            std::process::exit(1);
        }
    }
}
