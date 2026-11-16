//! `audit_session_ssh` executable (see `terraforming_runtime::audit_ssh`).
fn main() {
    std::process::exit(terraforming_runtime::audit_ssh::main(
        &terraforming_runtime::argp::argv(),
    ));
}
