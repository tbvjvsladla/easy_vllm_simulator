//! `native_remote` — the fixed sub-node runner of `native_multinode_serve` (see `upstream_runtime::native_multinode` module
//! `remote`). Delivered to the Sub with the other sub-runner ELFs; no Python and no cargo is needed where it runs.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(u8::try_from(upstream_runtime::native_multinode::remote_main(&args)).unwrap_or(1))
}
