//! `publish_install_request` executable (see `terraforming_runtime::publish_install_request`).
fn main() {
    std::process::exit(terraforming_runtime::publish_install_request::main(
        &terraforming_runtime::argp::argv(),
    ));
}
