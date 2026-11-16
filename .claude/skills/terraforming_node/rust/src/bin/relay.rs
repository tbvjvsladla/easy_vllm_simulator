//! `relay` executable (see `terraforming_runtime::relay`).
fn main() {
    std::process::exit(terraforming_runtime::relay::main(
        &terraforming_runtime::argp::argv(),
    ));
}
