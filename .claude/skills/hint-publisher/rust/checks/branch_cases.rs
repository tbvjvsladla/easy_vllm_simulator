//! `branch` checks (owner: the branch port): the module self-test (isolated temp repositories + local bare remotes).
use hintlib::branch;

#[test]
fn branch_selftest_passes() {
    let bad = branch::selftest();
    assert!(bad.is_empty(), "branch selftest failures:\n{}", bad.join("\n"));
}

/// plan_26112012 #1 (규칙의 권위 = ENV_SECRET_KEY_LEDGER.C3): branch push 는 메인에서 ssh 키로만. 상태를 바꾸는 수용 사례
/// (https 원격 → ref·원격 불변, 메인 선언 없음, ssh 원격 → 기존 결과)는 `branch::selftest` 안에 있다(위 시험 · `hint.sh --self-test`).
/// 여기는 그 앞 판정 두 개다 — URL 원격은 git 을 부르지 않고 판정되며, 선언 파일 판정은 파일 시스템만 본다.
#[test]
fn branch_push_is_ssh_and_main_only() {
    let td = hintlib::core::TempDir::new("hint-branch-push-plane-");
    let dir = td.path();
    let code = |r: Result<(), hintlib::core::HintError>| r.err().map(|e| e.code);
    for url in ["https://example.invalid/hint.git", "HTTPS://example.invalid/hint.git", "http://example.invalid/hint.git", "git://example.invalid/hint.git"] {
        assert_eq!(code(branch::_require_branch_push_ssh(dir, url)).as_deref(), Some("HINT_BRANCH_TRANSITION_SSH_ONLY"), "{url}");
    }
    for url in ["ssh://git@example.invalid/hint.git", "git+ssh://git@example.invalid/hint.git", "../remote.git", "/srv/hint.git"] {
        assert_eq!(code(branch::_require_branch_push_ssh(dir, url)), None, "{url}");
    }
    assert_eq!(branch::BRANCH_PUSH_TRANSPORTS, ["local", "ssh"]);

    assert_eq!(code(branch::_require_branch_push_main(dir)).as_deref(), Some("HINT_BRANCH_TRANSITION_MAIN_ONLY"));
    std::fs::create_dir_all(dir.join("hints")).unwrap();
    std::fs::write(dir.join(hintlib::core::REL_PUSH_FLAG), "").unwrap();
    assert_eq!(code(branch::_require_branch_push_main(dir)).as_deref(), Some("HINT_BRANCH_TRANSITION_MAIN_ONLY"), "push role is not the main declaration");
    std::fs::write(dir.join(hintlib::core::REL_CENTRAL_FLAG), "").unwrap();
    assert_eq!(code(branch::_require_branch_push_main(dir)), None);
}

#[test]
fn allowlist_pure() {
    assert_eq!(branch::allowlist_violation("00-hint.md").unwrap(), None);
    assert!(branch::allowlist_violation("artifacts/x/y").unwrap().is_some());
    assert_eq!(branch::required_top(&hintlib::value::J::Null).unwrap().len(), 8);
}
