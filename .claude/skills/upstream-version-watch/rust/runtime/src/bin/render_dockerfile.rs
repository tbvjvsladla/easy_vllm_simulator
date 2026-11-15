use std::process::ExitCode;

fn main() -> ExitCode {
    let (script, args) = upstream_runtime::gate_cli::process_args();
    let status = upstream_runtime::render_dockerfile::run(script.as_ref(), &args);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
