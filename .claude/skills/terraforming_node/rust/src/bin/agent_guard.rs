//! `agent_guard` executable (see `terraforming_runtime::agent_guard`).
fn main() {
    std::process::exit(terraforming_runtime::agent_guard::main(
        &terraforming_runtime::argp::argv(),
    ));
}
