use agent_runtime_support::pystr::PyStr;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use upstream_runtime::classify_cli::run_process;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().map(|arg| PyStr::from_surrogateescape(arg.as_bytes()));
    // `argv[0]` is the caller's fact: the script's path when the legacy entry executes this program in its place
    let script = arguments.next();
    let args: Vec<PyStr> = arguments.collect();
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    let status = run_process(script.as_ref(), &args, Some(&columns));
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
