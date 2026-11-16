//! `hint`: the single hint-publisher CLI (port of `scripts/hint.py`). Launcher: `scripts/hint.sh`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = hintlib::cli::main(&args);
    hintlib::io::flush();
    std::process::exit(code);
}
