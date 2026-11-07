use agent_runtime_support::pystr::PyStr;
use recipe_runtime::anchor::process_skill_root;
use recipe_runtime::memory_cli::run_process;
use recipe_runtime::memory_system::System;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    let skill_root = match process_skill_root() {
        Ok(root) => root,
        Err(failure) => {
            let _ = writeln!(
                std::io::stderr(),
                "preload_ram_gate: {}",
                failure.line().to_utf8().unwrap_or_default()
            );
            return ExitCode::from(1);
        }
    };
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    let status = run_process(&mut System::new(), &skill_root, &args, Some(&columns));
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
