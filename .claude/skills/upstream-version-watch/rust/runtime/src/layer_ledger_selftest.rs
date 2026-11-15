//! `layer_ledger.py --self-test` in Rust: the same positive/negative controls on the same git fixtures (real git, real worktrees), no Python.

use super::*;

type R = Result<(), String>;

fn ck(name: &str, cond: bool, got: &PyValue) -> R {
    if cond {
        Ok(())
    } else {
        Err(format!("[layer_ledger] self-test FAIL: {name} (got={})", dumps(got)))
    }
}

fn ckb(name: &str, cond: bool) -> R {
    ck(name, cond, &s(""))
}

fn ex(e: Exc) -> String {
    format!("{}: {}", e.kind, e.msg)
}

fn st(v: &PyValue, k: &str) -> String {
    dstr(v, k).unwrap_or_default()
}

fn is_bool(v: &PyValue, k: &str, want: bool) -> bool {
    matches!(dget(v, k).map(|x| x.peel()), Some(PyValue::Bool(b)) if *b == want)
}

fn strs(v: &PyValue, k: &str) -> Vec<String> {
    match dget(v, k).map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.iter().map(text_of).collect(),
        _ => vec![],
    }
}

fn is_none(v: &PyValue, k: &str) -> bool {
    matches!(dget(v, k).map(|x| x.peel()), Some(PyValue::None))
}

fn ent(items: Vec<(&str, PyValue)>) -> PyValue {
    obj(items)
}

fn with(base: &PyValue, key: &str, val: PyValue) -> PyValue {
    let mut b = base.clone();
    set_key(&mut b, key, val);
    b
}

fn good_entry() -> PyValue {
    ent(vec![
        ("entry_id", s("e1")),
        ("utc", s("2026-09-12T00:00:00Z")),
        ("departure", s("multi-node")),
        ("departure_commit", s("deadbee")),
        ("approved_by", s("maintainer")),
        ("approved_utc", s("2026-09-12T00:01:00Z")),
        ("classification", PyValue::List(vec![row("CLAUDE.md", "common_promote", "양 토폴로지가 읽는다", "agent-judged")])),
    ])
}

fn row(file: &str, verdict: &str, reason: &str, prov: &str) -> PyValue {
    ent(vec![("file", s(file)), ("verdict", s(verdict)), ("reason", s(reason)), ("provenance", s(prov))])
}

fn with_row(base: &PyValue, row_key: &str, val: &str) -> PyValue {
    let mut r = match dget(base, "classification").map(|x| x.peel()) {
        Some(PyValue::List(l)) => l[0].clone(),
        _ => PyValue::None,
    };
    set_key(&mut r, row_key, s(val));
    with(base, "classification", PyValue::List(vec![r]))
}

struct Fx {
    td: PathBuf,
}

impl Fx {
    fn g(&self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        let o = git(cwd, args);
        if o.code != 0 {
            return Err(format!("fixture git {args:?} failed: {}", o.err_text().chars().take(300).collect::<String>()));
        }
        Ok(o.text().trim().to_string())
    }
    fn write(&self, where_: &Path, rel: &str, text: &str) -> R {
        let p = where_.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, text).map_err(|e| e.to_string())
    }
    fn commit(&self, where_: &Path, msg: &str) -> Result<String, String> {
        self.g(where_, &["add", "-A"])?;
        self.g(where_, &["commit", "-qm", msg])?;
        self.g(where_, &["rev-parse", "HEAD"])
    }
}

