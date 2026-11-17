//! `runtime_selftest.py` `main()`: `--tripwires-only` (pre-commit) and the full regression suite.
use crate::argp::{self, Kind as K, Opt};
use crate::tripwires;
use std::path::Path;

pub const USAGE: &str = "usage: runtime_selftest.sh [-h] [--tripwires-only]\n";

pub fn main(argv: &[String], repo: &Path) -> i32 {
    let opts = [Opt { long: "--tripwires-only", kind: K::Flag }];
    let help = format!("{USAGE}{}", include_str!("runtime_selftest_help.txt"));
    let a = match argp::parse(argv, &opts, "runtime_selftest.sh", USAGE, &help) {
        Ok(a) => a,
        Err(e) => return argp::finish(e),
    };
    if a.flag("--tripwires-only") {
        return tripwires::run_tripwires(repo, repo);
    }
    crate::suite::run(repo)
}
