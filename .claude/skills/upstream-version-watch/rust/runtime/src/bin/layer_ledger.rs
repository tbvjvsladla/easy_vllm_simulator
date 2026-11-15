use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(u8::try_from(upstream_runtime::layer_ledger::run(&args)).unwrap_or(1))
}
