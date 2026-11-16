//! `staleness_gate` executable (see `terraforming_runtime::staleness_gate`).
fn main() {
    std::process::exit(terraforming_runtime::staleness_gate::main(
        &terraforming_runtime::argp::argv(),
    ));
}
