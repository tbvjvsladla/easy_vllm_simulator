//! `constitution_lint.py` (constitution quality lint, information only) as a native program.
fn main() {
    std::process::exit(policy_selftest::constitution_lint::main(&policy_selftest::args()));
}
