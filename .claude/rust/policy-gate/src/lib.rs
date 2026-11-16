//! `policy-gate`: Rust ports of `.claude/policies/runtime/completion_gate.py` and `doc_naming.py`.
//!
//! * [`completion_gate`] — the verify/authorize gate (library API for the other policy ports + the CLI body).
//! * [`doc_naming`] — the naming SSOT shim: prose names from `wiki-desk/src/doc_naming.ts` (via `node`), bench
//!   names from the adversarial-benchmark Rust owner (via its `bench_reporting_api` launcher), as the original.

pub mod argp;
pub mod cli;
pub mod completion_gate;
pub mod doc_naming;
pub mod fsx;
pub mod markdown;
pub mod pytext;
