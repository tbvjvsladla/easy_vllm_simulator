//! Public `campaign_init.py` command line (launcher: terraforming_node/scripts/campaign_init.sh).
//!
//! The two read shapes (`--active`, `--derive`) are answered by `campaign_read`; the writer shapes that
//! `campaign_write` models are answered natively; every other shape reaches the owner program unchanged through the
//! legacy route of `campaign_read`.
use std::io::{stderr, stdin, stdout};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::campaign_read::{parse_exact, run, Environment, ExecLegacy};
use sweep_stop::campaign_write::{self, Ctx, Outcome, Sinks};
use sweep_stop::cli::Streams;
use sweep_stop::stdout::StdoutProfile;

fn native_write(env: &Environment, args: &[Vec<u8>]) -> Option<i32> {
    if env.explicit_python.is_some() {
        return None;
    }
    let owner = env.owner.as_ref()?;
    let site = role_contract::campaign::module_site(owner).ok()?;
    let argv: Vec<String> = args
        .iter()
        .map(|a| String::from_utf8(a.clone()))
        .collect::<Result<_, _>>()
        .ok()?;
    let ctx = Ctx {
        root: std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&site.root)),
    };
    let (mut out, mut err) = (stdout().lock(), stderr().lock());
    let outcome = campaign_write::run(&ctx, &argv, &mut Sinks { out: &mut out, err: &mut err });
    match outcome {
        Outcome::Done(status) => Some(status),
        Outcome::Fallback => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.as_bytes().to_vec())
        .collect();
    let environment = Environment::from_process();
    if parse_exact(&args).is_none() {
        if let Some(status) = native_write(&environment, &args) {
            return ExitCode::from(u8::try_from(status).unwrap_or(1));
        }
    }
    let (mut input, mut output, mut errors) = (stdin().lock(), stdout().lock(), stderr().lock());
    let status = run(
        &args,
        &environment,
        &mut Streams::new(&mut input, &mut output, &mut errors, StdoutProfile::of_process()),
        &mut ExecLegacy,
    );
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
