//! `regen_envelope` executable (see `terraforming_runtime::regen_envelope`).
fn main() {
    std::process::exit(terraforming_runtime::regen_envelope::main(
        &terraforming_runtime::argp::argv(),
    ));
}
