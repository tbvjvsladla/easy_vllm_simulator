//! `selftest_harness_switch` executable (see `terraforming_runtime::selftests::harness_switch`; launcher `scripts/selftest_harness_switch.sh`).
fn main() {
    std::process::exit(terraforming_runtime::selftests::harness_switch::run());
}
