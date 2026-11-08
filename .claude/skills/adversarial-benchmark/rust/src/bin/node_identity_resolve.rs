//! Private helper of `classify_cell`'s node-identity lookup: the native counterpart of
//! `bash node_identity.sh --resolve --repo REPO`.
//!
//! Interface (a private contract between [`Owners::resolve_self_node`] and this program, not a public CLI):
//!
//! ```text
//! node_identity_resolve --repo <literal repository spelling> [--source-owner <logical path of node_identity.sh>]
//! ```
//!
//! Standard output, standard error and the exit status are the raw bytes and the status the original left for a
//! non-empty repository (`0` resolved, `1` fail-loud). `2` is this helper's own usage or platform error (never an answer of the original: the page size of the host unavailable). `--source-owner` only names
//! the logical source whose behavior this program is compatible with (it is where a Bash warning line would have named
//! its script); it is not a Bash script or OS stack and is never executed or read. The locale profile follows the
//! `LC_ALL` / `LC_CTYPE` / `LANG` precedence of the environment (see `role_contract::identity::Profile`). Nothing is
//! forked: the file-system reads are done in this process.

use role_contract::identity::{resolve, Profile};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

const USAGE: &str =
    "usage: node_identity_resolve --repo <repo> [--source-owner <node_identity.sh path>]";

fn usage_error(message: &str) -> ExitCode {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "node_identity_resolve: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut repo: Option<OsString> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_bytes() {
            b"--repo" => match args.next() {
                Some(value) => repo = Some(value),
                None => return usage_error("--repo needs a value"),
            },
            b"--source-owner" => {
                if args.next().is_none() {
                    return usage_error("--source-owner needs a value");
                }
            }
            _ => return usage_error(&format!("unknown argument {:?}", argument)),
        }
    }
    let Some(repo) = repo.filter(|value| !value.is_empty()) else {
        return usage_error("a non-empty --repo is required");
    };
    let env = |name: &str| std::env::var_os(name);
    let (lc_all, lc_ctype, lang) = (env("LC_ALL"), env("LC_CTYPE"), env("LANG"));
    let profile = Profile::from_locale_variables(
        lc_all.as_deref().map(|v| v.as_bytes()),
        lc_ctype.as_deref().map(|v| v.as_bytes()),
        lang.as_deref().map(|v| v.as_bytes()),
    );
    let resolution = resolve(repo.as_bytes(), profile);
    let _ = std::io::stdout().lock().write_all(&resolution.stdout);
    let _ = std::io::stderr().lock().write_all(&resolution.stderr);
    ExitCode::from(u8::try_from(resolution.status).unwrap_or(1))
}
