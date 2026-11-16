use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::pystr::PyStr;

fn main() -> ExitCode {
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    let status = sweep_stop::sweep_repeats_load_shim::main(&args);
    ExitCode::from(u8::try_from(status).unwrap_or(2))
}
