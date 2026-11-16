//! `bootstrap_canary` executable (see `terraforming_runtime::bootstrap_canary`).
fn main() {
    std::process::exit(terraforming_runtime::bootstrap_canary::main(
        &terraforming_runtime::argp::argv(),
    ));
}
