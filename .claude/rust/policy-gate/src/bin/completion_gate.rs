//! `completion_gate.py` command line (launcher: `.claude/policies/runtime/completion_gate.sh`).
fn main() {
    let argv: Vec<String> = std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    let o = policy_gate::cli::completion_gate_main(&argv);
    std::process::exit(policy_gate::cli::finish_process(o));
}
