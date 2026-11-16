//! `selftest_claude_native_events` executable (see `terraforming_runtime::selftests::claude_native_events`; launcher `scripts/selftest_claude_native_events.sh`).
fn main() {
    std::process::exit(terraforming_runtime::selftests::claude_native_events::run());
}
