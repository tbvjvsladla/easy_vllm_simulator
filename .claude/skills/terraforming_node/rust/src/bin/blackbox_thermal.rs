//! `blackbox_thermal` executable (see `terraforming_runtime::blackbox_thermal`).
fn main() {
    std::process::exit(terraforming_runtime::blackbox_thermal::main(
        &terraforming_runtime::argp::argv(),
    ));
}
