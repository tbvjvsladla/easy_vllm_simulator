//! `judge_version_delta.py --self-test` ported to Rust: same sections, same `  ok   name` / `  FAIL name` lines, same closing `self-test:` line.
//! The D0 section builds a real synthetic git repository (git subprocess), as the original did.
use super::*;
use std::cell::RefCell;

fn jv(text: &str) -> PyValue {
    agent_runtime_support::pyjson::loads_text(&PyStr::new(text)).expect("fixture json")
}

fn gs(v: &PyValue, key: &str) -> String {
    dget(v, key).and_then(sval).unwrap_or_default()
}

fn glen(v: &PyValue, key: &str) -> Option<usize> {
    match dget(v, key) {
        Some(PyValue::List(l)) => Some(l.len()),
        _ => None,
    }
}

fn gbool(v: &PyValue, key: &str) -> Option<bool> {
    match dget(v, key) {
        Some(PyValue::Bool(b)) => Some(*b),
        _ => None,
    }
}

fn first(v: &PyValue, key: &str) -> PyValue {
    match dget(v, key) {
        Some(PyValue::List(l)) if !l.is_empty() => l[0].clone(),
        _ => PyValue::None,
    }
}

fn files_of(paths: &[&str]) -> Vec<PyValue> {
    let raw: Vec<PyValue> = paths.iter().map(|p| dict(vec![("path", pstr(p))])).collect();
    classify_files(&raw).unwrap_or_default()
}

fn reader(rec: &PyValue) -> BlobReader<'_> {
    BlobReader { cache: None, recorded: Some(rec), to_sha_hint: None }
}

fn b_axis(paths: &[&str], prov: Option<&PyValue>, rec: &PyValue) -> (PyValue, Vec<String>) {
    axis_b(&files_of(paths), prov, None, &reader(rec), "F", "T", None, None)
}

fn pipeline(files: PyValue, source: Option<&str>) -> PyValue {
    let mut rp = PyDict::new();
    rp.set("delta_files", files);
    if let Some(s) = source {
        rp.set("delta_source", pstr(s));
    } else {
        rp.set("delta_source", PyValue::None);
    }
    pipeline_raw(PyValue::Dict(rp))
}

fn pipeline_raw(rp: PyValue) -> PyValue {
    let generated = pstr("00000000");
    let i = Inputs {
        repo: "fixture/repo",
        from_ref: "A",
        to_ref: "B",
        generated_kst: &generated,
        repo_cache: None,
        provenance: None,
        bundle_files_root: None,
        resolved: None,
        replay: Some(&rp),
        scope_origin: None,
        scope_path: None,
    };
    run_pipeline(&i).unwrap_or(PyValue::None)
}

fn doc_files(n: usize) -> PyValue {
    PyValue::List((0..n).map(|k| dict(vec![("path", pstr(&format!("docs/fixture-{k}.md")))])).collect())
}

fn gitc(dir: &str, extra: &[&str], args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(dir).args(extra).args(args).status().map(|s| s.success()).unwrap_or(false)
}

