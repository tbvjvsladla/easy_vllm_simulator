//! `blackbox_events` executable (see `terraforming_runtime::blackbox_events`).
fn main() {
    std::process::exit(terraforming_runtime::blackbox_events::main(
        &terraforming_runtime::argp::argv(),
    ));
}
