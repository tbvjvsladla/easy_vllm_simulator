//! Public `campaign_init` command line (launcher: terraforming_node/scripts/campaign_init.sh): every shape is answered by this
//! process (`sweep_stop::campaign_main`); none starts a Python interpreter.
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<Vec<u8>> = std::env::args_os().skip(1).map(|arg| arg.as_bytes().to_vec()).collect();
    ExitCode::from(u8::try_from(sweep_stop::campaign_main::main(&args)).unwrap_or(1))
}
