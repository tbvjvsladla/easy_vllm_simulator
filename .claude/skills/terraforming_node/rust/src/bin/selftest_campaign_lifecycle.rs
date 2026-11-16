//! `selftest_campaign_lifecycle` executable (see `terraforming_runtime::selftests::campaign_lifecycle`; launcher `scripts/selftest_campaign_lifecycle.sh`).
fn main() {
    std::process::exit(terraforming_runtime::selftests::campaign_lifecycle::run());
}
