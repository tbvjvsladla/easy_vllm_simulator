//! The public `campaign_init` command line (launcher: terraforming_node/scripts/campaign_init.sh), answered entirely by this
//! process: `campaign_args` parses it like the original's argparse, `campaign_write::dispatch` runs the original `main`'s
//! order, and the two read shapes it hands back (`--active`, `--derive`) are answered by `campaign_read`. No shape starts a
//! Python interpreter.
//!
//! Arguments that are not valid UTF-8 are decoded lossily (documented residual: the original decoded them with
//! `surrogateescape`).

use crate::campaign_read::{self, Command, DeriveRequest, Environment};
use crate::campaign_write::{self, Ctx, Next, Sinks};
use crate::cli::Streams;
use crate::stdout::StdoutProfile;
use std::io::{stderr, stdin, stdout, Write};
use std::os::unix::ffi::OsStrExt;

/// Runs the program for the process arguments (without the program name); returns the exit status.
pub fn main(args: &[Vec<u8>]) -> i32 {
    let environment = Environment::from_process();
    let Some(owner) = environment.owner.clone() else {
        let _ = writeln!(
            stderr(),
            "[campaign_read] ERROR {} is not set; the launcher campaign_init.sh exports the program path beside it",
            campaign_read::OWNER_ENV
        );
        return campaign_read::EXIT_CONTROL_FAILURE;
    };
    let site = match role_contract::campaign::module_site(&owner) {
        Ok(site) => site,
        Err(_) => {
            let _ = writeln!(
                stderr(),
                "[campaign_read] ERROR the checkout root cannot be derived from {}",
                String::from_utf8_lossy(&owner)
            );
            return campaign_read::EXIT_CONTROL_FAILURE;
        }
    };
    let ctx = Ctx {
        root: std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&site.root)),
    };
    let argv: Vec<String> = args.iter().map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    let next = {
        let (mut out, mut err) = (stdout().lock(), stderr().lock());
        let next = campaign_write::run_argv(&ctx, &argv, &mut Sinks { out: &mut out, err: &mut err });
        let _ = out.flush();
        next
    };
    let command = match next {
        Next::Done(status) => return status,
        Next::Active => Command::Active,
        Next::Derive { kind, cell, sweep, context, campaign_id } => Command::Derive(DeriveRequest {
            kind,
            cell,
            sweep,
            context,
            campaign_id,
        }),
    };
    let (mut input, mut output, mut errors) = (stdin().lock(), stdout().lock(), stderr().lock());
    campaign_read::run_command(
        &command,
        &environment,
        &mut Streams::new(&mut input, &mut output, &mut errors, StdoutProfile::of_process()),
    )
}
