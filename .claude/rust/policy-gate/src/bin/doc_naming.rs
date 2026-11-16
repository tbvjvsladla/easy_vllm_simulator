//! `doc_naming.py` command line (launcher: `.claude/policies/runtime/doc_naming.sh`).
fn main() {
    let argv: Vec<String> = std::env::args_os().map(|a| a.to_string_lossy().into_owned()).collect();
    let (out, err, code) = policy_gate::doc_naming::main_cli(&argv);
    std::process::exit(policy_gate::cli::finish_process(policy_gate::cli::Outcome { stdout: out, stderr: err, code }));
}
