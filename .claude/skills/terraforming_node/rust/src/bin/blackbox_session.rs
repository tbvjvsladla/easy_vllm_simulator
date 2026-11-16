//! `blackbox_session` executable (see `terraforming_runtime::blackbox_session`).
fn main() {
    std::process::exit(terraforming_runtime::blackbox_session::main(
        &terraforming_runtime::argp::argv(),
    ));
}
