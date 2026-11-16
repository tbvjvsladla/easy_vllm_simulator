//! Clauses whose original drives `runtime_selftest.py` tripwires in-process: BRANCH_CONSTITUTION_LAYERING.C4/C5,
//! ROOT_SURFACE_REGISTRY.C1, GIT_SINGLE_AUTHORITY.C1/C2. The tripwires and their constants now live in the
//! `policy_selftest` library (`tripwires.rs`, the Rust owner of `runtime_selftest.py --tripwires-only`), and the
//! four-way parity predicate in `role_contract::topology`. Same fixtures, same RED->GREEN cycles, same messages;
//! only the call target moved. `_tripwire_raises` = the tripwire returned `Fail::Selftest` (the original's
//! `RuntimeSelftestFailure`); any other failure (`Fail::Crash`) propagates and fails the clause, as the original's
//! uncaught exception does.
use crate::claims_lib::{canonical_fixture_repo, fixture_git, layer_fixture};
use crate::support::*;
use agent_runtime_support::pyvalue::PyValue;
use policy_selftest::tripwires as tw;
use sha2::{Digest, Sha256};
use std::path::Path;

fn crash(f: tw::Fail) -> Fail {
    match f {
        tw::Fail::Selftest(m) => Fail::new("RuntimeSelftestFailure", m),
        tw::Fail::Crash(m) => Fail::new("Exception", m),
    }
}

/// `_tripwire_raises(fn, root)`: true when the production tripwire fails closed on `root`.
/// A tripwire of the `policy_selftest` library.
pub type Tw = fn(&Path) -> tw::R<()>;

