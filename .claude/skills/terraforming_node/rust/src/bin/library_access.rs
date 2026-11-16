//! `library_access` executable (see `terraforming_runtime::library_access`).
fn main() {
    std::process::exit(terraforming_runtime::library_access::main(
        &terraforming_runtime::argp::argv(),
    ));
}
