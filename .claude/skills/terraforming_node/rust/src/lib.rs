//! Rust ports of `terraforming_node` programs. Each module keeps the original program's command line, exit codes,
//! standard output/error text and written files; `src/bin/<program>.rs` is the executable the `scripts/<program>.sh`
//! launcher starts. No module starts a Python interpreter.

pub mod agent_card;
pub mod agent_guard;
pub mod argp;
pub mod audit_ssh;
pub mod bb;
pub mod blackbox_collect;
pub mod blackbox_eta;
pub mod blackbox_events;
pub mod blackbox_session;
pub mod blackbox_thermal;
pub mod gz;
pub mod library_access;
pub mod logs_lifecycle;
pub mod owner;
pub mod pathfmt;
pub mod proc;
pub mod publish_install_request;
pub mod py;
pub mod pyre;
pub mod pytime;
pub mod raise;
pub mod regen_envelope;
pub mod seed_journal;
pub mod stress;
// A2A control plane (relay, agent_control and its providers, sub-env rendering, canary, probes, gates).
pub mod agent_control;
pub mod bootstrap_canary;
pub mod check_sub_hint_equip;
pub mod harness_permission_probe;
pub mod provider_claude_code;
pub mod provider_codex_cli;
pub mod relay;
pub mod render_sub_env;
pub mod staleness_gate;
pub mod sysexit;
// Rust self-test suites of the retired scripts/selftest_*.py programs (plan_26111615 pass 3).
pub mod selftests;
