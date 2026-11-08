//! The command line `resolve_ngc_tag.py` as a process: `main()` over `sys.argv`, the status the interpreter would end with and the report of an
//! exception nothing handles. Standard output is written live through CPython's finite checked buffering ([`LiveConsole`]); the exit flush of a
//! closed pipe is the interpreter's status 120.
//!
//! **Entry.** The executable may be copied to a path that ends in `resolve_ngc_tag.py` and executed directly (no interpreter): `argv[0]` is then
//! the logical script path and the frames of an uncaught exception name it. Under any other file name the usage text and the frames use the
//! canonical name. A relative script path is made absolute against the working directory; symbolic links are not resolved.
//!
//! The architecture default is the machine name of this host (`uname`), mapped as the original maps `platform.machine()`; `PYTHONIOENCODING` may
//! be unset or `ascii:strict` (any other value is refused by name).

use crate::live::LiveConsole;
use crate::ngc_inspect::{Docker, ProcessDocker};
use crate::ngc_main::{main, NgcWorld, Stop, PROGRAM};
use crate::runtime_patch::StdioProfile;
use crate::torch_main::{Refusal, REFUSAL_STATUS};
use crate::torch_traceback::render;
use agent_runtime_support::pystr::PyStr;
use std::os::unix::ffi::OsStrExt;

fn script_file(argv0: Option<&PyStr>) -> PyStr {
    let Some(path) = argv0.filter(|path| crate::main_cmd::basename(path) == PROGRAM) else {
        return PyStr::new(PROGRAM);
    };
    let points = path.code_points();
    let is_relative = points.first() != Some(&u32::from('/'));
    let has_directory = points.contains(&u32::from('/'));
    match std::env::current_dir() {
        Ok(cwd) if is_relative && has_directory => {
            let mut absolute = PyStr::from_surrogateescape(cwd.as_os_str().as_bytes());
            absolute.push_str("/");
            absolute.push_py(path);
            absolute
        }
        _ => path.clone(),
    }
}

/// The named report of an outcome this port does not decide.
pub fn refusal_report(refusal: &Refusal) -> PyStr {
    PyStr::new(&format!(
        "{PROGRAM}: not decided by this port ({}): {}\n",
        refusal.boundary, refusal.detail
    ))
}

#[repr(C)]
struct UtsName {
    fields: [[u8; 65]; 6],
}

extern "C" {
    fn uname(buffer: *mut UtsName) -> i32;
}

/// `platform.machine()`: the machine field of `uname` (empty when the call fails, as the module returns).
pub fn machine_name() -> PyStr {
    let mut names = UtsName {
        fields: [[0; 65]; 6],
    };
    // SAFETY: `names` is a live, correctly sized and aligned `struct utsname` (six 65-byte fields on Linux); `uname` only writes into it.
    if unsafe { uname(&mut names) } != 0 {
        return PyStr::default();
    }
    let machine = &names.fields[4];
    let length = machine
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(machine.len());
    PyStr::from_surrogateescape(&machine[..length])
}

/// Runs the program over `argv` (the whole `sys.argv`) on `console` and ends the streams as the interpreter does.
pub fn run(
    console: &mut LiveConsole,
    argv: &[PyStr],
    columns: Option<&str>,
    profile: StdioProfile,
    docker: &dyn Docker,
    machine: PyStr,
) -> i32 {
    let script = script_file(argv.first());
    let mut world = NgcWorld {
        host: console,
        argv,
        columns,
        profile,
        docker,
        machine,
        frames: Vec::new(),
    };
    let end = main(&mut world);
    let frames = std::mem::take(&mut world.frames);
    let status = match end {
        Ok(()) => 0,
        Err(Stop::Exit(code)) => code,
        Err(Stop::Uncaught(report)) => {
            let text = if frames.is_empty() {
                report
            } else {
                render(&frames, &script, &report)
            };
            console.write_uncaught(&text);
            1
        }
        Err(Stop::Refused(refusal)) => {
            console.write_uncaught(&refusal_report(&refusal));
            REFUSAL_STATUS
        }
    };
    console.shutdown(status)
}

/// The stack the JSON reader recurses on (one level per nesting of the inspected document).
const READER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// The program as a process, on a thread with the reader's stack. `script` is the caller's `argv[0]`; `args` are the arguments after it.
pub fn run_process(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let (script, args, columns) = (script.cloned(), args.to_vec(), columns.map(str::to_string));
    let spawned = std::thread::Builder::new()
        .stack_size(READER_STACK_BYTES)
        .spawn(move || run_on_this_thread(script.as_ref(), &args, columns.as_deref()));
    let outcome = match spawned {
        Ok(handle) => handle.join().map_err(|_| "the run panicked".to_string()),
        Err(error) => Err(format!("no thread with the reader's stack: {error}")),
    };
    outcome.unwrap_or_else(|detail| {
        let mut console = LiveConsole::for_streams();
        console.write_uncaught(&refusal_report(&Refusal {
            boundary: "process_stack",
            detail,
        }));
        console.shutdown(REFUSAL_STATUS)
    })
}

fn run_on_this_thread(script: Option<&PyStr>, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = LiveConsole::for_streams();
    let setting = std::env::var_os("PYTHONIOENCODING");
    let profile = match StdioProfile::from_setting(setting.as_deref().map(|value| value.as_bytes()))
    {
        Ok(profile) => profile,
        Err(refusal) => {
            console.write_uncaught(&refusal_report(&refusal));
            return console.shutdown(REFUSAL_STATUS);
        }
    };
    let mut argv = vec![script_file(script)];
    argv.extend(args.iter().cloned());
    run(
        &mut console,
        &argv,
        columns,
        profile,
        &ProcessDocker::default(),
        machine_name(),
    )
}
