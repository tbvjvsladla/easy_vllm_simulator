//! `render_bench_section`: bench_report reader/renderer (port of `scripts/render_bench_section.py`).
//! Launcher: `scripts/render_bench_section.sh`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = hintlib::render_bench_section::main(&args);
    hintlib::io::flush();
    std::process::exit(code);
}
