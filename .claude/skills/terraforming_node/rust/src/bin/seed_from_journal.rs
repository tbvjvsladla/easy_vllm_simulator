//! `seed_from_journal` executable (see `terraforming_runtime::seed_journal`).
fn main() {
    std::process::exit(terraforming_runtime::seed_journal::main(
        &terraforming_runtime::argp::argv(),
    ));
}
