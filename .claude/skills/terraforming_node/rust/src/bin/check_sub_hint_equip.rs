//! `check_sub_hint_equip` executable (see `terraforming_runtime::check_sub_hint_equip`).
fn main() {
    std::process::exit(terraforming_runtime::check_sub_hint_equip::main(
        &terraforming_runtime::argp::argv(),
    ));
}
