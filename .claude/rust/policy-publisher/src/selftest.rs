//! `--self-test`: the original's built-in roundtrip (in-process invocations against hermetic temporary repositories;
//! `finalize` still runs the real completion gate). Same probes, same order, same failure texts.

use crate::bench_section::MEASUREMENT_CONFIG_TITLE;
use crate::gate;
use crate::json::*;
use crate::publisher::{relpath, validate_record_shape, Ctx, Exit, PLACEHOLDER};
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::path::{Path, PathBuf};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        let base = std::env::temp_dir();
        for n in 0..1000u32 {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
            let p = base.join(format!("tmp{:x}{:x}{n}", std::process::id(), nanos));
            if std::fs::create_dir(&p).is_ok() {
                return TempDir(p);
            }
        }
        panic!("no temporary directory");
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct T<'a> {
    ctx: &'a Ctx,
    root: PathBuf,
}
type Out = (i32, Option<V>);

impl T<'_> {
    fn invoke(&self, argv: &[&str]) -> Out {
        let args: Vec<String> = argv.iter().map(|x| x.to_string()).collect();
        let Exit { code, stdout } = crate::run(self.ctx, &args);
        let v = if stdout.trim().is_empty() { None } else { pyjson::loads(&stdout).ok() };
        (code, v)
    }
    fn r(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.root.join(rel);
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(p, text).expect("write fixture");
    }
    fn read_json(&self, rel: &str) -> V {
        pyjson::loads(&std::fs::read_to_string(self.root.join(rel)).unwrap_or_default()).unwrap_or(PyValue::None)
    }
    fn listdir(&self, rel: &str) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.root.join(rel))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }
}

fn okv(o: &Out) -> bool {
    o.0 == 0 && o.1.as_ref().map(|v| matches!(get(v, "ok"), Some(PyValue::Bool(true)))).unwrap_or(false)
}
fn codes(o: &Out) -> Vec<String> {
    o.1.as_ref().and_then(|v| get(v, "reason_codes")).map(|l| match l.peel() {
        PyValue::List(x) => x.iter().filter_map(text).collect(),
        _ => vec![],
    }).unwrap_or_default()
}
fn has(o: &Out, code: &str) -> bool {
    codes(o).iter().any(|c| c == code)
}
fn rep(o: &Out) -> String {
    o.1.as_ref().map(|v| v.repr()).unwrap_or_else(|| "None".into())
}
fn field(o: &Out, k: &str) -> V {
    o.1.as_ref().and_then(|v| get(v, k)).unwrap_or(PyValue::None)
}
fn at(v: &V, path: &[&str]) -> V {
    let mut cur = v.clone();
    for k in path {
        cur = get(&cur, k).unwrap_or(PyValue::None);
    }
    cur
}
fn st(v: &V) -> Option<String> {
    text(v)
}
fn list_has(v: &V, item: &str) -> bool {
    matches!(v.peel(), PyValue::List(x) if x.iter().any(|y| text(y).as_deref() == Some(item)))
}

pub fn self_test(ctx: &Ctx) -> Exit {
    match run_all(ctx) {
        Ok(()) => {
            println!("[evidence_publisher] self-test PASS");
            Exit { code: 0, stdout: String::new() }
        }
        Err(m) => {
            eprintln!("Traceback (most recent call last):\nRuntimeError: {m}");
            Exit { code: 1, stdout: String::new() }
        }
    }
}

fn need(cond: bool, msg: impl FnOnce() -> String) -> Result<(), String> {
    if cond {
        Ok(())
    } else {
        Err(msg())
    }
}

const IDENTITY: &str = r#"{"model": "self-test-model", "gpu": "GB10", "vllm": "0.0.0", "quant": null, "topology": "single", "tp": 1}"#;

