//! `agent_card_contract` executable (see `terraforming_runtime::agent_card`).
fn main() {
    std::process::exit(terraforming_runtime::agent_card::main(
        &terraforming_runtime::argp::argv(),
    ));
}
