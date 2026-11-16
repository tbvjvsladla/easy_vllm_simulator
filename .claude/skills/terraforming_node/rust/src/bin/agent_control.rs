//! `agent_control` executable (see `terraforming_runtime::agent_control`).
fn main() {
    std::process::exit(terraforming_runtime::agent_control::main(
        &terraforming_runtime::argp::argv(),
    ));
}
