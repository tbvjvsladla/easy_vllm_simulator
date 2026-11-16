//! `adversarial_stress` executable (see `terraforming_runtime::stress`).
fn main() {
    std::process::exit(terraforming_runtime::stress::main(
        &terraforming_runtime::argp::argv(),
    ));
}