fn run_all(ctx: &Ctx) -> Result<(), String> {
    // ── minor_patch roundtrip ──
    {
        let td = TempDir::new();
        let t = T { ctx, root: td.0.clone() };
        for d in ["plan", "devlog", "testlog", "simlog", "benchmark", "report"] {
            let _ = std::fs::create_dir_all(t.root.join("docs").join(d));
        }
        t.write("identity.json", IDENTITY);
        let rs_ = t.r();
    let r = rs_.as_str();
        let idp = format!("{r}/identity.json");
        let o = t.invoke(&["init", "--repo-root", r, "--topic", "selftest", "--task-class", "minor_patch", "--generated-utc", "2026-01-01T00:00:00Z", "--identity-json", idp.as_str()]);
        need(okv(&o), || format!("self-test init failed: {}", rep(&o)))?;
        let mut req: Vec<String> = match field(&o, "required_evidence").peel() {
            PyValue::List(x) => x.iter().filter_map(text).collect(),
            _ => vec![],
        };
        req.sort();
        need(req == vec!["commit", "verification"], || format!("self-test required evidence mismatch: {}", rep(&o)))?;
        t.write("verify.txt", "py_compile OK\n");
        t.write("commit.txt", "deadbeef\n");
        for (kind, src) in [("verification", "verify.txt"), ("commit", "commit.txt")] {
            let o = t.invoke(&["append-raw", "--repo-root", r, "--topic", "selftest", "--kind", kind, "--src", src, "--recorded-utc", "2026-01-01T00:01:00Z"]);
            need(okv(&o), || format!("self-test append-raw failed for {kind}: {}", rep(&o)))?;
        }
        let mdir = format!("{r}/docs/_evidence");
        let pii = obj(vec![("passed", b(true)), ("scanned_paths", list_s(&[relpath(&format!("{r}/verify.txt"), &mdir), relpath(&format!("{r}/commit.txt"), &mdir)]))]);
        t.write("pii.json", &dumps_ascii(&pii));
        t.write(
            "runtime.json",
            &format!(r#"{{"health_ok": true, "functional_smoke_passed": true, "identity": {IDENTITY}, "containers": [{{"name": "svc", "restart_count": 0, "oom_killed": false}}]}}"#),
        );
        let o = t.invoke(&["finalize", "--repo-root", r, "--topic", "selftest", "--pii-scan-json", &format!("{r}/pii.json"), "--runtime-json", &format!("{r}/runtime.json")]);
        need(st(&field(&o, "state")).as_deref() == Some("evidence-complete"), || format!("self-test finalize state mismatch: {}", rep(&o)))?;
        need(matches!(field(&o, "eligible_for_promotion").peel(), PyValue::Bool(false)), || format!("self-test promotion cap mismatch: {}", rep(&o)))?;
    }
    // ── publish-benchmark binding and the rest ──
    let td = TempDir::new();
    let t = T { ctx, root: td.0.clone() };
    for d in ["plan", "devlog", "testlog", "simlog", "benchmark", "report", "_evidence/inputs"] {
        let _ = std::fs::create_dir_all(t.root.join("docs").join(d));
    }
    t.write("identity.json", IDENTITY);
    let rs_ = t.r();
    let r = rs_.as_str();
    let idp = format!("{r}/identity.json");
    let cert_text = "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: self-test-model\ngpu_model: GB10\nvllm_version: 0.0.0\nquantization: N/A\ntopology: single\ntensor_parallel_size: 1\nbenchmark_mode: full\ndecode_tps_conc1: 12.3\nrubric_authority: weak\nprimary_source: expected_achievable(roofline×MBU)\nprimary_tps: 10.0\nfloor_tps: 8.5\ntolerance: 0.15\nratio_M_over_primary: 1.23\nmeasured_utc: \"2026-01-01T00:00:00Z\"\n";
    let stem = "26010109_self-test-model_GB10_0.0.0";
    let cert_rel = format!("docs/benchmark/benchmark_{stem}.yaml");
    let rep_rel = format!("docs/benchmark/bench_report_{stem}.md");
    t.write(&cert_rel, cert_text);
    t.write(&rep_rel, "# report\n생성일 2026-01-01T00:00:00Z.\n");
    let o = t.invoke(&["init", "--repo-root", r, "--topic", "bench", "--task-class", "full_benchmark", "--generated-utc", "2026-01-01T02:00:00Z", "--identity-json", idp.as_str(), "--benchmark-mode", "full", "--benchmark-verdict", "PASS"]);
    need(okv(&o), || format!("bench init failed: {}", rep(&o)))?;
    let before = t.listdir("docs/benchmark");
    let publish = |verdict: &str, cert: Option<&str>, report: &str, utc: &str| -> Out {
        let mut argv = vec!["publish-benchmark", "--repo-root", r, "--topic", "bench", "--verdict", verdict, "--generated-utc", utc, "--bench-report-src", report];
        if let Some(c) = cert {
            argv.push("--certificate-src");
            argv.push(c);
        }
        t.invoke(&argv)
    };
    let u = "2026-01-01T02:05:00Z";
    let o = publish("PASS", Some(&cert_rel), &rep_rel, u);
    need(okv(&o), || format!("bind publish failed: {}", rep(&o)))?;
    need(st(&field(&o, "certificate_path")).as_deref() == Some(cert_rel.as_str()) && st(&field(&o, "bench_report_path")).as_deref() == Some(rep_rel.as_str()), || {
        format!("publish must BIND the supplied canonical paths, got {}", rep(&o))
    })?;
    let binding = field(&o, "binding");
    let key_last = match at(&binding, &["certificate_key"]).peel() {
        PyValue::List(x) => x.last().and_then(text),
        _ => None,
    };
    need(st(&at(&binding, &["certificate"])).as_deref() == Some("bound") && key_last.as_deref() == Some("2026-01-01T00:00:00Z"), || {
        format!("binding provenance missing: {}", binding.repr())
    })?;
    need(t.listdir("docs/benchmark") == before, || "publish-benchmark must not create files under docs/benchmark/ (no copies)".into())?;
    let rec = t.read_json("docs/_evidence/bench.json");
    need(st(&at(&rec, &["benchmark", "measured_utc"])).as_deref() == Some("2026-01-01T00:00:00Z"), || {
        format!("record.benchmark.measured_utc must derive from the certificate: {}", at(&rec, &["benchmark"]).repr())
    })?;
    t.write("docs/_evidence/inputs/x.yaml", cert_text);
    let o = publish("PASS", Some("docs/_evidence/inputs/x.yaml"), &rep_rel, u);
    need(o.0 == 2 && has(&o, "PUBLISH_BENCHMARK_CERTIFICATE_SRC_NOT_CANONICAL"), || format!("non-canonical src must be rejected, got {}", rep(&o)))?;
    let dup_rel = "docs/benchmark/benchmark_26010109_00_00_self-test-model_GB10_0.0.0.yaml";
    t.write(dup_rel, cert_text);
    let o = publish("PASS", Some(&cert_rel), &rep_rel, u);
    need(o.0 == 2 && has(&o, "PUBLISH_BENCHMARK_CERTIFICATE_AMBIGUOUS"), || format!("duplicate certificate must be AMBIGUOUS, got {}", rep(&o)))?;
    need(o.1.as_ref().map(dumps_compact_sorted).unwrap_or_default().contains("byte-identical"), || "AMBIGUOUS must say whether the duplicates are byte-identical".into())?;
    let _ = std::fs::remove_file(t.root.join(dup_rel));
    let other_rep = "docs/benchmark/bench_report_26010110_self-test-model_GB10_0.0.0.md";
    t.write(other_rep, "# r\n생성일 2026-01-01T01:00:00Z.\n");
    let o = publish("PASS", Some(&cert_rel), other_rep, u);
    need(o.0 == 2 && has(&o, "PUBLISH_BENCHMARK_REPORT_CERTIFICATE_STEM_MISMATCH"), || format!("stem mismatch must be rejected, got {}", rep(&o)))?;
    let _ = std::fs::remove_file(t.root.join(other_rep));
    let o = publish("REFUTE", None, &rep_rel, "2026-01-01T02:10:00Z");
    need(okv(&o) && is_none(&field(&o, "certificate_path")), || format!("REFUTE republish failed: {}", rep(&o)))?;
    need(st(&at(&field(&o, "binding"), &["certificate_unbound_from"])).as_deref() == Some(cert_rel.as_str()), || {
        format!("unbind must be surfaced, got {}", field(&o, "binding").repr())
    })?;
    need(t.root.join(&cert_rel).is_file(), || "PASS→REFUTE must UNBIND, never unlink the skill-issued certificate".into())?;
    let publish_vj = |authority: &str, verdict: &str| -> Out {
        let rel = format!("docs/_evidence/inputs/verdict_{authority}_{verdict}.json");
        t.write(&rel, &format!(r#"{{"verdict": "{verdict}", "rubric": {{"authority": "{authority}", "floor": 8.5, "ratio_M_over_primary": 1.23, "source": "expected_achievable(roofline×MBU)"}}}}"#));
        t.invoke(&["publish-benchmark", "--repo-root", r, "--topic", "bench", "--verdict", verdict, "--generated-utc", "2026-01-01T02:20:00Z", "--bench-report-src", &rep_rel, "--verdict-json-src", &rel])
    };
    for (authority, expect) in [("explore", false), ("weak", false), ("explicit", true)] {
        let o = publish_vj(authority, "PASS");
        need(okv(&o), || format!("PASS+{authority} publish (verdict json, no certificate) failed: {}", rep(&o)))?;
        let rec = t.read_json("docs/_evidence/bench.json");
        let pending = matches!(field(&o, "pending_certificate").peel(), PyValue::Bool(x) if *x == expect);
        need(pending && list_has(&at(&rec, &["required_evidence"]), "certificate") == expect, || {
            format!(
                "PASS+{authority}: pending_certificate={} required={} (certificate expected only for explicit)",
                field(&o, "pending_certificate").repr(),
                at(&rec, &["required_evidence"]).repr()
            )
        })?;
        need(validate_record_shape(&rec).is_none(), || format!("PASS+{authority} record fails its own load validation: {:?}", validate_record_shape(&rec)))?;
    }
    let o = publish("PASS", None, &rep_rel, "2026-01-01T02:25:00Z");
    let rec = t.read_json("docs/_evidence/bench.json");
    need(o.0 == 0 && matches!(field(&o, "pending_certificate").peel(), PyValue::Bool(true)) && list_has(&at(&rec, &["required_evidence"]), "certificate"), || {
        format!("PASS without a derived authority must keep requiring the certificate: {}", rep(&o))
    })?;
    let o = publish("REFUTE", None, &rep_rel, "2026-01-01T02:30:00Z");
    need(okv(&o), || format!("REFUTE re-publish after the waiver probes failed: {}", rep(&o)))?;
    let publish_obs = |doc: &str, with_json: bool| -> Out {
        t.write("docs/_evidence/inputs/verdict_observation.json", doc);
        let mut argv = vec!["publish-benchmark", "--repo-root", r, "--topic", "bench", "--verdict", "OBSERVATION-ONLY", "--generated-utc", "2026-01-01T02:35:00Z", "--bench-report-src", &rep_rel];
        if with_json {
            argv.push("--verdict-json-src");
            argv.push("docs/_evidence/inputs/verdict_observation.json");
        }
        t.invoke(&argv)
    };
    let nb = r#"{"verdict": "NEEDS_RUBRIC", "failure_axis": "establish", "reason_code": null, "rubric": {"authority": "weak", "primary": null, "source": null, "floor": null, "ratio_M_over_primary": null}}"#;
    let o = publish_obs(nb, true);
    let rec = t.read_json("docs/_evidence/bench.json");
    need(
        okv(&o)
            && st(&at(&rec, &["benchmark", "verdict"])).as_deref() == Some("OBSERVATION-ONLY")
            && is_none(&at(&rec, &["benchmark", "floor_tps"]))
            && st(&at(&rec, &["benchmark", "rubric_source"])).as_deref() == Some("verdict_json")
            && !list_has(&at(&rec, &["required_evidence"]), "certificate")
            && validate_record_shape(&rec).is_none(),
        || format!("OBSERVATION-ONLY ↔ NEEDS_RUBRIC(no baseline) publish failed: {} {}", rep(&o), at(&rec, &["benchmark"]).repr()),
    )?;
    for (label, doc, with_json, want) in [
        ("physical-exceeds", r#"{"verdict": "NEEDS_RUBRIC", "failure_axis": "establish", "reason_code": null, "rubric": {"authority": "weak", "primary": 59.0, "source": "E(external_reference)", "floor": null, "ratio_M_over_primary": null}}"#, true, "PUBLISH_BENCHMARK_VERDICT_JSON_VERDICT_MISMATCH"),
        ("spec-missing", r#"{"verdict": "NEEDS_RUBRIC", "failure_axis": "establish", "reason_code": "SPEC_ACCEPT_LEN_MISSING", "rubric": {"authority": "weak", "primary": null, "source": null, "floor": null, "ratio_M_over_primary": null}}"#, true, "PUBLISH_BENCHMARK_VERDICT_JSON_VERDICT_MISMATCH"),
        ("refute-json", r#"{"verdict": "REFUTE", "failure_axis": "establish", "reason_code": null, "rubric": {"authority": "weak", "primary": null, "source": null, "floor": null, "ratio_M_over_primary": null}}"#, true, "PUBLISH_BENCHMARK_VERDICT_JSON_VERDICT_MISMATCH"),
        ("no-json", nb, false, "PUBLISH_BENCHMARK_OBSERVATION_ONLY_REQUIRES_VERDICT_JSON"),
    ] {
        let o = publish_obs(doc, with_json);
        need(o.0 == 2 && has(&o, want), || format!("OBSERVATION-ONLY must be rejected ({label}): {}", rep(&o)))?;
    }
    let o = publish("REFUTE", None, &rep_rel, "2026-01-01T02:40:00Z");
    need(okv(&o), || format!("REFUTE re-publish after the observation probes failed: {}", rep(&o)))?;
    // set-narrative binding
    let plan_src = "docs/plan/plan_26010109_selftest_source.md";
    t.write(plan_src, "# plan\n실제 저작 본문.\n");
    let o = t.invoke(&["set-narrative", "--repo-root", r, "--topic", "bench", "--kind", "plan", "--narrative-file", plan_src, "--author", "selftest", "--generated-utc", "2026-01-01T02:20:00Z"]);
    need(o.0 == 0 && st(&field(&o, "status")).as_deref() == Some("bound") && st(&field(&o, "path")).as_deref() == Some(plan_src), || {
        format!("canonical narrative must be BOUND, got {}", rep(&o))
    })?;
    need(at(&field(&o, "binding"), &["scaffold_removed"]).is_truthy(), || "init's unauthored placeholder scaffold must be removed on bind".into())?;
    need(t.listdir("docs/plan") == vec!["plan_26010109_selftest_source.md".to_string()], || {
        format!("bind must leave exactly the bound original: {:?}", t.listdir("docs/plan"))
    })?;
    let rec = t.read_json("docs/_evidence/bench.json");
    need(st(&at(&rec, &["scaffolded", "plan"])).as_deref() == Some(plan_src) && st(&at(&rec, &["narrative_status", "plan", "status"])).as_deref() == Some("bound"), || {
        format!("record must point at the bound original: {}", at(&rec, &["scaffolded", "plan"]).repr())
    })?;
    t.write("docs/_evidence/inputs/dev.md", "서사 본문\n");
    let o = t.invoke(&["set-narrative", "--repo-root", r, "--topic", "bench", "--kind", "devlog", "--narrative-file", "docs/_evidence/inputs/dev.md", "--author", "selftest", "--generated-utc", "2026-01-01T02:21:00Z"]);
    need(o.0 == 0 && st(&field(&o, "status")).as_deref() == Some("authored"), || format!("temporary source must take the injection path, got {}", rep(&o)))?;
    let ph = "docs/testlog/testlog_26010109_selftest_ph.md";
    t.write(ph, &format!("{PLACEHOLDER}\n"));
    let o = t.invoke(&["set-narrative", "--repo-root", r, "--topic", "bench", "--kind", "testlog", "--narrative-file", ph, "--author", "selftest", "--generated-utc", "2026-01-01T02:22:00Z"]);
    need(o.0 == 2 && has(&o, "NARRATIVE_BIND_TARGET_IS_PLACEHOLDER"), || format!("placeholder document must not be bindable, got {}", rep(&o)))?;
    let _ = std::fs::remove_file(t.root.join(plan_src));
    let o = t.invoke(&["init", "--repo-root", r, "--topic", "bench", "--task-class", "full_benchmark", "--generated-utc", "2026-01-01T02:00:00Z", "--identity-json", idp.as_str()]);
    need(okv(&o) && !list_has(&field(&o, "created_now"), "plan") && !t.root.join(plan_src).exists(), || {
        format!("re-init must not synthesize a missing BOUND original: {}", rep(&o))
    })?;
    // downgrade
    let down_init = |topic: &str, tc: &str, extra: &[&str]| -> Out {
        let mut argv = vec!["init", "--repo-root", r, "--topic", topic, "--task-class", tc, "--generated-utc", "2026-01-01T03:00:00Z", "--identity-json", idp.as_str()];
        argv.extend_from_slice(extra);
        t.invoke(&argv)
    };
    let mc_report = |utc: &str, rows: &[(&str, &str)]| -> String {
        let mut s_ = format!("# report\n생성일 {utc}.\n\n{MEASUREMENT_CONFIG_TITLE}\n\n| 키 | 값 |\n|---|---|\n");
        for (k, v) in rows {
            s_.push_str(&format!("| {k} | {v} |\n"));
        }
        s_
    };
    let full_topic = |topic: &str, stem: &str, text_: &str| -> Result<String, String> {
        let rel = format!("docs/benchmark/bench_report_{stem}.md");
        t.write(&rel, text_);
        let o = down_init(topic, "full_benchmark", &["--benchmark-mode", "full", "--benchmark-verdict", "PASS"]);
        need(okv(&o), || format!("downgrade fixture init {topic} failed: {}", rep(&o)))?;
        let o = t.invoke(&["publish-benchmark", "--repo-root", r, "--topic", topic, "--verdict", "PASS", "--generated-utc", "2026-01-01T03:05:00Z", "--bench-report-src", &rel]);
        need(o.0 == 0 && matches!(field(&o, "pending_certificate").peel(), PyValue::Bool(true)), || {
            format!("downgrade fixture publish {topic} (full 정의 미충족 → 인증서 억제 상태) failed: {}", rep(&o))
        })?;
        Ok(rel)
    };
    let drep = full_topic(
        "down",
        "26010112_self-test-model_GB10_0.0.0",
        &mc_report("2026-01-01T03:00:00Z", &[("bench_mode", "lite"), ("bench_mode_kind", "downgraded-lite"), ("bench_mode_source", "반복 조건 불성립(판정점 run 2 실패)"), ("downgrade_reason", "run_failed")]),
    )?;
    full_topic(
        "downfull",
        "26010114_self-test-model_GB10_0.0.0",
        &mc_report("2026-01-01T03:10:00Z", &[("bench_mode", "full"), ("bench_mode_kind", "full"), ("bench_mode_source", "반복 조건 성립"), ("downgrade_reason", "N/A")]),
    )?;
    full_topic("downlegacy", "26010115_self-test-model_GB10_0.0.0", "# report\n생성일 2026-01-01T03:20:00Z.\n\n| 동시성 | decode t/s |\n|---|---|\n| 1 | 9 |\n")?;
    let o = down_init("downnoreport", "full_benchmark", &["--benchmark-mode", "full", "--benchmark-verdict", "PASS"]);
    need(okv(&o), || format!("downgrade fixture init downnoreport failed: {}", rep(&o)))?;
    for (topic, reason, want) in [
        ("downfull", "run_failed", "INIT_DOWNGRADE_EVIDENCE_MISMATCH"),
        ("downlegacy", "run_failed", "INIT_DOWNGRADE_EVIDENCE_MISMATCH"),
        ("down", "blackbox_kill", "INIT_DOWNGRADE_EVIDENCE_MISMATCH"),
        ("downnoreport", "run_failed", "INIT_DOWNGRADE_REPORT_ABSENT"),
    ] {
        let o = down_init(topic, "hint_map_only", &["--downgrade-from", "full_benchmark", "--downgrade-reason", reason]);
        need(o.0 == 2 && has(&o, want), || {
            format!("★downgrade of {} with reason {} must reject with {want} (the bound report does not record that downgrade), got {}", repr(topic), repr(reason), rep(&o))
        })?;
        let neg = t.read_json(&format!("docs/_evidence/{topic}.json"));
        need(st(&at(&neg, &["task_class"])).as_deref() == Some("full_benchmark") && get(&neg, "reclassification").is_none(), || {
            format!("★a rejected downgrade must leave the record untouched: {}", neg.repr())
        })?;
    }
    for (argv, want) in [
        (vec![], "INIT_IMMUTABLE_REBIND"),
        (vec!["--downgrade-from", "full_benchmark"], "INIT_DOWNGRADE_REASON_REQUIRED"),
        (vec!["--downgrade-from", "full_benchmark", "--downgrade-reason", "variance"], "INIT_DOWNGRADE_REASON_UNKNOWN"),
        (vec!["--downgrade-reason", "run_failed"], "INIT_DOWNGRADE_REASON_WITHOUT_FROM"),
        (vec!["--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed", "--benchmark-mode", "full"], "INIT_DOWNGRADE_MODE_CONTRADICTION"),
    ] {
        let o = down_init("down", "hint_map_only", &argv);
        need(o.0 == 2 && has(&o, want), || format!("★downgrade negative {argv:?} must reject with {want}, got {}", rep(&o)))?;
    }
    let o = down_init("down", "harness_change", &["--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed"]);
    need(o.0 == 2 && has(&o, "INIT_DOWNGRADE_TARGET_INVALID"), || format!("★downgrade into a class other than hint_map_only must reject, got {}", rep(&o)))?;
    let o = down_init("nope", "hint_map_only", &["--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed"]);
    need(o.0 == 2 && has(&o, "INIT_DOWNGRADE_PRIOR_ABSENT"), || format!("★downgrade without a prior record must reject, got {}", rep(&o)))?;
    let o = down_init("down", "hint_map_only", &["--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed"]);
    need(okv(&o) && field(&o, "required_evidence") == list_s(&["plan", "devlog", "testlog"]) && st(&at(&field(&o, "reclassification"), &["reason"])).as_deref() == Some("run_failed"), || {
        format!("downgrade full_benchmark → hint_map_only with a reason must pass, got {}", rep(&o))
    })?;
    let rec = t.read_json("docs/_evidence/down.json");
    need(
        st(&at(&rec, &["task_class"])).as_deref() == Some("hint_map_only")
            && st(&at(&rec, &["benchmark", "mode"])).as_deref() == Some("lite")
            && st(&at(&rec, &["benchmark", "verdict"])).as_deref() == Some("PASS")
            && st(&at(&rec, &["scaffolded", "bench_report"])).as_deref() == Some(drep.as_str())
            && st(&at(&rec, &["reclassification", "from"])).as_deref() == Some("full_benchmark")
            && st(&at(&rec, &["reclassification", "reason_source"])).map(|x| x.starts_with(&format!("derived(bench_report({drep})"))).unwrap_or(false),
        || format!("downgrade must preserve verdict/binding, set mode=lite and record provenance: {}", rec.repr()),
    )?;
    let o = down_init("down", "hint_map_only", &["--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed"]);
    need(okv(&o), || format!("re-running the same downgrade must be idempotent, got {}", rep(&o)))?;
    let o = down_init("down", "hint_map_only", &[]);
    need(o.0 == 0 && st(&at(&field(&o, "reclassification"), &["from"])).as_deref() == Some("full_benchmark"), || {
        format!("a plain re-init of a reclassified topic must carry the reclassification, got {}", rep(&o))
    })?;
    let o = down_init("down", "full_benchmark", &[]);
    need(o.0 == 2 && has(&o, "INIT_IMMUTABLE_REBIND"), || format!("★reverting a downgrade (map_only → full_benchmark) must stay REBIND, got {}", rep(&o)))?;
    let o = down_init("down", "hint_map_only", &["--downgrade-from", "full_benchmark", "--downgrade-reason", "blackbox_kill"]);
    need(o.0 == 2 && has(&o, "INIT_DOWNGRADE_PRIOR_MISMATCH"), || format!("★re-downgrading with a different reason must reject, got {}", rep(&o)))?;
    let mut rec_b = t.read_json("docs/_evidence/bench.json");
    if let Some(d) = dict_mut(&mut rec_b) {
        if let Some(sc) = d.get_mut("scaffolded").and_then(dict_mut) {
            sc.set("certificate", s(&cert_rel));
        }
    }
    t.write("docs/_evidence/bench.json", &dumps_ascii(&rec_b));
    let o = t.invoke(&["init", "--repo-root", r, "--topic", "bench", "--task-class", "hint_map_only", "--generated-utc", "2026-01-01T02:00:00Z", "--identity-json", idp.as_str(), "--downgrade-from", "full_benchmark", "--downgrade-reason", "run_failed"]);
    need(o.0 == 2 && has(&o, "INIT_DOWNGRADE_CERTIFICATE_BOUND"), || format!("★a record with a bound certificate is not a downgraded cell, got {}", rep(&o)))?;
    // publish-lite-report
    let lstem = "26010113_self-test-model_GB10_0.0.0";
    let lrep = format!("docs/benchmark/bench_report_{lstem}.md");
    t.write(
        &lrep,
        &format!("# 경량\n\nmode: lite\n\n{}", mc_report("2026-01-01T04:00:00Z", &[("bench_mode", "lite"), ("bench_mode_kind", "declared-lite"), ("bench_mode_source", "declared(lite_bench · lite-only 셀)")])),
    );
    let o = down_init("lite", "hint_map_only", &[]);
    need(okv(&o), || format!("map_only init failed: {}", rep(&o)))?;
    let lite_bind = |topic: &str, src: &str| -> Out {
        t.invoke(&["publish-lite-report", "--repo-root", r, "--topic", topic, "--generated-utc", "2026-01-01T04:05:00Z", "--bench-report-src", src])
    };
    let o = lite_bind("lite", &rep_rel);
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_NOT_LITE"), || format!("★a non-lite report must not be bound as lite evidence, got {}", rep(&o)))?;
    let o = lite_bind("bench", &lrep);
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_WRONG_TASK_CLASS"), || format!("★full_benchmark topic must use publish-benchmark, got {}", rep(&o)))?;
    let stray = "docs/benchmark/bench_report_26010116_self-test-model_GB10_0.0.0.md";
    t.write(stray, "# 산문\n생성일 2026-01-01T04:01:00Z.\n\n| bench_mode | lite |\n");
    let o = lite_bind("lite", stray);
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_NOT_LITE"), || format!("★a lone `| bench_mode | lite |` line outside a measurement-config table must not bind, got {}", rep(&o)))?;
    let o = lite_bind("lite", "docs/benchmark/bench_report_26010114_self-test-model_GB10_0.0.0.md");
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_NOT_LITE"), || format!("★a report whose table says bench_mode=full must not bind as lite evidence, got {}", rep(&o)))?;
    let before = t.listdir("docs/benchmark");
    let ltext = std::fs::read_to_string(t.root.join(&lrep)).unwrap_or_default();
    t.write("docs/_evidence/inputs/lite.md", &ltext);
    let o = lite_bind("lite", "docs/_evidence/inputs/lite.md");
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_BENCH_REPORT_SRC_NOT_CANONICAL"), || format!("★non-canonical lite report src must be rejected, got {}", rep(&o)))?;
    let o = lite_bind("lite", &lrep);
    need(o.0 == 0 && st(&field(&o, "bench_report_path")).as_deref() == Some(lrep.as_str()) && st(&field(&o, "benchmark_mode")).as_deref() == Some("lite"), || {
        format!("publish-lite-report must bind the canonical lite report, got {}", rep(&o))
    })?;
    need(t.listdir("docs/benchmark") == before, || "publish-lite-report must not create files under docs/benchmark/ (no copies)".into())?;
    let lcert = format!("docs/benchmark/benchmark_{lstem}.yaml");
    let lcert_text = "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: not_applicable\nmodel: self-test-model\ngpu_model: GB10\nvllm_version: 0.0.0\nquantization: N/A\ntopology: single\ntensor_parallel_size: 1\nbenchmark_mode: lite\nlite_verdict: pass\nentry_path: beta_lite_only_cell\nlite_included: true\nlite_gen_tps_warm: 40.0\nmeasured_utc: \"2026-01-01T04:00:00Z\"\nmeasured_node: main\n";
    let lite_cert_bind = |src: &str, text_: &str| -> Out {
        t.write(src, text_);
        t.invoke(&["publish-lite-report", "--repo-root", r, "--topic", "lite", "--generated-utc", "2026-01-01T04:05:00Z", "--bench-report-src", &lrep, "--certificate-src", src])
    };
    for (bad, want) in [
        (lcert_text.replace("lite_verdict: pass", "lite_verdict: server_failed"), "PUBLISH_LITE_REPORT_CERTIFICATE_GRADE_MISMATCH"),
        (lcert_text.replace("benchmark_mode: lite", "benchmark_mode: full"), "PUBLISH_LITE_REPORT_CERTIFICATE_GRADE_MISMATCH"),
        (lcert_text.replace("model: self-test-model", "model: other-model"), "PUBLISH_LITE_REPORT_CERTIFICATE_IDENTITY_MISMATCH"),
    ] {
        let o = lite_cert_bind(&lcert, &bad);
        need(o.0 == 2 && has(&o, want), || format!("★a non-lite-pass / mismatched certificate must not bind to a map-only topic ({want}), got {}", rep(&o)))?;
    }
    let other = "docs/benchmark/benchmark_26010115_self-test-model_GB10_0.0.0.yaml";
    let o = lite_cert_bind(other, &lcert_text.replace("04:00:00Z", "05:00:00Z"));
    let _ = std::fs::remove_file(t.root.join(other));
    need(o.0 == 2 && has(&o, "PUBLISH_LITE_REPORT_CERTIFICATE_STEM_MISMATCH"), || format!("★a lite certificate of a different measurement must not bind, got {}", rep(&o)))?;
    let o = lite_cert_bind(&lcert, lcert_text);
    need(o.0 == 0 && st(&field(&o, "certificate_path")).as_deref() == Some(lcert.as_str()) && st(&at(&field(&o, "binding"), &["certificate"])).as_deref() == Some("bound"), || {
        format!("publish-lite-report --certificate-src must bind the lite-grade certificate, got {}", rep(&o))
    })?;
    for kind in ["plan", "devlog", "testlog"] {
        let src = format!("docs/_evidence/inputs/lite_{kind}.md");
        t.write(&src, &format!("# {kind}\n경량 셀 서사.\n"));
        let o = t.invoke(&["set-narrative", "--repo-root", r, "--topic", "lite", "--kind", kind, "--narrative-file", &src, "--author", "selftest", "--generated-utc", "2026-01-01T04:06:00Z"]);
        need(o.0 == 0, || format!("lite narrative {kind} failed: {}", rep(&o)))?;
    }
    let rec_l = t.read_json("docs/_evidence/lite.json");
    let mdir = format!("{r}/docs/_evidence");
    let mut scanned: Vec<String> = ["plan", "devlog", "testlog", "bench_report", "certificate"]
        .iter()
        .map(|k| relpath(&format!("{r}/{}", st(&at(&rec_l, &["scaffolded", k])).unwrap_or_default()), &mdir))
        .collect();
    scanned.sort();
    t.write("pii_l.json", &dumps_ascii(&obj(vec![("passed", b(true)), ("scanned_paths", list_s(&scanned))])));
    t.write(
        "runtime_l.json",
        &format!(r#"{{"health_ok": true, "functional_smoke_passed": true, "identity": {IDENTITY}, "containers": [{{"name": "svc", "restart_count": 0, "oom_killed": false}}]}}"#),
    );
    let pii_l = format!("{r}/pii_l.json");
    let rt_l = format!("{r}/runtime_l.json");
    let o = t.invoke(&["finalize", "--repo-root", r, "--topic", "lite", "--pii-scan-json", &pii_l, "--runtime-json", &rt_l]);
    need(matches!(field(&o, "eligible_for_promotion").peel(), PyValue::Bool(true)) && has(&o, "HINT_MAP_ONLY_PROMOTION"), || {
        format!("a map_only topic with a bound lite report + lite certificate must reach the map publication path: {}", rep(&o))
    })?;
    let man = t.read_json("docs/_evidence/lite.work-manifest.json");
    need(
        st(&at(&man, &["evidence", "bench_report", "path"])).map(|p| p.ends_with(&format!("bench_report_{lstem}.md"))).unwrap_or(false)
            && st(&at(&man, &["benchmark", "mode"])).as_deref() == Some("lite")
            && st(&at(&man, &["evidence", "certificate", "path"])).map(|p| p.ends_with(&format!("benchmark_{lstem}.yaml"))).unwrap_or(false)
            && st(&at(&man, &["benchmark", "measured_utc"])).as_deref() == Some("2026-01-01T04:00:00Z"),
        || format!("work-manifest must carry the lite report + lite certificate (measured_utc from it): {}", man.repr()),
    )?;
    need(get(&man, "promotion_target").is_none(), || format!("★finalize without a recorded target must not invent promotion_target: {}", man.repr()))?;
    // set-promotion-target
    let a40 = "a".repeat(40);
    let b40 = "b".repeat(40);
    let tag_x = "hint/0.0.0/self-test-model/gb10-1g1n-main-native/qbf16-len1024-kvauto-plenone-specoff-graph";
    let set_target = |topic: &str, tag: &str, topology: &str, anchor: &str, utc: &str| -> Out {
        t.invoke(&["set-promotion-target", "--repo-root", r, "--topic", topic, "--tag", tag, "--topology", topology, "--anchor", anchor, "--generated-utc", utc])
    };
    let u5 = "2026-01-01T05:00:00Z";
    let rec_before = std::fs::read(t.root.join("docs/_evidence/lite.json")).unwrap_or_default();
    let upper = "A".repeat(40);
    for (tag, topo, anchor, why) in [
        (tag_x, "single TP=1", "abc123", "short anchor"),
        (tag_x, "single TP=1", upper.as_str(), "uppercase/non-hex anchor"),
        ("release/0.0.0", "single TP=1", a40.as_str(), "tag outside hint/"),
        ("hint/*", "single TP=1", a40.as_str(), "glob in tag"),
        (tag_x, "multi\nTP=2", a40.as_str(), "multi-line topology"),
    ] {
        let o = set_target("lite", tag, topo, anchor, u5);
        need(o.0 == 2 && has(&o, "PROMOTION_TARGET_INVALID"), || format!("★set-promotion-target must reject {why}, got {}", rep(&o)))?;
    }
    need(std::fs::read(t.root.join("docs/_evidence/lite.json")).unwrap_or_default() == rec_before, || "★a rejected set-promotion-target must leave the record byte-identical".into())?;
    let o = set_target("nope", tag_x, "single TP=1", &a40, u5);
    need(o.0 == 2, || format!("★set-promotion-target on an unknown topic must reject, got {}", rep(&o)))?;
    let o = set_target("lite", tag_x, "single TP=1", &a40, "2026-01-01 05:00");
    need(o.0 == 2, || format!("★set-promotion-target must require an injected UTC timestamp, got {}", rep(&o)))?;
    let o = set_target("lite", tag_x, "single TP=1", &a40, u5);
    need(o.0 == 0 && st(&at(&field(&o, "binding"), &["state"])).as_deref() == Some("bound") && st(&at(&field(&o, "promotion_target"), &["kind"])).as_deref() == Some("hint"), || {
        format!("set-promotion-target must bind the target into the record, got {}", rep(&o))
    })?;
    let o = set_target("lite", tag_x, "single TP=1", &a40, "2026-01-01T05:10:00Z");
    need(o.0 == 0 && st(&at(&field(&o, "binding"), &["state"])).as_deref() == Some("unchanged") && st(&at(&field(&o, "binding"), &["recorded_utc"])).as_deref() == Some(u5), || {
        format!("re-setting the same target must be idempotent (first recording time kept), got {}", rep(&o))
    })?;
    let o = set_target("lite", tag_x, "single TP=1", &b40, "2026-01-01T05:20:00Z");
    need(o.0 == 0 && st(&at(&field(&o, "binding"), &["state"])).as_deref() == Some("rebound") && st(&at(&field(&o, "binding"), &["rebound_from", "anchor"])).as_deref() == Some(a40.as_str()), || {
        format!("★a different target must rebind LOUDLY (rebound_from), got {}", rep(&o))
    })?;
    let o = down_init("lite", "hint_map_only", &[]);
    let rec_re = t.read_json("docs/_evidence/lite.json");
    need(o.0 == 0 && st(&at(&rec_re, &["promotion_target", "anchor"])).as_deref() == Some(b40.as_str()) && st(&at(&rec_re, &["promotion_target_provenance", "writer"])).as_deref() == Some("set-promotion-target"), || {
        format!("★re-init must carry the recorded promotion_target pair (silent loss), got {} {}", o.0, rec_re.repr())
    })?;
    let mpath = t.root.join("docs/_evidence/lite.work-manifest.json");
    need(!(mpath.is_file() && get(&t.read_json("docs/_evidence/lite.work-manifest.json"), "promotion_target").is_some()), || {
        "★set-promotion-target must not write the work-manifest itself (finalize is its writer)".into()
    })?;
    let o = t.invoke(&["finalize", "--repo-root", r, "--topic", "lite", "--pii-scan-json", &pii_l, "--runtime-json", &rt_l]);
    let man = t.read_json("docs/_evidence/lite.work-manifest.json");
    let want = obj(vec![("kind", s("hint")), ("tag", s(tag_x)), ("topology", s("single TP=1")), ("anchor", s(&b40))]);
    need(get(&man, "promotion_target").map(|p| py_eq(&p, &want)).unwrap_or(false), || {
        format!("finalize must emit the recorded promotion_target verbatim: {}", get(&man, "promotion_target").map(|v| v.repr()).unwrap_or("None".into()))
    })?;
    need(matches!(field(&o, "eligible_for_promotion").peel(), PyValue::Bool(true)), || format!("emitting promotion_target must not change the gate's verdict: {}", rep(&o)))?;
    let rec_hand = t.read_json("docs/_evidence/lite.json");
    let mut stripped = agent_runtime_support::pyvalue::PyDict::new();
    if let Some(d) = dict(&rec_hand) {
        for (k, v) in d.iter() {
            if !k.equals("promotion_target_provenance") {
                stripped.insert(k.clone(), v.clone());
            }
        }
    }
    t.write("docs/_evidence/lite.json", &dumps_ascii(&PyValue::Dict(stripped)));
    let o = t.invoke(&["finalize", "--repo-root", r, "--topic", "lite"]);
    need(o.0 == 2 && has(&o, "PUBLICATION_RECORD_PROMOTION_TARGET_INVALID"), || {
        format!("★a hand-appended promotion_target (no provenance) must be rejected at load, got {}", rep(&o))
    })?;
    // PII exemption coverage
    let o = down_init("piix", "full_benchmark", &["--benchmark-mode", "full", "--benchmark-verdict", "REFUTE"]);
    need(okv(&o), || format!("X5 fixture init failed: {}", rep(&o)))?;
    for kind in ["plan", "devlog", "testlog"] {
        let src = format!("docs/_evidence/inputs/piix_{kind}.md");
        t.write(&src, &format!("# {kind}\n면제 커버리지 픽스처 서사.\n"));
        let o = t.invoke(&["set-narrative", "--repo-root", r, "--topic", "piix", "--kind", kind, "--narrative-file", &src, "--author", "selftest", "--generated-utc", "2026-01-01T06:00:00Z"]);
        need(o.0 == 0, || format!("X5 fixture narrative {kind} failed: {}", rep(&o)))?;
    }
    t.write("docs/_evidence/inputs/measured.json", "{\"measurement_ok\": true}\n");
    let o = t.invoke(&["append-raw", "--repo-root", r, "--topic", "piix", "--kind", "simlog", "--src", "docs/_evidence/inputs/measured.json", "--recorded-utc", "2026-01-01T06:01:00Z", "--dest-name", "level_01_measured.json"]);
    need(o.0 == 0, || format!("X5 fixture simlog append failed: {}", rep(&o)))?;
    let o = t.invoke(&["publish-benchmark", "--repo-root", r, "--topic", "piix", "--verdict", "REFUTE", "--generated-utc", "2026-01-01T06:02:00Z", "--bench-report-src", &rep_rel]);
    need(o.0 == 0, || format!("X5 fixture publish-benchmark failed: {}", rep(&o)))?;
    let rec_x = t.read_json("docs/_evidence/piix.json");
    let rel_of = |k: &str| relpath(&format!("{r}/{}", st(&at(&rec_x, &["scaffolded", k])).unwrap_or_default()), &mdir);
    let mut scanned_x: Vec<String> = ["plan", "devlog", "testlog", "bench_report"].iter().map(|k| rel_of(k)).collect();
    scanned_x.sort();
    let simlog_x = rel_of("simlog");
    let raw_reason = gate::PII_MACHINE_RAW_EXEMPTION_REASON;
    let pii_finalize = |exempt: &[(&str, &str)], scanned: &[String]| -> Out {
        let ex = PyValue::List(exempt.iter().map(|(p, rr)| obj(vec![("path", s(p)), ("reason", s(rr))])).collect());
        t.write("pii_x.json", &dumps_ascii(&obj(vec![("passed", b(true)), ("scanned_paths", list_s(scanned)), ("exempt_paths", ex)])));
        t.invoke(&["finalize", "--repo-root", r, "--topic", "piix", "--pii-scan-json", &format!("{r}/pii_x.json"), "--runtime-json", &rt_l])
    };
    let o = pii_finalize(&[(&simlog_x, raw_reason)], &scanned_x);
    need(st(&field(&o, "state")).as_deref() == Some("evidence-complete") && !codes(&o).iter().any(|c| c.starts_with("PII_SCAN_COVERAGE_INCOMPLETE")), || {
        format!("a simlog recorded as machine-raw exempt must count as PII coverage: {}", rep(&o))
    })?;
    let o = pii_finalize(&[(&simlog_x, "귀찮아서")], &scanned_x);
    need(has(&o, "PII_SCAN_COVERAGE_INCOMPLETE:simlog"), || format!("★an exemption with any other reason must not count as coverage: {}", rep(&o)))?;
    let o = pii_finalize(&[], &scanned_x);
    need(has(&o, "PII_SCAN_COVERAGE_INCOMPLETE:simlog"), || format!("★an unscanned, unexempted simlog must stay a coverage gap: {}", rep(&o)))?;
    let testlog_x = rel_of("testlog");
    let without: Vec<String> = scanned_x.iter().filter(|p| **p != testlog_x).cloned().collect();
    let o = pii_finalize(&[(&simlog_x, raw_reason), (&testlog_x, raw_reason)], &without);
    need(has(&o, "PII_SCAN_COVERAGE_INCOMPLETE:testlog"), || format!("★human-authored prose (testlog) must never be exemptible: {}", rep(&o)))?;
    let abs = format!("/abs/{simlog_x}");
    let o = pii_finalize(&[(&abs, raw_reason)], &scanned_x);
    need(o.0 == 2 && has(&o, "ABSOLUTE_PII_EXEMPT_PATH:0"), || format!("★an absolute exempt path must be rejected like scanned_paths: {}", rep(&o)))?;
    // listdir on a closed descriptor fails closed
    let fd = crate::fsx::open_path(r, libc::O_RDONLY | libc::O_DIRECTORY).map_err(|e| format!("open: {e}"))?;
    let closed = fd.into_raw();
    unsafe { libc::close(closed) };
    let code = match crate::publisher::safe_listdir_names(closed, "SELFTEST") {
        Ok(_) => None,
        Err(e) => Some(e.code),
    };
    need(code == Some(2), || format!("listdir on a closed fd must fail closed with exit 2, got {}", code.map(|c| c.to_string()).unwrap_or("None".into())))?;
    let _ = Path::new("");
    Ok(())
}
