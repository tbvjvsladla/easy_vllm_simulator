//! `harness_permission_probe` executable (see `terraforming_runtime::harness_permission_probe`).
fn main() {
    std::process::exit(terraforming_runtime::harness_permission_probe::main(
        &terraforming_runtime::argp::argv(),
    ));
}