fn tripwire_raises(f: fn(&Path) -> tw::R<()>, root: &Path) -> R<bool> {
    match f(root) {
        Ok(()) => Ok(false),
        Err(tw::Fail::Selftest(_)) => Ok(true),
        Err(other) => Err(crash(other)),
    }
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn remove_file(p: &Path) -> R {
    std::fs::remove_file(p).map_err(|e| Fail::new("OSError", format!("{}: {e}", p.display())))
}

fn remove_tree(p: &Path) -> R {
    std::fs::remove_dir_all(p).map_err(|e| Fail::new("OSError", format!("{}: {e}", p.display())))
}

fn sorted_repr(items: &[&str]) -> String {
    let mut v: Vec<&str> = items.to_vec();
    v.sort();
    repr_list(&v)
}

// =============================================================================== GIT_SINGLE_AUTHORITY

pub fn predicate_GIT_SINGLE_AUTHORITY_C1() -> R {
    let allowed: std::collections::BTreeSet<&str> = tw::ALLOWED_BRANCHES.iter().copied().collect();
    req(
        allowed == ["single-node", "multi-node", "hint"].into_iter().collect(),
        format!("refs/heads allowlist must be exactly the three operating branches: {}", sorted_repr(&tw::ALLOWED_BRANCHES)),
    )?;
    req(tw::ALLOWED_TAG_PREFIX == "hint/", format!("tags must be confined to the hint namespace, not {}", repr(tw::ALLOWED_TAG_PREFIX)))?;
    req(
        tw::BACKUP_SUFFIXES == [".bak", ".orig"],
        format!("the backup suffix set must stay (.bak, .orig): ('{}', '{}')", tw::BACKUP_SUFFIXES[0], tw::BACKUP_SUFFIXES[1]),
    )?;
    let tokens: std::collections::BTreeSet<&str> = tw::BACKUP_PATH_TOKENS.iter().copied().collect();
    req(
        tokens == ["backup", "백업"].into_iter().collect(),
        "the path-token set must cover this repo's actual Korean backup naming, not ASCII only",
    )?;

    git_single_authority_c1_cycle(tw::no_backup_artifacts)
}

/// The RED->GREEN cycle of GIT_SINGLE_AUTHORITY.C1 against tripwire (1) `f` (injectable: the checks drive mutants).
pub fn git_single_authority_c1_cycle(f: Tw) -> R {
    let tmp = TempDir::new("tmp")?;
    let root = canonical_fixture_repo(tmp.path())?;
    req(!tripwire_raises(f, &root)?, "a clean canonical fixture repo must pass tripwire (1) -- GREEN baseline")?;

    let copy = root.join("Dockerfile.bak");
    write(&copy, "x")?;
    req(tripwire_raises(f, &root)?, "a *.bak working-tree copy must fail tripwire (1)")?;
    remove_file(&copy)?;
    req(!tripwire_raises(f, &root)?, "deleting the .bak copy must return tripwire (1) to GREEN (the check is live, not sticky)")?;

    let korean = root.join("이전 plan 백업");
    mkdirs(&korean)?;
    write(&korean.join("a.md"), "x")?;
    req(
        tripwire_raises(f, &root)?,
        "a 백업-named directory must fail tripwire (1) -- an ASCII-only token had zero detection power here",
    )?;
    remove_tree(&korean)?;

    fixture_git(&root, &["branch", "wip-anchor"])?;
    req(tripwire_raises(f, &root)?, "a branch outside {single-node, multi-node, hint} must fail tripwire (1)")?;
    fixture_git(&root, &["branch", "-D", "wip-anchor"])?;
    req(!tripwire_raises(f, &root)?, "deleting the stray branch must return tripwire (1) to GREEN")?;

    fixture_git(&root, &["tag", "last-good-26090301"])?;
    req(
        tripwire_raises(f, &root)?,
        "a last-good-* tag must fail tripwire (1) -- the rollback anchor is a commit, never a tag",
    )?;
    fixture_git(&root, &["tag", "-d", "last-good-26090301"])?;

    write(&root.join(".gitignore"), "*.log\n*.bak\n*.orig\n")?;
    req(
        tripwire_raises(f, &root)?,
        "resurrecting the *.bak/*.orig ignore lines must fail tripwire (1) -- hiding is not forbidding",
    )?;
    write(&root.join(".gitignore"), "*.log\n")?;
    req(!tripwire_raises(f, &root)?, "restoring the ignore file must return tripwire (1) to GREEN")
}

pub fn predicate_GIT_SINGLE_AUTHORITY_C2() -> R {
    let a40 = "a".repeat(40);
    let b64 = "b".repeat(64);
    req(
        tw::hex_const_findall(&a40) == [a40.clone()] && tw::hex_const_findall(&b64) == [b64.clone()],
        "the derived predicate must recognise both git blob sha1 (40-hex) and sha256 (64-hex) constants",
    )?;

    git_single_authority_c2_cycle(tw::no_tracked_digest_rewrite)
}

/// The RED->GREEN cycle of GIT_SINGLE_AUTHORITY.C2 against tripwire (2) `f`.
pub fn git_single_authority_c2_cycle(f: Tw) -> R {
    let tmp = TempDir::new("tmp")?;
    let root = canonical_fixture_repo(tmp.path())?;
    req(!tripwire_raises(f, &root)?, "a fixture repo with no transcribed digest must pass tripwire (2) -- GREEN baseline")?;

    let blob_sha1 = fixture_git(&root, &["rev-parse", "HEAD:CLAUDE.md"])?;
    let claude = std::fs::read(root.join("CLAUDE.md")).map_err(|e| Fail::new("OSError", e.to_string()))?;
    let blob_sha256 = sha256_hex(&claude);
    let ledger = root.join("ledger.json");

    for (label, digest) in [("git blob sha1", &blob_sha1), ("sha256", &blob_sha256)] {
        write(&ledger, &format!("{{\"CLAUDE.md\": {}}}", json_str(digest)))?;
        fixture_git(&root, &["add", "-A"])?;
        req(tripwire_raises(f, &root)?, format!("a tracked ledger restating the {label} of a tracked blob must fail tripwire (2)"))?;
    }

    let outside = sha256_hex(b"upstream payload git does not carry");
    write(&ledger, &format!("{{\"upstream_payload\": {}}}", json_str(&outside)))?;
    fixture_git(&root, &["add", "-A"])?;
    req(
        !tripwire_raises(f, &root)?,
        "a digest of bytes git does not carry is outside the predicate by construction -- \
         the blind layer stays, and it is not maintained as an allowlist",
    )?;

    remove_file(&ledger)?;
    fixture_git(&root, &["add", "-A"])?;
    req(!tripwire_raises(f, &root)?, "removing the transcription must return tripwire (2) to GREEN")
}

// =============================================================================== ROOT_SURFACE_REGISTRY

pub fn predicate_ROOT_SURFACE_REGISTRY_C1() -> R {
    req(tw::ROOT_REGISTRY_REL == ".claude/policies/root_registry.json", format!("등록부 경로가 옮겨졌다: {}", tw::ROOT_REGISTRY_REL))?;
    let doc = tw::load_root_registry(&repo_root()).map_err(crash)?;
    let mut names: Vec<String> = jlist(jget(&doc, "entries")?).iter().filter_map(|e| jget(e, "name").ok().and_then(jstr)).collect();
    names.sort();
    names.dedup();
    req(
        ["campaigns", ".claude", "CLAUDE.md"].iter().all(|n| names.iter().any(|x| x == n)),
        format!("등록부가 비정상: {}", repr_list(&names[..names.len().min(8)])),
    )?;
    let tombstones = jget(&doc, "tombstones").map(jlist).unwrap_or_default();
    req(
        tombstones.iter().any(|t| jget(t, "name").ok().and_then(jstr).as_deref() == Some("tasks")),
        "폐지된 `tasks/` 가 tombstone 으로 남아 있어야 한다",
    )?;

    root_surface_registry_c1_cycle(tw::root_surface_registry)
}

/// The RED->GREEN cycle of ROOT_SURFACE_REGISTRY.C1 against tripwire ⑦ `f`.
pub fn root_surface_registry_c1_cycle(f: Tw) -> R {
    let tmp = TempDir::new("tmp")?;
    let root = canonical_fixture_repo(tmp.path())?;
    let reg = root.join(".claude/policies/root_registry.json");
    let entries = r#"{"name": "CLAUDE.md", "kind": "file", "git": "tracked"}, {"name": ".gitignore", "kind": "file", "git": "tracked"}, {"name": ".claude", "kind": "dir", "git": "tracked"}"#;
    let doc_with = |extra: &str| {
        format!(
            r#"{{"schema_version": 1, "policy_id": "ROOT_SURFACE_REGISTRY", "entries": [{entries}{extra}], "tombstones": [{{"name": "config.camp1-*.yaml", "kind": "glob", "retired_utc": "2026-09-06", "successor": "campaigns/"}}]}}"#
        )
    };
    let write_reg = |text: &str| -> R {
        write(&reg, text)?;
        fixture_git(&root, &["add", "-A"])?;
        Ok(())
    };
    let base = doc_with("");

    write_reg(&base)?;
    req(!tripwire_raises(f, &root)?, "등록부와 인덱스가 일치하면 tripwire ⑦ 는 GREEN 이어야 한다")?;

    let stray = root.join("stray.md");
    write(&stray, "x")?;
    fixture_git(&root, &["add", "-A"])?;
    req(tripwire_raises(f, &root)?, "미등재 루트 추적물은 tripwire ⑦ 를 RED 로 만들어야 한다")?;
    fixture_git(&root, &["rm", "-q", "--cached", "stray.md"])?;
    remove_file(&stray)?;
    req(!tripwire_raises(f, &root)?, "추적 해제하면 GREEN 으로 돌아와야 한다(끈적이지 않는다)")?;

    let leaked = root.join("config.camp1-20b-0180.yaml");
    write(&leaked, "nas: x\n")?;
    fixture_git(&root, &["add", "-A"])?;
    req(tripwire_raises(f, &root)?, "폐지된 거처가 되살아나면 RED 여야 한다 — 이것이 21개를 흘린 이름 모양이다")?;
    fixture_git(&root, &["rm", "-q", "--cached", "config.camp1-20b-0180.yaml"])?;
    remove_file(&leaked)?;

    write_reg(&doc_with(r#", {"name": "campaigns", "kind": "dir", "git": "tracked"}"#))?;
    req(tripwire_raises(f, &root)?, "등록부가 tracked 라고 선언한 항목이 인덱스에 없으면 RED 여야 한다(무력화 검출)")?;
    write_reg(&base)?;
    req(!tripwire_raises(f, &root)?, "복원하면 GREEN")?;

    remove_file(&reg)?;
    fixture_git(&root, &["add", "-A"])?;
    req(
        tripwire_raises(f, &root)?,
        "등록부 자체가 사라지면 통과가 아니라 RED 다 — 무력한 가드를 통과한 가드로 보이게 하지 않는다",
    )
}

// =============================================================================== BRANCH_CONSTITUTION_LAYERING

pub fn predicate_BRANCH_CONSTITUTION_LAYERING_C4() -> R {
    let vocab = tw::TOPOLOGY_ONLY_VOCABULARY;
    let mut axes: Vec<&str> = vocab.iter().map(|(k, _)| *k).collect();
    axes.sort();
    req(axes == ["multi", "single"], format!("어휘 목록의 축이 바뀌었다: {}", repr_list(&axes)))?;
    req(vocab.iter().all(|(_, terms)| !terms.is_empty()), "한쪽 어휘 목록이 비었다")?;
    for (rel, term, frag, reason) in tw::TOPOLOGY_VOCABULARY_EXEMPTIONS {
        req(
            reason.chars().count() > 20,
            format!("면제에 사유가 없다 -- 사유 없는 면제는 조용한 구멍이다: ({}, {}, {})", repr(rel), repr(term), repr(frag)),
        )?;
    }
    let term = vocab.iter().find(|(k, _)| *k == "multi").map(|(_, t)| t[0]).unwrap_or_default();
    layering_c4_cycle(tw::vocabulary_offenders, term)
}

/// The fire/no-fire cycle of BRANCH_CONSTITUTION_LAYERING.C4 against the common-layer scanner `offenders`.
pub fn layering_c4_cycle(offenders: fn(&Path) -> Vec<String>, term: &str) -> R {
    let tmp = TempDir::new("tmp")?;
    let root = tmp.path();
    mkdirs(&root.join(".claude/rules"))?;
    write(&root.join("CLAUDE.md"), &format!("# c\n서브는 {term} 다.\n"))?;
    req(!offenders(root).is_empty(), "공통층의 토폴로지 전용 어휘가 발화하지 않았다")?;
    write(&root.join("CLAUDE.md"), "# c\n서브의 정체는 토폴로지가 정한다.\n")?;
    req(offenders(root).is_empty(), "음성대조 실패 -- 어휘가 없는데 발화했다")?;
    write(&root.join(".claude/rules/strategy.topology.md"), &format!("**topology: multi**\n서브는 {term} 다.\n"))?;
    req(offenders(root).is_empty(), "특화층이 공통층 스캔에 들어왔다 -- 거기 있는 것이 정상이다")
}

fn field<'a>(v: &'a PyValue, path: &[&str]) -> R<&'a PyValue> {
    let mut cur = v;
    for k in path {
        cur = jget(cur, k)?;
    }
    Ok(cur)
}

/// `topology_parity.evaluate(root)` (the owner library `role_contract::topology`).
pub fn parity_evaluate(root: &Path) -> R<PyValue> {
    Ok(tw::parity_evaluate(root).map_err(crash)?.3)
}

fn codes(res: &PyValue) -> R<Vec<String>> {
    Ok(jstrs(jget(res, "reason_codes")?))
}

pub fn predicate_BRANCH_CONSTITUTION_LAYERING_C5() -> R {
    layering_c5_cycle(parity_evaluate, tw::topology_layer_parity)?;
    let tmp = TempDir::new("tmp")?;
    let root = canonical_fixture_repo(tmp.path())?; // 추적 특화 파일 0개
    let res = parity_evaluate(&root)?;
    let c = codes(&res)?;
    req(c.iter().any(|x| x == "LAYER_ABSENT"), format!("추적 입력의 부재는 위반이어야 한다: {}", repr_list(&c)))
}

/// The match / mismatch fixtures of BRANCH_CONSTITUTION_LAYERING.C5 against the parity `evaluate` and its tripwire `f`.
pub fn layering_c5_cycle(evaluate: fn(&Path) -> R<PyValue>, f: Tw) -> R {
    {
        let tmp = TempDir::new("tmp")?;
        let root = layer_fixture(tmp.path(), "single", "single-node")?;
        let res = evaluate(&root)?;
        req(
            jstr(jget(&res, "verdict")?).as_deref() == Some("PASS"),
            format!("일치 상태가 PASS 가 아니다: {}", repr_list(&jstrs(jget(&res, "reasons")?))),
        )?;
        req(
            jstr(field(&res, &["legs", "manifest", "status"])?).as_deref() == Some("absent")
                && jstr(field(&res, &["legs", "campaign", "status"])?).as_deref() == Some("absent"),
            "비추적 다리의 부재는 absent 여야 한다(위반이 아니다)",
        )?;
        let checked = jget(&res, "legs_checked")?;
        req(*checked == PyValue::int(2), format!("2자 판정을 4자처럼 보고하면 안 된다: {}", checked.repr()))?;
    }
    {
        let tmp = TempDir::new("tmp")?;
        let root = layer_fixture(tmp.path(), "multi", "single-node")?; // 브랜치 single-node ↔ 헤더 multi
        let res = evaluate(&root)?;
        let c = codes(&res)?;
        req(c.iter().any(|x| x == "LAYER_HEADER_MISMATCH"), format!("2026-09-11 사고의 형태가 발화하지 않았다: {}", repr_list(&c)))?;
        let remediation = jstr(jget(&res, "remediation")?).unwrap_or_default();
        req(jstr(jget(&res, "verdict")?).as_deref() == Some("RED") && !remediation.is_empty(), "불일치에 해소 안내가 없다")?;
        req(remediation.contains("자동 교정하지 않는다"), "자동 교정 금지가 출력에서 사라졌다")?;
        req(tripwire_raises(f, &root)?, "tripwire 가 불일치에서 fail-closed 하지 않았다")?;
        let spec = root.join(".claude/rules/strategy.topology.md");
        let before = read_path(&spec)?;
        evaluate(&root)?;
        req(read_path(&spec)? == before, "술어가 파일을 고쳤다 -- 이 술어는 읽기만 해야 한다")?;
    }
    Ok(())
}
