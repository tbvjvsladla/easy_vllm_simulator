//! `blackbox_eta` executable (see `terraforming_runtime::blackbox_eta`).
fn main() {
    std::process::exit(terraforming_runtime::blackbox_eta::main(
        &terraforming_runtime::argp::argv(),
    ));
}
