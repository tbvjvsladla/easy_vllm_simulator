use agent_runtime_support::pystr::PyStr;
use recipe_runtime::cli::emit;
use recipe_runtime::model_cli::crosscheck_main_real;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    let status = emit(&crosscheck_main_real(&args, Some(&columns)));
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
