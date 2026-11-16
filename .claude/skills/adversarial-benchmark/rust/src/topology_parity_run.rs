//! The program body of the private helper `topology_parity`: the owner CLI of `topology_parity.py` over the native predicate
//! (the `role-contract` library, the Rust owner).
//!
//! ```text
//! topology_parity [PRIVATE FLAGS] <owner CLI args>
//!   --source-owner PATH       logical owner path shown in traceback frames: metadata only, never read, never run
//!   --route-sidecar PATH      optional exclusive private observation file (JSON)
//! ```
//!
//! **Route.** Always `NativeCurrentSource`: the native predicate answers and no Python runs, whatever bytes the checkout's
//! `topology_parity.py` / `campaign_init.py` hold or whether they exist. The former source-version compatibility route (which ran
//! a changed original program under an explicit interpreter, with a supervising watchdog) is retired (plan_26111615 pass 3): the
//! Rust library is the owner, so a changed or missing `.py` no longer changes which program judges. The retired private flags
//! (`--compat-owner`, `--compat-python`, `--compat-deadline-ms`) are refused with status 2 and a named reason, never ignored.
//!
//! **Output.** Business bytes go out through the interpreter's stdout model ([`crate::stdout`]): a write that fails at once is
//! the traceback and status 1 of CPython, a failure when the buffered rest is flushed is its status 120. Status 2 outside argparse
//! usage errors is this helper's private domain (unsupported text profile, a retired flag, a sidecar that cannot be created).

use crate::cli::{write_stderr, Streams};
use crate::pystr::PyStr;
use crate::repeat_api_cli::print_failed;
use role_contract::pyprofile::TextProfile;
use role_contract::topology_cli::{run as run_native, Environment};
use std::os::unix::ffi::OsStrExt;

const DEFAULT_OWNER: &[u8] = b".claude/policies/runtime/topology_parity.py";
/// Private flags of the retired compatibility route: refused by name.
const RETIRED_FLAGS: [&[u8]; 3] = [b"--compat-owner", b"--compat-python", b"--compat-deadline-ms"];

#[derive(Default)]
struct Private {
    source_owner: Option<Vec<u8>>,
    sidecar: Option<Vec<u8>>,
    retired: Option<Vec<u8>>,
}

/// Splits the private leading flags from the owner's command line.
fn split_private(mut args: Vec<Vec<u8>>) -> (Private, Vec<Vec<u8>>) {
    let mut private = Private::default();
    loop {
        let Some(first) = args.first().cloned() else { break };
        if RETIRED_FLAGS.contains(&first.as_slice()) {
            private.retired = Some(first);
            break;
        }
        let slot = match first.as_slice() {
            b"--source-owner" => &mut private.source_owner,
            b"--route-sidecar" => &mut private.sidecar,
            _ => break,
        };
        if args.len() < 2 {
            break;
        }
        *slot = Some(args[1].clone());
        args.drain(..2);
    }
    (private, args)
}

/// The sidecar record (exclusive creation; a failure to create it is the helper's private error, not a business result).
fn write_sidecar(path: &[u8], fields: &[(&str, String)]) -> Result<(), String> {
    use std::io::Write;
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("\"{k}\": {v}"))
        .collect();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::path::Path::new(std::ffi::OsStr::from_bytes(path)))
        .map_err(|error| format!("route sidecar cannot be created: {error}"))?;
    file.write_all(format!("{{{}}}\n", body.join(", ")).as_bytes())
        .map_err(|error| error.to_string())
}

fn private_error(io: &mut Streams<'_>, text: &str) -> i32 {
    write_stderr(io, &PyStr::new(&format!("topology_parity: {text}\n")));
    io.finish(2)
}

/// Delivers business bytes: stderr directly (a failure is the exit-time 120), stdout through the interpreter model: a write that
/// fails at once is CPython's traceback and status 1, a failure of the final flush its status 120.
fn deliver(io: &mut Streams<'_>, stdout: &[u8], stderr: &[u8], status: i32) -> i32 {
    if io.stderr.write_all(stderr).is_err() || io.stderr.flush().is_err() {
        return 120;
    }
    if !stdout.is_empty() {
        if let Err(failure) = io.write_plain(&PyStr::from_surrogateescape(stdout)) {
            let status = print_failed(io, &failure);
            return io.finish(status);
        }
    }
    io.finish(status)
}

/// Runs the helper: `args` excludes the program name; `env` reads one environment variable.
pub fn run(args: Vec<Vec<u8>>, io: &mut Streams<'_>, env: &dyn Fn(&str) -> Option<Vec<u8>>) -> i32 {
    let (private, args) = split_private(args);
    if let Some(flag) = private.retired {
        return private_error(
            io,
            &format!(
                "{} belongs to the retired source-version compatibility route (plan_26111615): the native predicate is the only judge",
                String::from_utf8_lossy(&flag)
            ),
        );
    }
    let profile = match TextProfile::from_environment(env) {
        Ok(profile) => profile,
        Err(why) => return private_error(io, &format!("unsupported text profile: {why}")),
    };
    let environment = Environment {
        profile,
        source_owner: private
            .source_owner
            .clone()
            .unwrap_or_else(|| DEFAULT_OWNER.to_vec()),
        tmpdir: env("TMPDIR"),
    };
    if let Some(path) = &private.sidecar {
        if let Err(why) = write_sidecar(
            path,
            &[
                ("route", "\"NativeCurrentSource\"".to_string()),
                ("native", "true".to_string()),
                ("python_children", "0".to_string()),
            ],
        ) {
            return private_error(io, &why);
        }
    }
    let out = run_native(&args, &environment);
    deliver(io, &out.stdout, &out.stderr, out.status)
}