fn real_sync_script() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for a in exe.ancestors() {
        // split literal: the retirement consumer audit must not see the retired top-level path as a consumer
        let p = a.join("scripts").join(concat!("sync_branches", ".sh"));
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for a in cwd.ancestors() {
            let p = a.join(SYNC_SCRIPT_REL);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("sync_branches.sh not found next to the executable".into())
}

pub fn self_test() -> i32 {
    let td = std::env::temp_dir().join(format!("ll_selftest_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let res = std::fs::create_dir_all(&td).map_err(|e| e.to_string()).and_then(|_| body(&td));
    let _ = std::fs::remove_dir_all(&td);
    match res {
        Ok(()) => {
            eprintln!("[layer_ledger] self-test PASS");
            0
        }
        Err(m) => {
            eprintln!("{m}");
            1
        }
    }
}

fn body(td: &Path) -> R {
    let good = good_entry();
    let vp = validate_entry(&good);
    ck("양성: 완전한 항목", vp.is_empty(), &str_list(&vp))?;
    let negs: Vec<(PyValue, &str)> = vec![
        (with_row(&good, "reason", ""), "근거 없음"),
        (with_row(&good, "verdict", "maybe"), "판정 값역"),
        (with_row(&good, "provenance", "human"), "provenance"),
        (with(&good, "approved_by", s("")), "승인자 없음"),
    ];
    for (m, why) in &negs {
        ckb(&format!("음성: {why}"), !validate_entry(m).is_empty())?;
    }
    let fm = |t: &str| format!("{FILL_MARK}{t}");
    for key in ["entry_id", "utc", "departure_commit", "approved_by", "approved_utc", "departure"] {
        let bad = with(&good, key, s(&fm(": 채워라>")));
        let probs = validate_entry(&bad);
        ck(&format!("★S4 음성: {key} 에 자리표시자가 남으면 거부"), probs.iter().any(|p| p.contains(key) && p.contains(FILL_MARK)), &str_list(&probs))?;
    }
    let bad_row = with_row(&good, "reason", &fm(">"));
    ckb("★S4 음성: 분류 행 근거 자리표시자 거부", validate_entry(&bad_row).iter().any(|p| p.contains("classification[0].reason")))?;
    let skeleton = ent(vec![
        ("entry_id", s(&fm(": 예 sync-YYYYMMDDHHMM>"))),
        ("utc", s(&fm(": 시각>"))),
        ("departure", s("multi-node")),
        ("departure_commit", s(&fm(": HEAD>"))),
        ("approved_by", s(&fm(": 사람>"))),
        ("approved_utc", s(&fm(">"))),
        ("classification", PyValue::List(vec![row("CLAUDE.md", "<common_promote|branch_only>", &fm(">"), "agent-judged")])),
    ]);
    let quoting = with_row(&good, "reason", "provenance 기본값을 <<FILL>> 빈칸으로 바꿔 precheck 가 막는다");
    let qp = validate_entry(&quoting);
    ck("★S4 양성: 캠페인 뼈대 자리표시자 <<FILL>> 을 **인용한** 근거는 채워진 칸이다(승인 뒤 표를 고치게 하지 않는다)", qp.is_empty(), &str_list(&qp))?;
    let partial = with_row(&good, "reason", &format!("근거 일부만 적었다 {}>", FILL_MARK));
    ckb("★S4 음성: 산문 뒤에 남은 자리표시자도 거부", validate_entry(&partial).iter().any(|p| p.contains("classification[0].reason")))?;
    let sk = validate_entry(&skeleton);
    ck("★S4 음성: 빈칸 그대로의 분류표는 자리표시자 6건 + 판정 값역을 전부 드러낸다", sk.iter().filter(|p| p.contains(FILL_MARK)).count() == 6 && sk.iter().any(|p| p.contains("판정은")), &str_list(&sk))?;

    // ledger file behaviour
    {
        let root = td.join("ledger-root");
        std::fs::create_dir_all(root.join(".claude/policies")).map_err(|e| e.to_string())?;
        let r = append(&root, &good).map_err(ex)?;
        ck("첫 append", st(&r, "status") == "appended", &r)?;
        let r = append(&root, &good).map_err(ex)?;
        ck("동일 재append = 멱등", st(&r, "status") == "already-recorded", &r)?;
        let drifted = with(&good, "departure_commit", s("cafef00"));
        let r = append(&root, &drifted).map_err(ex)?;
        ck("내용 다른 중복 = 거부", st(&r, "status") == "rejected", &r)?;
        let n_entries = |root: &Path| entries_of(&load(root).unwrap_or(PyValue::None)).len();
        ckb("적재", n_entries(&root) == 1)?;
        let r = append(&root, &skeleton).map_err(ex)?;
        ckb("★S4 음성: 자리표시자 항목은 원장에 들어가지 않는다", st(&r, "status") == "rejected" && n_entries(&root) == 1)?;
        let broken = b"{\"entries\": [ {\"entry_id\": \"old\"";
        std::fs::write(root.join(LEDGER_REL), broken).map_err(|e| e.to_string())?;
        let r = append(&root, &with(&good, "entry_id", s("e2"))).map_err(ex)?;
        ck("★음성: 못 읽는 원장 위 append 는 거부하고 바이트를 건드리지 않는다", st(&r, "status") == "rejected" && std::fs::read(root.join(LEDGER_REL)).map_err(|e| e.to_string())? == broken, &r)?;
    }

    // common layer spec from sync_branches.sh
    let real_sync = real_sync_script()?;
    {
        let root = td.join("spec-root");
        let dst = root.join(SYNC_SCRIPT_REL);
        std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::copy(&real_sync, &dst).map_err(|e| e.to_string())?;
        let spec = common_layer(&root).map_err(ex)?;
        ck("★S3 docs 판정 대상 = 복사∩미러(docs/*/example.md)", strs(&spec, "include_docs") == ["docs/*/example.md"], &spec)?;
        let ps = strs(&spec, "pathspec");
        ck("★S3 합집합 수렴(report·인증서)은 판정에서 뺀다", strs(&spec, "union_docs").iter().any(|x| x == "docs/report/*") && ps.iter().any(|x| x == ":(exclude)docs/report/*"), &spec)?;
        ck("★S3 pathspec 에 docs/*/example.md 가 실린다", ps.iter().any(|x| x == "docs/*/example.md"), &spec)?;
        let copy_only = strs(&spec, "copy_only_docs");
        let reach = strs(&spec, "reach_pathspec");
        ck(
            "★S3 도달범위(분류표 빈칸)는 충돌 검사 없이 덮는 복사 전용 arm(인증서)을 보인다 · report 는 뺀다",
            !copy_only.is_empty() && copy_only.iter().all(|p| reach.contains(p)) && reach.iter().any(|x| x == ":(exclude)docs/report/*") && !copy_only.iter().any(|p| ps.contains(p)),
            &spec,
        )?;
        std::fs::write(&dst, "ALLOWLIST=(\n    CLAUDE.md\n)\n").map_err(|e| e.to_string())?;
        ckb("★음성: arm 을 못 읽으면 빈 목록으로 접지 않고 소리낸다", common_layer(&root).is_err())?;
    }

    // git fixtures
    let gitconfig = td.join("gitconfig");
    std::fs::write(&gitconfig, "[commit]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n").map_err(|e| e.to_string())?;
    for k in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX"] {
        std::env::remove_var(k);
    }
    std::env::set_var("GIT_CONFIG_GLOBAL", &gitconfig);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    for (k, v) in [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")] {
        std::env::set_var(k, v);
    }
    fixtures(td, &real_sync)
}

fn fixtures(td: &Path, real_sync: &Path) -> R {
    let fx = Fx { td: td.to_path_buf() };
    let repo = fx.td.join("repo");
    std::fs::create_dir_all(&repo).map_err(|e| e.to_string())?;
    let g = |cwd: &Path, a: &[&str]| fx.g(cwd, a);
    g(&repo, &["init", "-q", "-b", "multi-node"])?;
    let dst = repo.join(SYNC_SCRIPT_REL);
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::copy(real_sync, &dst).map_err(|e| e.to_string())?;
    fx.write(&repo, "CLAUDE.md", "v1\n")?;
    fx.write(&repo, ".claude/rules/common.md", "c1\n")?;
    fx.write(&repo, ".claude/rules/strategy.topology.md", "**topology: multi**\n")?;
    fx.write(&repo, "docs/plan/example.md", "p1\n")?;
    fx.write(&repo, "docs/report/r1.md", "r1\n")?;
    fx.commit(&repo, "A")?;
    g(&repo, &["branch", "single-node"])?;
    let wt = fx.td.join("wt-single");
    g(&repo, &["worktree", "add", "-q", &wt.to_string_lossy(), "single-node"])?;
    fx.write(&wt, ".claude/rules/strategy.topology.md", "**topology: single**\n")?;
    fx.commit(&wt, "S0 특화층")?;

    let good = good_entry();
    let mref = || vec!["multi-node".to_string()];
    let ent_of = |id: &str, utc: &str, commit: &str| with(&with(&with(&good, "entry_id", s(id)), "utc", s(utc)), "departure_commit", s(commit));

    fx.write(&repo, "CLAUDE.md", "v2\n")?;
    let b = fx.commit(&repo, "B")?;
    let r = append(&repo, &ent_of("sync-1", "2026-09-12T06:16:53Z", &b)).map_err(ex)?;
    ck("원장 E1 append", st(&r, "status") == "appended", &r)?;
    fx.commit(&repo, "ledger(sync-1)")?;
    fx.write(&repo, ".claude/rules/common.md", "c2\n")?;
    let b2 = fx.commit(&repo, "B2 교정")?;
    g(&wt, &["checkout", "multi-node", "--", "CLAUDE.md", ".claude/rules/common.md"])?;
    let landing = fx.commit(&wt, "sync: 공통층 multi-node→single-node")?;

    let spec = common_layer_data(&repo).map_err(ex)?.pathspec;
    let mut dargs = vec!["-C".to_string(), repo.to_string_lossy().into_owned(), "diff".into(), "--quiet".into(), b.clone(), landing.clone(), "--".into()];
    dargs.extend(spec.iter().cloned());
    let differs = Command::new("git").args(&dargs).status().map(|s| s.code() != Some(0)).unwrap_or(false);
    ckb("픽스처가 사고의 형태다: 기록된 출발 커밋 B 와 착지 트리가 다르다", differs)?;

    let sp_ = |p: &Path, br: &str, cp: Option<&str>, refs: Vec<String>| sync_point(p, br, cp, &refs).map_err(ex);
    let dv_ = |p: &Path, br: &str, cp: Option<&str>, refs: Vec<String>| divergence(p, br, None, cp, &refs).map_err(ex);

    let no_ref = dv_(&wt, "single-node", None, vec![])?;
    ck("★S1 음성대조: 대상 워크트리 원장만 보면 no-sync-point(empty-ledger)", st(&no_ref, "status") == "no-sync-point" && dget(&no_ref, "sync_point").map(|x| st(x, "status")) == Some("empty-ledger".into()), &no_ref)?;
    let sp = sp_(&wt, "single-node", None, mref())?;
    ck(
        "★S2 착지 = 트리 동치(재실행 회차) · converged · 착지한 출발 커밋 = B2 · drift 표시",
        st(&sp, "status") == "converged" && st(&sp, "commit") == landing && st(&sp, "landed_departure_commit") == b2 && is_bool(&sp, "departure_drift", true) && st(&sp, "recorded_departure_commit") == b,
        &sp,
    )?;
    let dv = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★S1 ref 원장으로 실제 판정: clean · basis=ledger-sync-point", st(&dv, "status") == "clean" && st(&dv, "basis") == "ledger-sync-point" && st(&dv, "base") == landing, &dv)?;
    let dep = sp_(&repo, "multi-node", Some("single-node"), vec![])?;
    ck("★S2 출발 측 동기 지점 = 착지한 출발 커밋(B2 · 기록 B 아님)", st(&dep, "status") == "departure-landed" && st(&dep, "commit") == b2, &dep)?;
    let legacy = sp_(&repo, "multi-node", None, vec![])?;
    ck("상대 없이 출발 측을 물으면 기록 커밋 · landing_verified=false 로 표시", st(&legacy, "status") == "departure" && st(&legacy, "commit") == b && is_bool(&legacy, "landing_verified", false), &legacy)?;

    fx.write(&repo, "CLAUDE.md", "v3\n")?;
    let c = fx.commit(&repo, "C")?;
    let r = append(&repo, &ent_of("sync-2", "2026-09-14T00:00:00Z", &c)).map_err(ex)?;
    ck("원장 E2 append", st(&r, "status") == "appended", &r)?;
    fx.commit(&repo, "ledger(sync-2)")?;
    let sp2 = sp_(&wt, "single-node", Some("multi-node"), mref())?;
    let skipped_has = matches!(dget(&sp2, "skipped").map(|x| x.peel()), Some(PyValue::List(l)) if l.iter().any(|x| st(x, "entry_id") == "sync-2"));
    ck("★S2 새것부터 거꾸로: 착지 전인 이번 항목(E2)을 건너뛰고 E1 착지를 찾는다", st(&sp2, "status") == "converged" && st(&sp2, "entry_id") == "sync-1" && st(&sp2, "commit") == landing && skipped_has, &sp2)?;

    fx.write(&wt, "docs/report/r2.md", "single 전용 발행본\n")?;
    fx.commit(&wt, "report(single)")?;
    let dv_r = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★S3 음성대조: 대상 전용 report 는 갈라짐이 아니다(합집합)", st(&dv_r, "status") == "clean", &dv_r)?;
    fx.write(&wt, "docs/plan/example.md", "single 전용 스켈레톤 수정\n")?;
    fx.commit(&wt, "docs skeleton(single)")?;
    let dv_d = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★S3 대상 전용 docs/*/example.md 변경이 갈라짐으로 보인다", st(&dv_d, "status") == "diverged" && strs(&dv_d, "files") == ["docs/plan/example.md"], &dv_d)?;
    g(&wt, &["revert", "--no-edit", "HEAD"])?;
    fx.write(&wt, ".claude/rules/common.md", "single 전용\n")?;
    fx.commit(&wt, "target-only(single)")?;
    let dv_x = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck(
        "★S1 대상 전용 공통층 변경 → diverged (동기 지점 이후 · 파일 지목)",
        st(&dv_x, "status") == "diverged" && dget(&dv_x, "sync_point").map(|x| st(x, "status")) == Some("converged".into()) && strs(&dv_x, "files") == [".claude/rules/common.md"],
        &dv_x,
    )?;

    fx.write(&repo, "CLAUDE.md", "v4\n")?;
    fx.commit(&repo, "D 출발 전용")?;
    fx.write(&repo, ".claude/rules/common.md", "single 전용\n")?;
    fx.commit(&repo, "cherry-pick(single 전용 → multi)")?;
    let absorbed = |v: &PyValue| match dget(v, "absorbed").map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.iter().map(|a| st(a, "file")).collect::<Vec<_>>(),
        _ => vec![],
    };
    let dv_c = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★교착 해소: 대상 전용 변경을 출발 브랜치로 옮기면 clean · 흡수(absorbed)로 드러난다", st(&dv_c, "status") == "clean" && absorbed(&dv_c) == [".claude/rules/common.md"], &dv_c)?;
    fx.write(&repo, ".claude/rules/common.md", "multi 가 옮긴 뒤 더 고쳤다\n")?;
    fx.commit(&repo, "E 옮긴 뒤 출발 수정")?;
    let dv_c2 = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★흡수: 옮겨진 뒤 출발 브랜치가 더 고쳐도 clean(대상 바이트가 동기 지점 이후 상대 계보에 있다)", st(&dv_c2, "status") == "clean" && absorbed(&dv_c2) == [".claude/rules/common.md"], &dv_c2)?;
    let nocp = dv_(&wt, "single-node", None, mref())?;
    ck("상대 없이 물으면 흡수를 셀 수 없다 -- 동기 지점 이후 대상 변경 전부가 diverged(종전 규칙)", st(&nocp, "status") == "diverged" && strs(&nocp, "files").iter().any(|f| f == ".claude/rules/common.md"), &nocp)?;

    let wr = fx.td.join("wt-revert");
    g(&repo, &["worktree", "add", "-q", "-b", "revert-probe", &wr.to_string_lossy(), &landing])?;
    fx.write(&wr, ".claude/rules/common.md", "c1\n")?;
    fx.commit(&wr, "대상 되돌림(동기 지점 이전 바이트)")?;
    let rv = dv_(&wr, "revert-probe", Some("multi-node"), mref())?;
    ck(
        "★되돌림: 옛 출발 트리와 같아진 대상 커밋을 착지로 뽑지 않는다(착지 = landing) · diverged",
        st(&rv, "status") == "diverged" && st(&rv, "base") == landing && strs(&rv, "files") == [".claude/rules/common.md"] && dget(&rv, "sync_point").map(|x| st(x, "landed_departure_commit")) == Some(b2.clone()),
        &rv,
    )?;

    // ledger integrity
    let ledger_text = std::fs::read_to_string(repo.join(LEDGER_REL)).map_err(|e| e.to_string())?;
    let mut forged = pyjson::loads(&ledger_text).map_err(|e| e.to_string())?;
    if let Some(PyValue::List(l)) = match &mut forged {
        PyValue::Dict(d) => d.get_mut("entries"),
        _ => None,
    } {
        set_key(&mut l[0], "approved_by", s("someone-else"));
    }
    fx.write(&wt, LEDGER_REL, &dumps(&forged))?;
    let conf = sp_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★음성: 출처마다 다른 같은 entry_id → ledger-conflict", st(&conf, "status") == "ledger-conflict", &conf)?;
    let dconf = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★음성: 원장 충돌이면 갈라짐 판정은 unjudged(basis=ledger-integrity) -- blob 조상으로 접지 않는다", st(&dconf, "status") == "unjudged" && st(&dconf, "basis") == "ledger-integrity" && is_none(&dconf, "diverged"), &dconf)?;
    std::fs::remove_file(wt.join(LEDGER_REL)).map_err(|e| e.to_string())?;
    let nosuch = || vec!["no-such-ref".to_string()];
    let unres = sp_(&wt, "single-node", Some("multi-node"), nosuch())?;
    ck("★음성: 풀리지 않는 ledger-ref → ledger-unreadable(빈 원장으로 접지 않는다)", st(&unres, "status") == "ledger-unreadable", &unres)?;
    let dunres = dv_(&wt, "single-node", Some("multi-node"), nosuch())?;
    ck("★음성: 못 읽는 원장이면 갈라짐 판정은 unjudged(basis=ledger-integrity)", st(&dunres, "status") == "unjudged" && st(&dunres, "basis") == "ledger-integrity", &dunres)?;
    fx.write(&wt, LEDGER_REL, "{\"entries\": [ {\"entry_id\"")?;
    let dbroken = dv_(&wt, "single-node", Some("multi-node"), mref())?;
    ck("★음성: 깨진 JSON 원장이면 갈라짐 판정은 unjudged", st(&dbroken, "status") == "unjudged", &dbroken)?;
    std::fs::remove_file(wt.join(LEDGER_REL)).map_err(|e| e.to_string())?;

    // blob ancestry
    let wb = fx.td.join("wt-blob");
    g(&repo, &["worktree", "add", "-q", "-b", "blob-probe", &wb.to_string_lossy(), &b])?;
    let ghost = ent_of("sync-ghost", "2026-09-14T01:00:00Z", &c);
    let doc = obj(vec![("schema_version", PyValue::int(1)), ("entries", PyValue::List(vec![ghost]))]);
    fx.write(&wb, LEDGER_REL, &dumps(&doc))?;
    let nb = dv_(&wb, "blob-probe", Some("multi-node"), vec![])?;
    ck("★blob 조상: 착지 없음 → basis=blob-ancestry · 대상 바이트가 출발 이력에 있으면 clean", st(&nb, "basis") == "blob-ancestry" && st(&nb, "status") == "clean" && dget(&nb, "sync_point").map(|x| st(x, "status")) == Some("unresolved".into()), &nb)?;
    fx.write(&wb, ".claude/rules/common.md", "어디에도 없던 바이트\n")?;
    fx.commit(&wb, "blob-only")?;
    let nb2 = dv_(&wb, "blob-probe", Some("multi-node"), vec![])?;
    ck("★blob 조상 음성대조: 출발 이력에 한 번도 없던 바이트 → diverged", st(&nb2, "basis") == "blob-ancestry" && st(&nb2, "status") == "diverged" && strs(&nb2, "files") == [".claude/rules/common.md"], &nb2)?;
    let nb3 = dv_(&wb, "blob-probe", None, vec![])?;
    ck("상대가 없으면 blob 조상도 못 한다 -- no-sync-point 로 드러낸다", st(&nb3, "status") == "no-sync-point", &nb3)?;

    let wd = fx.td.join("wt-del");
    g(&repo, &["worktree", "add", "-q", "-b", "del-probe", &wd.to_string_lossy(), &b])?;
    g(&wd, &["rm", "-q", ".claude/rules/common.md"])?;
    fx.commit(&wd, "대상 전용 삭제")?;
    fx.write(&repo, ".claude/rules/new.md", "상대만 새로 만든 파일\n")?;
    fx.commit(&repo, "상대 추가")?;
    let nd = dv_(&wd, "del-probe", Some("multi-node"), vec![])?;
    ck("★blob 조상: 대상 전용 삭제 → diverged · 상대만 추가한 파일(대상이 가진 적 없음)은 갈라짐이 아니다", st(&nd, "basis") == "blob-ancestry" && st(&nd, "status") == "diverged" && strs(&nd, "files") == [".claude/rules/common.md"], &nd)?;

    let wv = fx.td.join("wt-rev");
    g(&repo, &["worktree", "add", "-q", "-b", "rev-probe", &wv.to_string_lossy(), &b])?;
    fx.write(&wv, "CLAUDE.md", "v1\n")?;
    fx.commit(&wv, "대상 되돌림(옛 상대 바이트)")?;
    let nv = dv_(&wv, "rev-probe", Some("multi-node"), vec![])?;
    let why = dget(&nv, "file_reasons").and_then(|r| dget(r, "CLAUDE.md")).map(text_of).unwrap_or_default();
    ck("★blob 조상: 상대 이력의 더 새 판본(v2)을 든 뒤 옛 판본(v1)으로 되돌림 → diverged(file_reasons 표시)", st(&nv, "status") == "diverged" && strs(&nv, "files") == ["CLAUDE.md"] && why.contains("되돌림"), &nv)?;

    let wf = fx.td.join("wt-fwd");
    let first = g(&repo, &["rev-list", "--max-parents=0", "multi-node"])?;
    g(&repo, &["worktree", "add", "-q", "-b", "fwd-probe", &wf.to_string_lossy(), &first])?;
    fx.write(&wf, "CLAUDE.md", "v2\n")?;
    fx.commit(&wf, "대상이 상대의 옛 판본을 뒤늦게 받음")?;
    let nf = dv_(&wf, "fwd-probe", Some("multi-node"), vec![])?;
    ck("★blob 조상 음성대조: 더 새 판본을 든 적 없이 상대의 옛 판본으로 앞으로 옮긴 것은 갈라짐이 아니다", st(&nf, "status") == "clean" && st(&nf, "basis") == "blob-ancestry", &nf)?;
    Ok(())
}
