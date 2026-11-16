//! `antipattern_scan.py` (harness antipattern inventory + removal tripwire) as a native program.
fn main() {
    let repo = policy_selftest::repo_root();
    let code = policy_selftest::antipattern::main(&policy_selftest::args(), &repo.to_string_lossy());
    std::process::exit(code);
}
