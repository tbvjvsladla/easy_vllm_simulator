//! `blackbox_collect` executable (see `terraforming_runtime::blackbox_collect`).
fn main() {
    std::process::exit(terraforming_runtime::blackbox_collect::main(
        &terraforming_runtime::argp::argv(),
    ));
}
