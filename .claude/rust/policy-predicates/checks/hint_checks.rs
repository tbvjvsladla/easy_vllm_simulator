//! Scenario checks of the hint-publisher clauses (HINT_TAG_ACTIVATION_GATE.C1-C6, LAST_GOOD_ROLLBACK_ANCHOR.C3) and of the two
//! ENV_SECRET_KEY_LEDGER clauses this module's predicates own (C1, C3). Nothing here runs Python.
//!   * the real tree: every one of the nine clauses passes;
//!   * the Rust source lexer reads literals/calls/functions the way the source checks need (comments never count);
//!   * the O6 call-graph judge is green on a guarded source and RED on the three regressions the original guards
//!     against (approval moved below the side effect, approval deleted, entry point without approval);
//!   * negative controls: each mutation below, applied to a copy of the tracked tree, fails the targeted clause with
//!     exactly the expected failure (every occurrence of the anchor replaced; an exact message, or a stated prefix where the message lists findings).
//! One sequential test: the predicates read the repository root from `POLICY_PREDICATES_REPO` and the hint sandbox
//! scopes the process environment.
use policy_predicates::claims::{self, Impl};
use policy_predicates::claims_hint as h;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLAUSES: [&str; 9] = [
    "HINT_TAG_ACTIVATION_GATE.C1",
    "HINT_TAG_ACTIVATION_GATE.C2",
    "HINT_TAG_ACTIVATION_GATE.C3",
    "HINT_TAG_ACTIVATION_GATE.C4",
    "HINT_TAG_ACTIVATION_GATE.C5",
    "HINT_TAG_ACTIVATION_GATE.C6",
    "LAST_GOOD_ROLLBACK_ANCHOR.C3",
    "ENV_SECRET_KEY_LEDGER.C1",
    "ENV_SECRET_KEY_LEDGER.C3",
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn run_clause(clause: &str) -> Option<String> {
    let e = claims::predicates().into_iter().find(|e| e.clause_id == clause).unwrap();
    match e.imp {
        Impl::Ported(f) => f().err().map(|x| format!("{}: {}", x.kind, x.message)),
        Impl::NotPorted(_) => panic!("{clause} is not ported"),
    }
}

fn tracked_copy(dst: &Path) {
    let out = Command::new("git").arg("-C").arg(repo()).args(["ls-files", "-z"]).output().unwrap();
    for rel in String::from_utf8(out.stdout).unwrap().split('\0').filter(|r| !r.is_empty()) {
        let src = repo().join(rel);
        let meta = match std::fs::symlink_metadata(&src) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() {
            continue;
        }
        let d = dst.join(rel);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(&src, &d).unwrap();
    }
    // the renderer of C4 asks `git ls-files` for the tracked list: the copy is an index-only git tree
    for args in [&["init", "-q"][..], &["add", "-A"][..]] {
        let st = Command::new("git").arg("-C").arg(dst).args(args).output().unwrap();
        assert!(st.status.success(), "fixture git {args:?}");
    }
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "pp-hint-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// (id, file, old, new, clause, expected failure, exact?)
const MUTATIONS: &[(&str, &str, &str, &str, &str, &str, bool)] = &[
    ("H01-drafts-not-ignored", ".gitignore", "hints/.drafts/", "hints/.drafts-moved/", "HINT_TAG_ACTIVATION_GATE.C1",
     "PredicateFailure: the scaffold staging dir must be gitignored (never at HEAD)", true),
    ("H02-tag-cleanup-strip", ".claude/skills/hint-publisher/rust/src/tag.rs", "\"tag\", \"-a\", \"--no-sign\", \"--cleanup=verbatim\",",
     "\"tag\", \"-a\", \"--no-sign\",", "HINT_TAG_ACTIVATION_GATE.C1",
     "PredicateFailure: the tag message must be kept verbatim (2026-08-20 `## ` loss)", true),
    ("H03-tag-second-call-site", ".claude/skills/hint-publisher/rust/src/tag.rs", "pub fn _rollback_created_tag(",
     "pub fn _second_tag(repo: &Path) -> HR<()> { core::git(repo, &[\"tag\", \"-a\", \"x\"]).map(|_| ()) }\npub fn _rollback_created_tag(",
     "HINT_TAG_ACTIVATION_GATE.C1", "PredicateFailure: exactly one production `git tag -a` call site must create hint tags:", false),
    ("H04-banner-removed", ".claude/skills/hint-publisher/templates/00-hint.prompt.md", "이 자료는 **지도이지 정답이 아니다.**",
     "이 자료는 지도다.", "HINT_TAG_ACTIVATION_GATE.C3",
     "PredicateFailure: 00-hint template must carry the banner outside every PROMPT block/comment: '이 자료는 **지도이지 정답이 아니다.**'", true),
    ("H05-sync-carries-catalog", ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh", "sub_dirty() {",
     "# carries HINTS.md\nsub_dirty() {", "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: sync_to_sub.sh must never carry the catalog or a role marker of its own -- delivery is the render staging", true),
    ("H06-engine-names-relay", ".claude/skills/hint-publisher/rust/src/tag.rs", "pub fn _rollback_created_tag(",
     "pub const _RELAY: &str = \"relay.py\";\npub fn _rollback_created_tag(", "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: .claude/skills/hint-publisher/rust/src/tag.rs must never reference sub delivery/A2A tooling ('relay.py')", true),
    ("H07-engine-ssh-argv", ".claude/skills/hint-publisher/rust/src/tag.rs", "pub fn _rollback_created_tag(",
     "pub fn _ssh_argv() -> [&'static str; 2] { [\"ssh\", \"host\"] }\npub fn _rollback_created_tag(", "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: .claude/skills/hint-publisher/rust/src/tag.rs must never build a remote-shell argv ('ssh') -- publication is local", true),
    ("H08-verify-scans-last-good", ".claude/skills/hint-publisher/rust/src/tag.rs",
     "pub fn verify_local(repo: &Path, tag: &str, lint_fn: Option<LintFn<'_>>, mut infos: Option<&mut Vec<J>>, source_repo: Option<&Path>) -> HR<Vec<String>> {",
     "pub fn verify_local(repo: &Path, tag: &str, lint_fn: Option<LintFn<'_>>, mut infos: Option<&mut Vec<J>>, source_repo: Option<&Path>) -> HR<Vec<String>> {\n    // origin last-good scan",
     "HINT_TAG_ACTIVATION_GATE.C5", "PredicateFailure: verify must not re-introduce an origin-side last-good-* scan (F-6c)", true),
    ("H09-push-widening", ".claude/skills/hint-publisher/rust/src/tag.rs", "    let mut args: Vec<&str> = vec![\"push\"];",
     "    let mut args: Vec<&str> = vec![\"push\", \"--follow-tags\"];", "HINT_TAG_ACTIVATION_GATE.C5",
     "PredicateFailure: git_push_authenticated must never carry the widening argument '--follow-tags'", true),
    ("H10-approval-deleted", ".claude/skills/hint-publisher/rust/src/cli/ops.rs",
     "let ap = evidence::approval_for(repo, &sel[\"campaign\"], &sel[\"cell\"], &sel[\"node\"])?;",
     "let ap = evidence::approval_lookup(repo, &sel[\"campaign\"], &sel[\"cell\"], &sel[\"node\"])?;", "HINT_TAG_ACTIVATION_GATE.C6",
     "PredicateFailure: hint.sh must consult the recorded Y/N (evidence.approval_for) before any branch/tag/push side effect:", false),
    ("H11-skill-rule-dropped", ".claude/skills/hint-publisher/SKILL.md", "무인 자동 태깅 ✗", "무인 자동 태깅 가능", "HINT_TAG_ACTIVATION_GATE.C6",
     "PredicateFailure: .claude/skills/hint-publisher/SKILL.md must state the activation rule: '무인 자동 태깅 ✗'", true),
    ("H12-remote-defaulted", ".claude/skills/hint-publisher/rust/src/tag.rs", "pub fn remote_tag_object(repo: &Path, remote: &str, tag: &str)",
     "pub fn remote_tag_object(repo: &Path, remote: Option<&str>, tag: &str)", "LAST_GOOD_ROLLBACK_ANCHOR.C3",
     "PredicateFailure: hintlib.tag.remote_tag_object must not default the remote -- the operator names it", true),
    ("H13-catalog-hardcodes-url", ".claude/skills/hint-publisher/rust/src/catalog.rs", "pub fn ", "// see https://github.com/x/y\npub fn ",
     "LAST_GOOD_ROLLBACK_ANCHOR.C3", "PredicateFailure: .claude/skills/hint-publisher/rust/src/catalog.rs must not hardcode a project remote URL", true),
    ("H14-origin-url-filled", "manifest.template.yaml", "origin_url: \"\"", "origin_url: \"git@example.invalid:x/y.git\"",
     "LAST_GOOD_ROLLBACK_ANCHOR.C3", "PredicateFailure: origin_url must default empty (user-supplied)", true),
    // plan_26112012 M1-② (h90-h92): judged after the delivery, strictly, never on a ray-worker, exit only after the close.
    ("H15-push-transport-exit-in-transaction", SYNC, "        PUSH_TRANSPORT_BLOCKED+=(\"$t\")\n",
     "        PUSH_TRANSPORT_BLOCKED+=(\"$t\"); exit \"$PUSH_TRANSPORT_EXIT\"\n", "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: check_sub_push_transport must not end the sync itself -- the block is recorded and the exit comes after the transaction closes", true),
    ("H16-push-transport-not-called", SYNC, "    check_sub_push_transport \"$t\"\n", "", "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: the M1-② push-transport judgement must run in each B1 target after verify_checksums (after the delivery)", true),
    ("H17-push-transport-ray-worker-called", SYNC,
     "    if [ \"$mode\" = \"ray-worker\" ]; then\n        echo \"[sync] push-transport($t): not_applicable\"\n        return 0\n    fi\n", "",
     "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: a ray-worker sub has no equip tool: check_sub_push_transport must record not_applicable before (instead of) calling it", true),
    ("H18-push-transport-exit-before-close", SYNC, "\nfinalize_remote_transactions\necho \"[sync] 완료. (현재 서브 브랜치:",
     "\nif [ ${#PUSH_TRANSPORT_BLOCKED[@]} -gt 0 ]; then exit \"$PUSH_TRANSPORT_EXIT\"; fi\nfinalize_remote_transactions\necho \"[sync] 완료. (현재 서브 브랜치:",
     "HINT_TAG_ACTIVATION_GATE.C4",
     "PredicateFailure: the M1-② block must end the sync only after finalize_remote_transactions, so the delivered content stays", true),
    // plan_26112012 #4 (K1): envs/.env is never delivered, its correction never opens the file, the main sync only observes; the PAT
    // is read for an https transport only, the hint branch code never names it, and the branch-transition judges before it acts.
    ("K01-sync-corrects-sub-env", SYNC, "    echo \"[sync] env-mode($t): $v\"\n}",
     "    echo \"[sync] env-mode($t): $v\"\n    sub_run \"chmod 600 envs/.env\" </dev/null\n}", "ENV_SECRET_KEY_LEDGER.C1",
     "PredicateFailure: the main sync must only observe and record the sub's envs/.env mode (no chmod, never blocking)", true),
    ("K02-env-dir-delivered", SYNC, "ls-files -z -- .claude CLAUDE.md ", "ls-files -z -- .claude CLAUDE.md envs ", "ENV_SECRET_KEY_LEDGER.C1",
     "PredicateFailure: envs/.env must never be delivered: the transactional source is the index of the named roots and envs/ is none of them", true),
    ("K03-correction-reads-env", ".claude/rust/role-contract/src/env_file_mode.rs", "pub fn observe(workspace: &Path) -> EnvFileState {\n",
     "pub fn observe(workspace: &Path) -> EnvFileState {\n    let _ = std::fs::read(workspace.join(ENV_FILE_REL));\n", "ENV_SECRET_KEY_LEDGER.C1",
     "PredicateFailure: the envs/.env correction must never open, write or print the file (fs::read)", true),
    ("K04-transition-judges-late", ".claude/skills/hint-publisher/rust/src/branch.rs", "    _require_branch_push_ssh(repo, remote)?;\n", "",
     "ENV_SECRET_KEY_LEDGER.C3", "PredicateFailure: the branch-transition must judge ssh-only and main-only before any remote query or push", true),
    ("K05-token-for-every-transport", ".claude/skills/hint-publisher/rust/src/tag.rs",
     "let token = if ts.iter().any(|t| t == \"https\") { read_push_token(repo, &J::Null)? } else { None };",
     "let token = read_push_token(repo, &J::Null)?;", "ENV_SECRET_KEY_LEDGER.C3",
     "PredicateFailure: the PAT (GITHUB_TOKEN) must be read only for an https transport (tag::_remote_env)", true),
    ("K06-branch-names-token", ".claude/skills/hint-publisher/rust/src/branch.rs", "pub fn _require_branch_push_main(repo: &Path) -> HR<()> {\n",
     "pub fn _require_branch_push_main(repo: &Path) -> HR<()> {\n    let _ = crate::tag::read_push_token(repo, &J::Null);\n",
     "ENV_SECRET_KEY_LEDGER.C3", "PredicateFailure: the hint branch code must never name the PAT path (read_push_token)", true),
];

const SYNC: &str = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";

fn lexer_checks() {
    let src = "/// doc \"relay.py\"\nfn a() { let x = [\"ssh\", r#\"raw\"#]; b(1); m!(c(2)); /* \"hidden\" */ 'x'; }\nfn d<'a>(remote: &'a str, n: Option<&str>) {}\n";
    let toks = h::lex(src);
    let strs: Vec<String> = toks.iter().filter(|t| t.k == h::Tk::Str).map(|t| t.s.clone()).collect();
    assert_eq!(strs, vec!["ssh".to_string(), "raw".to_string()], "comments never count; raw strings decode");
    let fns = h::functions(&toks);
    assert_eq!(fns.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["a", "d"]);
}

fn order_checks() {
    let guarded = "fn cmd_continue() { _approval();\n    finish();\n    }\nfn _approval() { evidence::approval_for();\n    }\nfn finish() { branch::commit_payload();\n    tag::seal();\n    }\nfn main() { cmd_continue();\n    }\n";
    assert!(h::approval_order_problems(&[guarded.to_string()]).is_empty(), "a guarded CLI is green");
    let moved = "fn cmd_continue() { finish();\n    _approval();\n    }\nfn _approval() { evidence::approval_for();\n    }\nfn finish() { branch::commit_payload();\n    tag::seal();\n    }\nfn main() { cmd_continue();\n    }\n";
    assert!(!h::approval_order_problems(&[moved.to_string()]).is_empty(), "approval below the side effect is RED");
    let deleted = "fn cmd_continue() { finish();\n    }\nfn finish() { branch::commit_payload();\n    tag::seal();\n    }\nfn main() { cmd_continue();\n    }\n";
    assert!(!h::approval_order_problems(&[deleted.to_string()]).is_empty(), "approval deleted is RED");
    let entry = "fn cmd_continue() { _approval();\n    finish();\n    }\nfn _approval() { evidence::approval_for();\n    }\nfn finish() { branch::commit_payload();\n    tag::seal();\n    }\nfn cmd_other() { tag::seal();\n    }\nfn main() { cmd_continue();\n    }\n";
    let p = h::approval_order_problems(&[entry.to_string()]);
    assert!(p.iter().any(|x| x.starts_with("cmd_other: an entry point")), "an ungated entry point is RED: {p:?}");
}

#[test]
fn hint_clauses() {
    lexer_checks();
    order_checks();
    std::env::set_var("POLICY_PREDICATES_REPO", repo());
    for c in CLAUSES {
        assert_eq!(run_clause(c), None, "{c} passes on the real tree");
    }
    for (id, file, old, new, clause, want, exact) in MUTATIONS {
        let root = scratch(id);
        tracked_copy(&root);
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(old), "{id}: mutation anchor missing");
        std::fs::write(&path, text.replace(old, new)).unwrap();
        std::env::set_var("POLICY_PREDICATES_REPO", &root);
        let got = run_clause(clause);
        std::env::set_var("POLICY_PREDICATES_REPO", repo());
        let _ = std::fs::remove_dir_all(&root);
        let got = got.unwrap_or_else(|| panic!("{id}: {clause} must fail"));
        if *exact {
            assert_eq!(&got, want, "{id}");
        } else {
            assert!(got.starts_with(want), "{id}: {got}");
        }
    }
}
