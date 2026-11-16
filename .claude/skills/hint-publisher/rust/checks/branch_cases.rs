//! `branch` checks (owner: the branch port): the module self-test (isolated temp repositories + local bare remotes).
use hintlib::branch;

#[test]
fn branch_selftest_passes() {
    let bad = branch::selftest();
    assert!(bad.is_empty(), "branch selftest failures:\n{}", bad.join("\n"));
}

#[test]
fn allowlist_pure() {
    assert_eq!(branch::allowlist_violation("00-hint.md").unwrap(), None);
    assert!(branch::allowlist_violation("artifacts/x/y").unwrap().is_some());
    assert_eq!(branch::required_top(&hintlib::value::J::Null).unwrap().len(), 8);
}
