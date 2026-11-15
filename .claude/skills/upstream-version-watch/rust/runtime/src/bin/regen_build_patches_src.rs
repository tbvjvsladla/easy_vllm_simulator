use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = upstream_runtime::regen_patches_src::run(&args);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
