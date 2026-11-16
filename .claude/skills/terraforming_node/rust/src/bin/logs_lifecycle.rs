//! `logs_lifecycle` executable (see `terraforming_runtime::logs_lifecycle`).
fn main() {
    std::process::exit(terraforming_runtime::logs_lifecycle::main(
        &terraforming_runtime::argp::argv(),
    ));
}
