//! `selftest_bootstrap_permissions` executable (see `terraforming_runtime::selftests::bootstrap_permissions`; launcher `scripts/selftest_bootstrap_permissions.sh`).
fn main() {
    std::process::exit(terraforming_runtime::selftests::bootstrap_permissions::run());
}
