//! `python3 -m unittest companion_predicate` replacement (exit 0 all pass / 1 any failure; see `unit.rs`).
fn main() {
    std::process::exit(policy_predicates::unit::run(policy_predicates::companion::cases()));
}
