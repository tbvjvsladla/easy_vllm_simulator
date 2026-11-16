//! `verify_distribution` — native entry of `.claude/policies/runtime/verify_distribution.sh`.
fn main() {
    let argv: Vec<String> = std::env::args().collect();
    std::process::exit(policy_distribution::cli::main_with(argv));
}
