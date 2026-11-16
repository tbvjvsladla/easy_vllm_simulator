use std::process::ExitCode;

fn main() -> ExitCode {
    let mut it = std::env::args();
    let argv0 = it.next();
    let args: Vec<String> = it.collect();
    ExitCode::from(u8::try_from(upstream_runtime::branch_sync_selftest::run(argv0, &args)).unwrap_or(1))
}
