//! `runtime_selftest.py` (pre-commit tripwires + regression suite) as a native program.
fn main() {
    let repo = policy_selftest::repo_root();
    std::process::exit(policy_selftest::selftest::main(&policy_selftest::args(), &repo));
}
