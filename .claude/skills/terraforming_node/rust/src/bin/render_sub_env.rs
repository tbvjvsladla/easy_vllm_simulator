//! `render_sub_env` executable (see `terraforming_runtime::render_sub_env`).
fn main() {
    std::process::exit(terraforming_runtime::render_sub_env::main(
        &terraforming_runtime::argp::argv(),
    ));
}
