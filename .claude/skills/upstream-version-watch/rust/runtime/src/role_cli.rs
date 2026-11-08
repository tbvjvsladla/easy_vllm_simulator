//! The command line `node_role_contract.py` as a process: `main()` over `sys.argv`, the status the interpreter would end with. Standard output goes
//! through CPython's finite checked buffering ([`LiveConsole`]); the exit flush of a closed pipe is the interpreter's status 120.

use crate::live::LiveConsole;
use crate::role_main::{main, RoleWorld, PROGRAM};
use crate::torch_main::{Refusal, REFUSAL_STATUS};
use agent_runtime_support::pystr::PyStr;

pub fn refusal_report(refusal: &Refusal) -> PyStr {
    PyStr::new(&format!(
        "{PROGRAM}: not decided by this port ({}): {}\n",
        refusal.boundary, refusal.detail
    ))
}

/// Runs the program over `argv` (the whole `sys.argv`) on `console`.
pub fn run(console: &mut LiveConsole, argv: &[PyStr], columns: Option<&str>) -> i32 {
    let end = {
        let mut world = RoleWorld {
            host: console,
            argv,
            columns,
        };
        main(&mut world)
    };
    let status = match end {
        Ok(code) => code,
        Err(refusal) => {
            console.write_uncaught(&refusal_report(&refusal));
            REFUSAL_STATUS
        }
    };
    console.shutdown(status)
}

/// The program as a process. `script` is the caller's `argv[0]`; `args` are the arguments after it.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::for_streams();
    let mut argv = vec![script.cloned().unwrap_or_else(|| PyStr::new(PROGRAM))];
    argv.extend(args.iter().cloned());
    run(&mut console, &argv, columns)
}