pub fn run_selftest() -> i32 {
    let fails: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let check = |name: &str, cond: bool, detail: &str| {
        let tail = if !cond && !detail.is_empty() { format!(" — {detail}") } else { String::new() };
        gate_cli::write_out(&format!("  {} {name}{tail}\n", if cond { "ok  " } else { "FAIL" }));
        if !cond {
            fails.borrow_mut().push(name.to_string());
        }
    };
    let say = |t: &str| gate_cli::write_out(&format!("{t}\n"));

    say("[glob/plane]");
    check("vllm py → RUNTIME_PY", classify_plane("vllm/model_executor/models/qwen3_dspark.py").0 == "RUNTIME_PY", "");
    check("vllm cu → BUILD_INPUT", classify_plane("vllm/a/b.cu").0 == "BUILD_INPUT", "");
    check("docker → UPSTREAM_OWN_DOCKER", classify_plane("docker/Dockerfile").0 == "UPSTREAM_OWN_DOCKER", "");
    check("buildkite → CI", classify_plane(".buildkite/release-pipeline.yaml").0 == "CI", "");
    check("csrc → BUILD_INPUT", classify_plane("csrc/moe/x.cu").0 == "BUILD_INPUT", "");
    check("root md → DOCS", classify_plane("README.md").0 == "DOCS", "");
    for path in [
        ".agents/skills/model-recipes/SKILL.md",
        ".agents/skills/model-recipes/agents/openai.yaml",
        ".agents/skills/transformers-upgrade/scripts/dump_configs.py",
    ] {
        check(&format!("agent skill metadata → REPO_META: {path}"), classify_plane(path).0 == "REPO_META", "");
    }
    check("unknown agent plane remains unknown", classify_plane(".agents/runtime/loader.py").0 == UNKNOWN_PLANE, "");
    check("*.md 는 / 를 넘지 않음", classify_plane("newplane/a.md").0 == UNKNOWN_PLANE, "");
    check("열거 밖 → UNKNOWN_PLANE", classify_plane("newplane/thing.toml").0 == UNKNOWN_PLANE, "");

    say("[axis_a]");
    check("빌드입력 없음 → NO_IMPACT", gs(&axis_a(&files_of(&["docker/Dockerfile", ".buildkite/x.yaml"])).0, "verdict") == "NO_IMPACT", "");
    check("csrc 변경 → IMPACT", gs(&axis_a(&files_of(&["csrc/x.cu"])).0, "verdict") == "IMPACT", "");
    check("UNKNOWN_PLANE → UNDETERMINED", gs(&axis_a(&files_of(&["newplane/thing.toml"])).0, "verdict") == "UNDETERMINED", "");

    say("[compare completeness]");
    for (source, count, expected) in [
        (Some("github-compare-api"), 299usize, "NO_IMPACT"),
        (Some("github-compare-api"), 300, "UNDETERMINED"),
        (None, 300, "UNDETERMINED"),
        (Some("unknown"), 300, "UNDETERMINED"),
        (Some("git-local"), 300, "NO_IMPACT"),
    ] {
        let out = pipeline(doc_files(count), source);
        let incomplete = match dget(&out, "unknown") {
            Some(PyValue::List(l)) => l.iter().any(|s| sval(s).is_some_and(|t| t.contains("INCOMPLETE_DELTA"))),
            _ => false,
        };
        let want_incomplete = source != Some("git-local") && count >= 300;
        check(&format!("{} {count} files → {expected}", source.unwrap_or("None")), gs(&out, "verdict") == expected && incomplete == want_incomplete, "");
    }
    let mut rp = PyDict::new();
    rp.set("delta_files", doc_files(300));
    let ms = pipeline_raw(PyValue::Dict(rp));
    let inc = matches!(dget(&ms, "unknown"), Some(PyValue::List(l)) if l.iter().any(|s| sval(s).is_some_and(|t| t.contains("INCOMPLETE_DELTA"))));
    check("missing delta_source 300 files → UNDETERMINED", gs(&ms, "verdict") == "UNDETERMINED" && inc, "");

    say("[axis_b 프로브]");
    let p = "vllm/x.py";
    let prov = jv(r#"{"files": {"vllm/x.py": "aaa"}}"#);
    let rec_ft = jv(r#"{"F": {"vllm/x.py": "aaa"}, "T": {"vllm/x.py": "bbb"}}"#);
    let (b, _) = b_axis(&[p], Some(&prov), &rec_ft);
    check("번들=from ≠ to → WOULD_REVERT", gs(&first(&b, "silent_revert_risk"), "verdict") == "WOULD_REVERT" && gs(&b, "verdict") == "IMPACT", "");
    let prov_b = jv(r#"{"files": {"vllm/x.py": "bbb"}}"#);
    let (b, _) = b_axis(&[p], Some(&prov_b), &rec_ft);
    check("번들=to → NO_REVERT", gs(&first(&b, "probes"), "verdict") == "NO_REVERT", "");
    check("NO_REVERT 는 silent_revert_risk 에 담기지 않음(§5.3 정지조건 도달 가능)", glen(&b, "silent_revert_risk") == Some(0) && glen(&b, "probes") == Some(1), "");
    let prov_c = jv(r#"{"files": {"vllm/x.py": "ccc"}, "source": {"upstream_base_tag": "T"}}"#);
    let (b, _) = b_axis(&[p], Some(&prov_c), &rec_ft);
    let e = first(&b, "probes");
    check("파생베이스=to_ref 여도 verdict 는 fail-closed 유지", gs(&e, "verdict") == "WOULD_REVERT" && gbool(&e, "bundle_base_is_to_ref") == Some(true) && dget(&e, "note").is_some(), "");
    let rec_f = jv(r#"{"F": {"vllm/x.py": "aaa"}}"#);
    let (b, u) = b_axis(&[p], Some(&prov), &rec_f);
    check("상류 조회 실패 → UNDETERMINED", gs(&b, "verdict") == "UNDETERMINED" && u.len() == 1, "");
    let rec_empty = jv("{}");
    let (b, _) = b_axis(&["other.md"], Some(&prov), &rec_empty);
    check("교차 없음 → NO_IMPACT", gs(&b, "verdict") == "NO_IMPACT" && glen(&b, "overlap") == Some(0), "");
    let (b, _) = b_axis(&[p], None, &rec_empty);
    check("PROVENANCE 없음 → applicable=False·NO_IMPACT", gs(&b, "verdict") == "NO_IMPACT" && gbool(&b, "applicable") == Some(false), "");

    say("[worst-wins]");
    let v = |x: &str| dict(vec![("verdict", pstr(x))]);
    let o = synthesize(PyDict::new(), vec![], v("NO_IMPACT"), v("IMPACT"), vec![], None);
    check("A=NO,B=IMPACT → IMPACT", gs(&o, "verdict") == "IMPACT", "");
    let o = synthesize(PyDict::new(), vec![], v("NO_IMPACT"), v("NO_IMPACT"), vec!["x".into()], None);
    check("unknown 있으면 UNDETERMINED", gs(&o, "verdict") == "UNDETERMINED", "");
    let o = synthesize(PyDict::new(), vec![], v("NO_IMPACT"), v("NO_IMPACT"), vec![], None);
    check("전부 NO → NO_IMPACT", gs(&o, "verdict") == "NO_IMPACT", "");
    let axes_ok = matches!(dget(&o, "axes_in_global"), Some(PyValue::List(l)) if l.len() == 2 && sval(&l[0]).as_deref() == Some("axis_A_build_input") && sval(&l[1]).as_deref() == Some("axis_B_port_scope"));
    check("C축은 전역에서 제외", axes_ok && dget(&o, "axis_C_model_path").is_some_and(|c| gs(c, "verdict") == "NOT_IMPLEMENTED"), "");

    say("[손저작 감사]");
    let res = jv(r#"{"upstream_delta_a_to_b": {"files": 1, "detail": [{"file": "docker/Dockerfile", "delta": "+6/-2"}]}}"#);
    let raw = vec![jv(r#"{"path": "docker/Dockerfile", "additions": 5, "deletions": 1}"#)];
    let au = audit_manual_block(Some(&res), &raw).ok().flatten();
    let ok = au.as_ref().is_some_and(|a| gbool(a, "match") == Some(false) && gs(&first(a, "discrepancies"), "measured") == "+5/-1");
    check("+6/-2 vs +5/-1 불일치 검출", ok, "");

    say("[D0 git-local (합성 리포)]");
    let tmp = std::env::temp_dir().join(format!("judge-selftest-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    let t = tmp.to_string_lossy().into_owned();
    let built = (|| -> Option<()> {
        let id = ["-c", "user.email=t@t", "-c", "user.name=t"];
        std::fs::create_dir_all(&tmp).ok()?;
        Command::new("git").args(["init", "-q", &t]).status().ok()?.success().then_some(())?;
        std::fs::create_dir_all(tmp.join("vllm")).ok()?;
        std::fs::write(tmp.join("vllm").join("m.py"), "a=1\n").ok()?;
        gitc(&t, &[], &["add", "-A"]).then_some(())?;
        gitc(&t, &id, &["commit", "-qm", "c1"]).then_some(())?;
        gitc(&t, &[], &["tag", "T1"]).then_some(())?;
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(tmp.join("vllm").join("m.py")).ok()?;
            f.write_all(b"b=2\nc=3\n").ok()?;
        }
        std::fs::create_dir_all(tmp.join("docker")).ok()?;
        std::fs::write(tmp.join("docker").join("Dockerfile"), "FROM x\n").ok()?;
        gitc(&t, &[], &["add", "-A"]).then_some(())?;
        gitc(&t, &id, &["commit", "-qm", "c2"]).then_some(())?;
        gitc(&t, &[], &["tag", "T2"]).then_some(())?;
        Some(())
    })();
    if built.is_none() {
        check("git-local 델타 수집", false, "합성 리포 구성 실패");
    } else {
        match collect_delta_git(&t, "T1", "T2") {
            Ok(d) => {
                let mut got: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>)> = d
                    .files
                    .iter()
                    .map(|f| (gs(f, "path"), gs(f, "status"), dget(f, "additions").and_then(int_of), dget(f, "deletions").and_then(int_of), dget(f, "hunks").and_then(int_of)))
                    .collect();
                got.sort();
                let want = vec![
                    ("docker/Dockerfile".to_string(), "added".to_string(), Some(1), Some(0), Some(1)),
                    ("vllm/m.py".to_string(), "modified".to_string(), Some(2), Some(0), Some(1)),
                ];
                check("git-local 델타 수집", got == want, &format!("{got:?}"));
                check("git-local delta_source 라벨", matches!(&d.delta_source, PyValue::Str(s) if s.equals("git-local")), "");
            }
            Err(e) => check("git-local 델타 수집", false, &e),
        }
        let r = BlobReader { cache: Some(&t), recorded: None, to_sha_hint: None };
        let (st, sha) = r.sha("T1", "vllm/m.py");
        check("blob@T1", st == "ok" && sha.as_deref() == Some(sha256_hex(b"a=1\n").as_str()), "");
        check("blob 부재 판별", r.sha("T1", "docker/Dockerfile") == ("absent", None), "");
    }
    let _ = std::fs::remove_dir_all(&tmp);

    let fails = fails.into_inner();
    say(&format!("\nself-test: {}", if fails.is_empty() { "PASS".to_string() } else { format!("FAIL {}", fails.join(", ")) }));
    i32::from(!fails.is_empty())
}
