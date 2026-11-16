//! Port of `relay.py` — the main↔sub turn-based relay (A2A plane). Same command line, exit codes, standard output/error
//! text, ledger/pending/request/report files and JSON key order as the original. The in-process Python imports become:
//! `turn_budget` → `role_contract::turn_budget` (Rust), `agent_control` → `crate::agent_control` (Rust) and its
//! `agent_control.sh` launcher for `runners`/`invoke`, `bootstrap_canary` → `crate::bootstrap_canary`, `campaign_init`
//! → `role_contract::campaign` (paths, ACTIVE) plus the `campaign_init.sh` command line (lifecycle fold, slice emission,
//! sub echo). No Python interpreter is started.

pub mod cli;
pub mod core;
pub mod dispatch;
pub mod pv;
pub mod selftest;

pub fn main(argv: &[String]) -> i32 {
    crate::sysexit::run(cli::main(argv))
}
