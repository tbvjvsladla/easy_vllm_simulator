//! template selftest — port of `scripts/hintlib/template.py` lines 4082–5721 (`selftest()` and its local fixtures).
//!
//! The real path (render → author → refresh → lint → seal → lint) runs in an isolated temp repository. Translations
//! (no Rust meaning / different mechanism, each kept as the closest Rust fact):
//! - Python copies `render_bench_section.py` into the temp repo and loads it (`_bench_module`) / runs its CLI `main`
//!   (`--verify --section`). The owner is compiled into this crate: the selftest calls
//!   `crate::render_bench_section::{parse_report, render, main}` in-process (stdout/stderr captured = Python's
//!   `redirect_stdout/stderr`); nothing is copied (the Rust linter never reads that file).
//! - Python passes callables (`substitute=lambda`, `resolve_stem=fixture_resolve`, `snapshot_origin=...`); Rust passes
//!   them through `TemplateHooks` to `lint_with` / `substituted_source_with`.
//! - An exception escaping a Python check aborts the selftest (traceback); here it ends the run with one
//!   `template: 예외 …` entry (never `[]`).
#![allow(non_snake_case, clippy::all)]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::*;
use crate::core::{self, HintError, HR};
use crate::pyre::{group, Re};
use crate::render_bench_section as rbs;
use crate::value::{loads, py_repr, Map, J};

// ── harness ──────────────────────────────────────────────────────────────────────────────────
struct St {
    bad: Vec<String>,
    ran: usize,
}
impl St {
    fn ck(&mut self, name: &str, cond: bool) {
        self.ran += 1;
        if !cond {
            self.bad.push(format!("template: {name}"));
        }
    }
}

/// Fixture text: `@<c><n>@` = char `c` repeated n times (Python `"a" * 40`); `@CUDA@` = the four-part CUDA version
/// (assembled so this tracked file holds no IPv4-shaped literal — the 2026-08-06 self-scan incident).
fn x(s: &str) -> String {
    let s = s.replace("@CUDA@", &["13.3", ".1.008"].concat());
    Re::new(r"@([a-z0-9])(\d+)@", 0).sub_fn(&s, |m| group(m, 1).unwrap_or("").repeat(group(m, 2).unwrap_or("0").parse().unwrap_or(0)))
}
fn j(s: &str) -> J {
    loads(&x(s)).unwrap_or_else(|e| panic!("fixture json: {e}: {s}"))
}
/// `json.loads(core.dumps(v))` (deep copy · keys sorted).
fn jc(v: &J) -> J {
    loads(&core::dumps(v)).expect("dumps round trip")
}
/// `dict(base, k=v, ...)` / `{**base, k: v}`.
fn with(base: &J, kv: &[(&str, J)]) -> J {
    let mut b = if base.is_dict() { base.clone() } else { J::dict() };
    for (k, v) in kv {
        b.set(*k, v.clone());
    }
    b
}
/// `{**base, **extra}`.
fn merge(base: &J, extra: &J) -> J {
    let mut b = if base.is_dict() { base.clone() } else { J::dict() };
    if let Some(m) = extra.as_dict() {
        for (k, v) in m.iter() {
            b.set(k, v.clone());
        }
    }
    b
}
/// Order-insensitive dict equality (Python `==`).
fn jeq(a: &J, b: &J) -> bool {
    core::dumps(a) == core::dumps(b)
}
/// Mutable path into a fixture (`v["a"][1]["b"]`, missing dict keys are created as null).
fn pm<'a>(v: &'a mut J, path: &[&str]) -> &'a mut J {
    let mut cur = v;
    for seg in path {
        cur = match cur {
            J::List(l) => {
                let i: usize = seg.parse().expect("index");
                &mut l[i]
            }
            J::Dict(m) => m.setdefault(seg, J::Null),
            _ => panic!("fixture path {seg}"),
        };
    }
    cur
}
fn pop(v: &mut J, k: &str) {
    if let Some(m) = v.as_dict_mut() {
        m.remove(k);
    }
}
fn io_e(e: std::io::Error, p: &Path) -> HintError {
    HintError::py("OSError", format!("{e}: {}", p.display()))
}
fn rt(p: &Path) -> HR<String> {
    std::fs::read_to_string(p).map_err(|e| io_e(e, p))
}
fn wt(p: &Path, s: &str) -> HR<()> {
    std::fs::write(p, s).map_err(|e| io_e(e, p))
}
fn mkdirs(p: &Path) -> HR<()> {
    std::fs::create_dir_all(p).map_err(|e| io_e(e, p))
}
/// `shutil.copytree(src, dst)` (dst must not exist · parents created).
fn copytree(src: &Path, dst: &Path) -> HR<()> {
    if dst.exists() {
        return Err(HintError::py("FileExistsError", format!("[Errno 17] File exists: {}", dst.display())));
    }
    mkdirs(dst)?;
    for e in std::fs::read_dir(src).map_err(|e| io_e(e, src))? {
        let e = e.map_err(|e| io_e(e, src))?;
        let p = e.path();
        let d = dst.join(e.file_name());
        if p.is_dir() {
            copytree(&p, &d)?;
        } else {
            std::fs::copy(&p, &d).map_err(|e| io_e(e, &p))?;
        }
    }
    Ok(())
}
fn codes(issues: &[J]) -> BTreeSet<String> {
    issues.iter().map(|i| i["code"].display()).collect()
}
fn raises<T>(r: HR<T>, code: &str) -> bool {
    matches!(r, Err(e) if e.code == code)
}
fn fb(md: &str, key: &str) -> HR<String> {
    let d = fact_blocks(md)?;
    match d.get(key) {
        Some(v) => Ok(v.display()),
        None => Err(HintError::py("KeyError", py_repr(key))),
    }
}
fn hd(v: &[J], n: usize) -> String {
    J::List(v.iter().take(n).cloned().collect()).repr()
}
fn set_repr(s: &BTreeSet<String>) -> String {
    J::List(s.iter().map(J::from).collect()).repr()
}
fn tp<'a>(t: &'a [(String, _Doc)], name: &str) -> &'a _Doc {
    &t.iter().find(|(n, _)| n == name).expect("template").1
}
fn prompt_of<'a>(t: &'a [(String, _Doc)], name: &str, ch: &str) -> Option<&'a _Prompt> {
    tp(t, name).prompts.iter().rev().find(|p| p.chapter == ch)
}
fn fields_of(t: &[(String, _Doc)], name: &str, ch: &str) -> J {
    prompt_of(t, name, ch).map(|p| p.fields.clone()).unwrap_or(J::Null)
}
fn count(h: &str, n: &str) -> usize {
    h.matches(n).count()
}
fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn pstem(p: &str) -> String {
    Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
/// `edit(name, old, new)` — replace the first occurrence (missing anchor = RuntimeError).
fn edit(p: &Path, name: &str, old: &str, new: &str) -> HR<()> {
    let f = p.join(name);
    let t = rt(&f)?;
    if !t.contains(old) {
        return Err(HintError::py("RuntimeError", format!("변형 앵커 없음: {name}: {}", py_repr(&core::head_chars(old, 40)))));
    }
    wt(&f, &t.replacen(old, new, 1))
}
type Mut<'a> = Box<dyn Fn(&Path) -> HR<()> + 'a>;
fn ed(name: &str, old: &str, new: &str) -> Mut<'static> {
    let (n, o, w) = (name.to_string(), old.to_string(), new.to_string());
    Box::new(move |p: &Path| edit(p, &n, &o, &w))
}
fn noop() -> Mut<'static> {
    Box::new(|_p: &Path| Ok(()))
}

type Sub<'a> = &'a dyn Fn(&str) -> HR<String>;
type Res<'a> = &'a dyn Fn(&J, &str) -> HR<Option<String>>;
type Org<'a> = &'a dyn Fn(&str, &str) -> HR<Option<Vec<u8>>>;

/// The keyword arguments of a Python `lint(...)` call.
#[derive(Clone)]
struct LA<'a> {
    lineage: J,
    subst_table: J,
    manifest: J,
    bench_report: Option<PathBuf>,
    facts: J,
    sealed: bool,
    historical: bool,
    pii_terms: Option<Option<Vec<String>>>,
    draft_dir: Option<PathBuf>,
    hooks: TemplateHooks<'a>,
}
impl<'a> LA<'a> {
    fn f(mut self, facts: &J) -> Self {
        self.facts = facts.clone();
        self
    }
    fn lin(mut self, l: &J) -> Self {
        self.lineage = l.clone();
        self
    }
    fn man(mut self, m: J) -> Self {
        self.manifest = m;
        self
    }
    fn br(mut self, b: Option<PathBuf>) -> Self {
        self.bench_report = b;
        self
    }
    fn sealed(mut self) -> Self {
        self.sealed = true;
        self
    }
    fn hist(mut self) -> Self {
        self.historical = true;
        self
    }
    fn sub(mut self, s: Option<Sub<'a>>) -> Self {
        self.hooks.substitute = s;
        self
    }
    fn st(mut self, t: J) -> Self {
        self.subst_table = t;
        self
    }
    fn org(mut self, o: Option<Org<'a>>) -> Self {
        self.hooks.snapshot_origin = o;
        self
    }
    fn draft(mut self, d: &Path) -> Self {
        self.draft_dir = Some(d.to_path_buf());
        self
    }
    fn pii(mut self, p: Option<Option<Vec<String>>>) -> Self {
        self.pii_terms = p;
        self
    }
    fn run(&self, repo: &Path, p: &Path) -> HR<Vec<J>> {
        lint_with(
            repo,
            p,
            &self.lineage,
            &self.subst_table,
            &self.manifest,
            self.bench_report.as_deref(),
            &self.facts,
            self.sealed,
            self.historical,
            self.pii_terms.as_ref().map(|o| o.as_deref()),
            self.draft_dir.as_deref(),
            &self.hooks,
        )
    }
    fn codes(&self, repo: &Path, p: &Path) -> HR<BTreeSet<String>> {
        Ok(codes(&self.run(repo, p)?))
    }
}

fn ctx<'a>(repo: &'a Path, facts: &'a J, tpls: &'a [(String, _Doc)]) -> _Ctx<'a> {
    _Ctx { repo, facts, templates: tpls }
}
/// `renderer(_Ctx(repo, facts, tpls))`.
fn rf(r: FactRenderer, repo: &Path, facts: &J, tpls: &[(String, _Doc)]) -> HR<String> {
    r(&ctx(repo, facts, tpls))
}

/// `bench.main(["--verify", "--section", <03>, "--report", <report>])` with stdout/stderr discarded.
fn cli_verify(p: &Path, report: &Path) -> i32 {
    let argv = strs(&["--verify", "--section", &p.join("03-benchmark.md").to_string_lossy(), "--report", &report.to_string_lossy()]);
    crate::io::capture(|| rbs::main(&argv)).0
}

fn vs_block(i: usize, c: &J, status: &str) -> String {
    let mut val = c["value"].display();
    if val.chars().any(|ch| ",:#\"".contains(ch)) {
        val = format!("'{val}'");
    }
    format!(
        "```hint-event\nid: V{i}\nkind: value-status\n노브: {}\n값: {val}\n지위: {status}\n근거: 픽스처 근거\n출처: [plan_26090918 §2]\n```",
        c["knob"].display()
    )
}

const LOG_P: &str = "output/multi/benchlog/serve_fail_fixture/master_failure.log";

fn answers(fx: &J) -> HR<Vec<((&'static str, &'static str), Option<String>)>> {
    let multi = _topology(fx)?.as_deref() == Some("multi");
    let vs: Vec<String> = _candidates(fx)?.iter().enumerate().map(|(i, c)| vs_block(i + 1, c, &c["candidate_status"].display())).collect();
    let vs = vs.join("\n\n");
    let s = |t: &str| Some(t.to_string());
    Ok(vec![
        (("00-hint", "0.1"), s("fixture-model 을 GB10 1g2n cluster(Ray TP=2)·v0.29.0rc6 로 서빙했다 — 동시성 1 decode 38.81 t/s.\n\
가장 비싼 벽은 CUTLASS FP8 MoE 가드 충돌(W1)이었고 moe-backend 를 비워 넘었다.\n\
KV 21474836480 바이트는 필요량 대비 과잉인 음성대조값(V2)이다 — 복사하지 마라.")),
        (("00-hint", "0.2"), s("통합메모리라 CPU offload 가 메모리를 거두지 못한다 — 근거 plan_26090918 §1.")),
        (("00-hint", "0.4"), s("source-build 를 골랐다 — 근거 testlog_26090921 §2. 재현 좌표는 v0.29.0rc6 이다.")),
        (("00-hint", "0.5"), s("① KV 는 재도출한다. ② moe-backend 는 명시하지 않는다(W1). ③ max-model-len 은 승계값이다.")),
        (("00-hint", "0.6"), s("1. 상위 버전에서 패치 필요 여부 → 트립와이어 로그 확인 → 그대로/폐기.\n빌드 문제 → upstream-version-watch.")),
        (("00-hint", "0.9"), s("빈 꼬리다 — 같은 q·len·kv 의 다른 셀이 없어 가르는 노브가 없다(inputs/tail.json = []).")),
        (("01-artifacts", "1.2"), s("### 60-fixture-patch.sh\n재구성 결과라 적용 미관측이다. 헤더 원문:\n\n\
> [원문] 60-fixture-patch.sh §L2-2\n> # fixture patch: PLE 게이트 완화")),
        (("01-artifacts", "1.3"), Some(vs + "\n\n승계값과 음성대조값을 복사하지 마라.")),
        (("01-artifacts", "1.4"), s("빌드 뒤 health 200 과 추론 1회로 판정한다. 기동 소요는 미관측이다.")),
        (("01-artifacts", "1.5"), if multi { s("slave 는 트리플렛을 받지 않는 Ray worker 다.") } else { None }),
        (("01-artifacts", "1.6"), s("### 가중치 획득\n관리 NAS 모드다 — 다운로드 없음(근거 plan_26090918 §1).\n\n\
### 스테이징\n해당 없음 — PLE 없는 체크포인트(근거 plan_26090918 §2).\n\n\
### 네트워크 단계\n베이스 이미지 pull · pip wheel — 오프라인이면 미리 받아 둔다(추론 — 실행 검증 ✗).")),
        (("02-narrative", "2.1"), s("첫 plan 의 목표:\n\n> [원문] plan_26090918 §1\n> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.")),
        (("02-narrative", "2.2"), Some(format!("### W1 CUTLASS 가드 충돌\n백엔드를 명시하자 가드와 충돌했다.\n\n```hint-event\nid: W1\nkind: wall\n\
증상: 기동 중 MoE 백엔드 선택에서 실패\n\
서명: \"ValueError: vLLM CUTLASS FP8 MoE backend is disabled…configuration.\"\n\
원인: 명시한 백엔드가 가드와 충돌\n해소: moe-backend 미설정\n검증: testlog_26090921 §2\n\
전이등급: arch-scaled\n출처: [testlog_26090921 §2, 60-fixture-patch.sh 헤더]\n```\n\n\
> [원문] testlog_26090921 §2\n> 회차 3: ValueError: vLLM CUTLASS FP8 MoE backend is disabled for this configuration.\n\n\
### W2 tuple 속성 오류\n```hint-event\nid: W2\nkind: wall\n증상: mmap 로드 중 예외\n\
서명: \"AttributeError: 'tuple' object has no attribute 'start'\"\n원인: range 대신 tuple\n\
해소: rng[0]/[1]\n검증: 재기동 PASS\n전이등급: arch-invariant\n출처: [{LOG_P}]\n```"))),
        (("02-narrative", "2.3"), s("```hint-event\nid: R1\nkind: rejected\n시도: moe-backend triton 명시\n기각사유: 미지원\n\
출처: [testlog_26090921 §2]\n```")),
        (("02-narrative", "2.4"), s("> [원문] testlog_26091009 §2\n> 당시 판정: hang 은 모델 결함이다.\n\n\
```hint-event\nid: M1\nkind: misdiagnosis\n증상: hang\n서명: \"당시 판정: hang 은 모델 결함이다\"\n\
원인: 워치독을 못 봤다\n해소: 호스트 워치독 사살\n검증: testlog_26091009 §2\n전이등급: judgment\n\
현재지위: 반증\n출처: [testlog_26091009 §2]\n```")),
        (("02-narrative", "2.5"), s("(V2) KV 는 필요량의 배수로 뒀다:\n\n> [원문] plan_26090918 §2\n> KV 필요량 = 24KiB/token x 262144 = 6144MiB")),
        (("02-narrative", "2.6"), s("- moe-backend 를 명시하지 마라 — W1 — 비워 둔다.")),
        (("02-narrative", "2.7"), s("```hint-event\nid: Q1\nkind: open-question\n물음: cold TTFT 이상치\n현재상태: 미설명\n\
출처: [testlog_26090921 §3]\n```")),
        (("03-benchmark", "3.4"), s("> [원문] testlog_26090921 §3\n> vllm bench serve --max-concurrency 1 --num-prompts 16\n\n- 반복 1회.")),
        (("03-benchmark", "3.5"), s("- 이전 측정 · 비교 불가 — 조건 차이. 판정 등급: full(반복 1회 · 현행 정의 미충족).")),
    ])
}

/// `fill(payload, fx)` — replace every `<<AGENT:` line by the fixture answer of its chapter.
fn fill(payload: &Path, fx: &J) -> HR<()> {
    let ans = answers(fx)?;
    let docs = payload_docs(fx)?;
    for name in TEMPLATE_FILES {
        if !docs.contains(&format!("{name}.md")) {
            continue;
        }
        let p = payload.join(format!("{name}.md"));
        let doc = _Doc::parse(name, &rt(&p)?);
        let mut lines = doc.lines.clone();
        for i in (0..lines.len()).rev() {
            if lines[i].starts_with(AGENT_MARK) {
                let ch = doc.chapter_of[i].as_str();
                let body = ans.iter().find(|((n, c), _)| *n == name && *c == ch).and_then(|(_, b)| b.clone());
                let Some(body) = body else {
                    return Err(HintError::py("RuntimeError", format!("픽스처 답 없음: {name} §{ch}")));
                };
                lines.splice(i..i + 1, body.split('\n').map(str::to_string));
            }
        }
        wt(&p, &lines.join("\n"))?;
    }
    Ok(())
}

/// `selftest()` — `[]` = pass.
pub fn selftest() -> Vec<String> {
    let mut st = St { bad: Vec::new(), ran: 0 };
    if let Err(e) = body(&mut st) {
        st.bad.push(format!("template: 예외 {}: {}", e.code, e.message));
        return st.bad;
    }
    let ran = st.ran;
    st.ck(&format!("검사 전수 실행({ran}) — 중도 반환으로 시험이 조용히 줄지 않게"), ran >= 285);
    st.bad
}

fn pname(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn body(st: &mut St) -> HR<()> {
    // ★ 하위키 출처(2026-10-17 · plan_26101622 granite 사실 검증 F4)
    {
        let mf = j(r#"{"measurement": {"lite": {"kv_gib": "35.0", "gen_tps": "38.1"},
            "sources": {"lite": "sweep_index.lite(x)", "lite.kv_gib": "sweep_index.lite(x) — 선언 클램프 되읽기(측정 아님)"}}}"#);
        let mt = _measurement_table(&ctx(Path::new("."), &mf, &[]))?.join("\n");
        let kv = mt.split('\n').find(|ln| ln.contains("`lite.kv_gib`")).unwrap_or("").to_string();
        let gt = mt.split('\n').find(|ln| ln.contains("`lite.gen_tps`")).unwrap_or("").to_string();
        st.ck("★하위키 출처는 그 행에만(kv_gib = 측정 아님 · gen_tps = 묶음 출처)", kv.contains("측정 아님") && !gt.contains("측정 아님")
            && gt.contains("sweep_index.lite(x)"));
    }

    // 제한 YAML 파서 단위(★음성대조 포함)
    let ok_md = "```hint-event\nid: W1   # 주석\nkind: wall\n증상: 로드 실패\n서명: \"Err: a # b, c\"\n원인: 원인\n해소: 해소\n\
검증: doc §1\n전이등급: arch-scaled\n출처: [doc_a §1, 'x, y.sh']\n```\n";
    let evs = parse_hint_events(ok_md)?;
    st.ck("YAML 파싱", evs.len() == 1 && evs[0]["id"] == J::from("W1") && evs[0]["서명"] == J::from("Err: a # b, c")
        && jeq(&evs[0]["출처"], &j(r#"["doc_a §1", "x, y.sh"]"#)) && evs[0]["_line"] == J::Int(1));
    st.ck("★YAML 중첩 줄 차단", raises(parse_hint_events(&ok_md.replace("원인: 원인", "원인:\n  - 중첩")), "HINT_EVENT_INVALID"));
    st.ck("★YAML 매핑 값 차단", raises(parse_hint_events(&ok_md.replace("원인: 원인", "원인: {a: 1}")), "HINT_EVENT_INVALID"));
    st.ck("★YAML 중복 키 차단", raises(parse_hint_events(&ok_md.replace("원인: 원인", "원인: 원인\n원인: 둘")), "HINT_EVENT_INVALID"));
    st.ck("★YAML 닫히지 않은 따옴표", raises(parse_hint_events(&ok_md.replace("\"Err: a # b, c\"", "\"Err")), "HINT_EVENT_INVALID"));
    st.ck("★필수 필드 부재(파서)", raises(parse_hint_events(&ok_md.replace("원인: 원인\n", "")), "HINT_EVENT_FIELD_MISSING"));
    st.ck("★kind 어휘 밖", raises(parse_hint_events(&ok_md.replace("kind: wall", "kind: wal")), "HINT_EVENT_KIND_UNKNOWN"));
    st.ck("★전이등급 어휘 밖", raises(parse_hint_events(&ok_md.replace("arch-scaled", "scaled")), "HINT_EVENT_VOCAB"));
    st.ck("★id 모양", raises(parse_hint_events(&ok_md.replace("id: W1", "id: V1")), "HINT_EVENT_ID_SHAPE"));
    st.ck("주석 안 예시는 사건 아님", parse_hint_events("<!-- PROMPT\n질문: q\n```hint-event\nid: <x>\n```\n-->\n")?.is_empty());
    st.ck("`패치#60` 은 주석 아님", _parse_value("패치#60 적용")?.0 == J::from("패치#60 적용"));
    st.ck("결손 0 명시", _missing_block(&J::List(vec![]), &J::Null, &J::Null)?.contains("없음")
        && _missing_block(&j(r#"["HINT_X"]"#), &J::dict(), &J::Null)?.contains("미등록"));

    let td = core::TempDir::new("hintlib-template-st-");
    let root = td.path().to_path_buf();
    let repo = root.join("repo");
    mkdirs(&repo.join(core::REL_SKILL))?;
    copytree(&core::templates_dir(), &repo.join(core::REL_SKILL).join("templates"))?;
    // Python also copies render_bench_section.py here; the Rust owner is in-process (module header).
    mkdirs(&repo.join(core::REL_SCRIPTS))?;
    let term = "zq-fixture-operator";
    mkdirs(&repo.join(".claude"))?;
    wt(&repo.join(core::REL_PII_TERMS), &format!("# fixture — 실물 pii_terms 를 복사하지 않는다\n{term}\n"))?;

    // 템플릿 트립와이어
    let tpls = _load_templates(&repo)?;
    let tpls: &[(String, _Doc)] = &tpls;
    let t00 = tp(tpls, "00-hint");
    let vis: Vec<&str> = t00.lines.iter().enumerate().filter(|(i, _)| !t00.masked[*i]).map(|(_, l)| l.as_str()).collect();
    let vis = vis.join("\n");
    st.ck("템플릿 00 에 배너 5줄(PROMPT 밖)", BANNER_LINES.iter().all(|b| vis.contains(b)));
    st.ck("템플릿 02 §2.2 wall>=1", jeq(&_directives(&fields_of(tpls, "02-narrative", "2.2"))?["blocks"], &j(r#"{"wall": 1}"#)));
    st.ck("템플릿 01 §1.5 조건", _directives(&fields_of(tpls, "01-artifacts", "1.5"))?["condition"] == j(r#"["topology", "multi"]"#));
    st.ck("템플릿 PROMPT 수(00 = 0.9 이름 꼬리 포함 · plan_26092908 §4.1 · 01 = 1.6 기동 전 준비 포함 R-e)",
        TEMPLATE_FILES.iter().map(|n| tp(tpls, n).prompts.len()).collect::<Vec<_>>() == vec![7, 5, 7, 2]);
    for n in TEMPLATE_FILES {
        let d = tp(tpls, n);
        let sc = &d.selfchecks;
        let body = if sc.len() == 1 { d.lines[sc[0].0..=sc[0].1].join("\n") } else { String::new() };
        let miss: Vec<String> = FACTCHECK_CLASSES()
            .as_dict()
            .map(|m| m.iter().filter(|(k, lab)| !body.contains(&format!("{k}. {}", lab.display()))).map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default();
        st.ck(&format!("템플릿 {n} SELFCHECK 1개 · preamble · 7부류 문구(빠짐 {})", J::from(miss.clone()).repr()), sc.len() == 1
            && d.chapter_of[sc[0].0] == "preamble" && miss.is_empty());
    }
    st.ck("템플릿 02 §2.4 필수 발췌 1(PROMPT 와 린터가 같은 규칙)",
        _directives(&fields_of(tpls, "02-narrative", "2.4"))?["excerpts"] == J::Int(1));
    st.ck("★템플릿 0.5 배너가 declared-requirement 를 '복사 금지' 무리에 넣지 않는다",
        tp(tpls, "00-hint").lines.iter().any(|ln| ln.contains("`tuned` · `declared-requirement` 가 아닌 값은")));
    let t03t = &tp(tpls, "03-benchmark").text;
    st.ck("★템플릿 3.5 비교 규칙 = 인증서 소프트 지문 블록(옛 digest/NGC/torch/CUDA 목록 ✗)", t03t.split('\n').any(|ln|
        ln.contains("kv_cache_memory_bytes") && ln.contains("gpu_memory_utilization") && ln.contains("moe_backend"))
        && !t03t.contains("00 §0.4 의 이미지 digest · NGC · torch · CUDA"));
    st.ck("★템플릿 어디에도 절 자리표시(§3.x) 없음", !TEMPLATE_FILES.iter().any(|n| _FACT_PLACEHOLDER().is_search(&tp(tpls, n).text)));
    st.ck("템플릿 03 부하 곡선 = 닫힘 표지 없는 영역 하나(§3.2)",
        tp(tpls, "03-benchmark").facts.iter().filter(|f| f.open_ended).map(|f| (f.fid.clone(), f.chapter.clone())).collect::<Vec<_>>()
            == vec![(BENCH_REGION_ID.to_string(), "3.2".to_string())]
        && !t03t.contains("FACT:bench_section"));
    let mcodes = crate::evidence::MISSING_CODES();
    st.ck("03 측정 결손 코드 ⊆ evidence.MISSING_CODES(tripwire)", BENCH_MISSING_CODES.iter().all(|c| mcodes.get(c).is_some())
        && _TRIPLET_ABSENT_CODES.iter().all(|c| mcodes.get(c).is_some()));

    // 계보 픽스처(격리 · IPv4 모양 리터럴 없음)
    let plan_p = "docs/plan/plan_26090918_fixture_딥캠페인.md";
    let tl_p = "docs/testlog/testlog_26090921_fixture_스모크_판정.md";
    let tl2_p = "docs/testlog/testlog_26091009_fixture_24셀_판정.md";
    let log_p = LOG_P;
    let rep_p = "docs/benchmark/bench_report_26091304_fixture_GB10_0.29.0.md";
    let long_rows: String = (1..46).map(|k| format!("줄 {k:02} 관측값 기록\n")).collect();
    let rep_body = "# bench report\n\n| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |\n\
|---|---|---|---|---|---|---|\n| 1 ★판정점 | 38.81 | 38.5 | 200.1 | 353.2 | 25.1 | 16/0 |\n\
| 2 | 60.25 | 59.9 | 310.4 | 410.0 | 30.2 | 16/0 |\n\n_절삭된 레벨 없음._\n";
    let files: Vec<(&str, String)> = vec![
        (plan_p, "# plan_26090918 — fixture 딥캠페인\n\n## 1. 목표\nGB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.\n\
노드당 메모리 총량은 124,610MiB 이고 인터커넥트는 RoCE 다.\n\n## 2. 예산\n\
floor = MemTotal - weights - kv - overhead >= 11264\nKV 필요량 = 24KiB/token x 262144 = 6144MiB\n".to_string()),
        (tl_p, format!("# testlog_26090921\n\n## 2. 회차\n회차 3: ValueError: vLLM CUTLASS FP8 MoE backend is disabled for this configuration.\n\
moe-backend 를 비우자 통과했다.\n\n## 3. 측정\nvllm bench serve --max-concurrency 1 --num-prompts 16\n\n## 4. 긴 표\n{long_rows}")),
        (tl2_p, "# testlog_26091009\n\n## 2. 오진\n당시 판정: hang 은 모델 결함이다.\n정정: 호스트 워치독이 사살했다.\n".to_string()),
        (log_p, "INFO start\n\x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready\r\n\
Loading 10%\rLoading 50%\rLoading 100%\nAttributeError: 'tuple' object has no attribute 'start'\n".to_string()),
        (rep_p, rep_body.to_string()),
    ];
    for (rel, body) in &files {
        let p = repo.join(rel);
        mkdirs(p.parent().unwrap())?;
        wt(&p, body)?;
    }
    let lineage = j(&format!(r#"{{"schema_version": 1,
        "documents": [{{"path": "{plan_p}", "kind": "plan", "date_key": "26090918", "depth": 1}},
                      {{"path": "{tl_p}", "kind": "testlog", "date_key": "26090921", "depth": 1}},
                      {{"path": "{tl2_p}", "kind": "testlog", "date_key": "26091009", "depth": 2}}],
        "evidence_candidates": [{{"path": "{log_p}", "kind": "engine_log", "sources": []}}]}}"#));

    let fixture_resolve = |lin: &J, stem: &str| -> HR<Option<String>> {
        let s = stem.trim();
        let mut paths: Vec<String> = lin["documents"].items().iter().map(|d| d["path"].display()).collect();
        paths.extend(lin["evidence_candidates"].items().iter().map(|c| c["path"].display()));
        if paths.iter().any(|p| p == s) {
            return Ok(Some(s.to_string()));
        }
        let pre = format!("{s}_");
        let hit: Vec<&String> = paths.iter().filter(|p| pstem(p) == s || pstem(p).starts_with(&pre)).collect();
        Ok(if hit.len() == 1 { Some(hit[0].clone()) } else { None })
    };
    let ident = |t: &str| -> HR<String> { Ok(t.to_string()) };
    let fres: Res = &fixture_resolve;
    let fsub: Sub = &ident;

    let bench_md = rbs::render(&rbs::parse_report(rep_body)?, &pname(rep_p))?;
    let mut base_facts = j(r##"{
        "tag": "hint/0.29.0rc6/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager",
        "generated_utc": "2026-09-21T10:21:01Z",
        "naming": {"grammar": "v6",
                   "segments": {"vllm": {"value": "0.29.0rc6", "source": "cell env VLLM_REF"},
                                "model": {"value": "fixture-model-nvfp4", "source": "checkpoint basename"}},
                   "axes": {"hw": {"value": "gb10", "source": "manifest gpu_model"}, "nodes": {"value": 2, "source": "manifest nodes"}},
                   "vllm_build_input": {"kind": "release", "ref": "v0.29.0rc6", "sha": "@a40@", "prev_release": null},
                   "vllm_observed": {"engine_self_report": "0.29.0", "wheel_meta": null, "source": "sweep_index.meta"}},
        "identity": {"model": "fixture-model-nvfp4", "gpu": "NVIDIA GB10", "vllm": "0.29.0", "quant": "nvfp4",
                     "topology": "multi", "tp": 2, "hf_repo": "Org/Fixture-Model-NVFP4", "base_model": "Org/Fixture-Model",
                     "base_slug": "fixture-model", "hf_revision": "@c40@",
                     "source": {"model": "certificate", "topology": "manifest", "quant": "naming-axis(q)",
                                "hf_revision": "checkpoint .git HEAD → refs/heads/main"}},
        "event_timeline": [
            {"utc": "2026-09-12T07:43:46Z", "node": "main", "kind": "budget_declare", "label": "smoke-fixture-cell",
             "detail": {"floor_mib": 40478}, "source": "docs/logs/main/events/2026-09.jsonl:727"},
            {"utc": "2026-09-12T08:14:38Z", "node": "main", "kind": "budget_clear", "label": "smoke-fixture-cell",
             "detail": null, "source": "docs/logs/main/events/2026-09.jsonl:728"},
            {"utc": "2026-09-12T18:54:37Z", "node": "main", "kind": "budget_declare", "label": "smoke-fixture-cell",
             "detail": {"floor_mib": 40478}, "source": "docs/logs/main/events/2026-09.jsonl:754"},
            {"utc": "2026-09-12T19:24:49Z", "node": "main", "kind": "budget_renew", "label": "smoke-fixture-cell",
             "detail": null, "source": "docs/logs/main/events/2026-09.jsonl:755"},
            {"utc": "2026-09-12T19:34:53Z", "node": "main", "kind": "budget_clear", "label": "smoke-fixture-cell",
             "detail": null, "source": "docs/logs/main/events/2026-09.jsonl:756"}],
        "bench_definition": {"current_full_definition": "full 계측(`lite ∪ GuideLLM × 반복 ≥3`)", "source": ".claude/rules/docs.md",
                             "this_repeats": 1, "repeats_source": "bench_report(fixture) 측정 구성 repeats",
                             "meets_current_full": false},
        "build": {"track": "source-build", "dockerfile": "Dockerfile.source-build", "image_tag": "easy-vllm:fixture",
                  "image_digest": "sha256:@b64@", "torch": "2.13.0a0+9186a08", "cuda": "@CUDA@", "ngc": "26.07",
                  "cpu_arch": "aarch64", "source": {"ngc": "docker image inspect", "torch": "docker image inspect"}},
        "plane": "docker",
        "applied_set": {"status": "unobservable", "source": "no-build-ledger",
                        "patches": [{"phase": "pre", "file": "60-fixture-patch.sh", "result": "unobserved",
                                     "reason": "원장 부재", "result_source": "none"},
                                    {"phase": "pre", "file": "50-skip-patch.sh", "result": "skipped",
                                     "reason": "SM12X_PORT=0", "result_source": "reconstructed(docker-history+gate)"}],
                        "reconstruction": {"source": "docker-history+gate-rule", "build_args": {"SM12X_PORT": "0"}}},
        "slots": {"build_patch_pre": {"files": ["artifacts/build_patch_pre/60-fixture-patch.sh"], "applicable": true,
                                      "rationale": "재구성", "evidence": {"kind": "reconstruction", "ref": "no-build-ledger"},
                                      "confidence": "2-signal"}},
        "qualification": {"health_200": true, "inference_observed": true, "sources": ["output/multi/benchlog/serve_proof_fixture.json"]},
        "measurement": {"measured_utc": "2026-09-12T19:32:52Z", "decode_tps_conc1": 38.81, "source": "sweep_fixture",
                        "levels": [{"level": 1, "status": "ok", "decode_tps": 38.81, "completed": 16},
                                   {"level": 2, "status": "ok", "decode_tps": 31.1, "completed": 16}],
                        "lite": {"gen_tps": 33.03, "cold_ttft_ms": 2156.6}},
        "measurement_config": {"bench_mode": "full", "repeats": 1, "source": "bench_report(fixture)"},
        "missing": {"HINT_MISSING_SLAVE_ATTESTATION": "서브 attestation 부재"},
        "lineage_reading_list": null,
        "value_status_candidates": [
            {"knob": "max-model-len", "value": "262144", "candidate_status": "inherited", "source": "serving-yaml", "comment": null},
            {"knob": "kv-cache-memory-bytes", "value": "21474836480", "candidate_status": "negative-control",
             "source": "serving-yaml", "comment": "A1 기준선 — 음성대조"},
            {"knob": "speculative-config", "value": "{\"method\": \"mtp\", \"num_speculative_tokens\": 3}",
             "candidate_status": "inherited", "source": "serving-yaml", "comment": null}],
        "reproduce_steps": [{"order": 1, "step": "render", "phase": "render",
                             "command": "python3 render.py --materialize-configs\npython3 render.py --cluster-envfile",
                             "command_source": "derived(렌더러 CLI)", "success": "러너 · env 생성", "duration": "미관측",
                             "source": "미관측"},
                            {"order": 2, "phase": "build", "command": "<owner smoke> fixture --build", "duration": "unobserved",
                             "success": "빌드 OK · 이미지 실재", "source": "cell build procedure"}],
        "sub_recipe": {"role_diff": {"master": "ray head + vllm serve", "slave": "ray worker"},
                       "launch_order": ["build", "master up", "slave up"],
                       "env_forward": [{"key": "VLLM_PLE_MMAP", "group": "serve_env", "from": "cell_env", "why": "worker 도 가중치를 올린다"}],
                       "image_identity_args": ["IMAGE_TAG"], "per_node_values": {"MASTER_HOST_IP": "<node:main>"},
                       "preconditions": ["가중치가 같은 경로에 실재"], "parity_attestation": {"observed": "unobservable"}},
        "bench_section_md": null,
        "task_class": "full_benchmark",
        "campaign": {"id": "camp-fixture", "cell": "fixture-cell", "node": "cluster", "mode": "campaign"},
        "approval": {"approved_by": "사용자(발화 전사) — 픽스처", "approved_utc": "2026-09-21T10:21:01Z", "source": "campaign:hint_targets"},
        "publication": {"topic": "camp_fixture", "manifest_ref": "docs/_evidence/camp_fixture.work-manifest.json",
                        "task_class": "full_benchmark"}
    }"##);
    let lrl: Vec<J> = lineage["documents"]
        .items()
        .iter()
        .map(|d| {
            crate::jd! {"path" => d["path"].clone(), "kind" => d["kind"].clone(), "date_key" => d["date_key"].clone(),
            "depth" => d["depth"].clone(), "axis_tokens" => crate::jl!["fixture-model"], "superseded_by" => J::List(vec![]),
            "tier" => "primary"}
        })
        .collect();
    base_facts.set("lineage_reading_list", J::List(lrl));
    base_facts.set("bench_section_md", bench_md.as_str());
    let base_facts = base_facts;

    let scaffold = |tag: &str, fx: &J| -> HR<PathBuf> {
        let payload = root.join(tag).join("payload");
        let art = payload.join("artifacts").join("build_patch_pre");
        mkdirs(&art)?;
        wt(&art.join("60-fixture-patch.sh"), "#!/bin/sh\n# fixture patch: PLE 게이트 완화\nexit 0\n")?;
        render_scaffold(&repo, fx, &payload, None)?;
        Ok(payload)
    };

    let facts_json = jc(&base_facts);
    let hooks = TemplateHooks { substitute: Some(fsub), source_text: None, resolve_stem: Some(fres), snapshot_origin: None };
    let kw = LA {
        lineage: lineage.clone(),
        subst_table: J::Null,
        manifest: j(r#"{"task_class": "full_benchmark"}"#),
        bench_report: Some(PathBuf::from(rep_p)),
        facts: J::Null,
        sealed: false,
        historical: false,
        pii_terms: None,
        draft_dir: None,
        hooks,
    };
    let all_docs = strs(&PAYLOAD_DOCS);

    // ── 기본 경로 ──
    let payload = scaffold("main", &base_facts)?;
    let ps = prompts(&payload)?;
    st.ck("PROMPT 목록(조건 성립 · 선택 포함 · 0.9 이름 꼬리 · 1.6 기동 전 준비)", ps.len() == 21 && ps.iter().all(|p| p["question"].truthy())
        && ps.iter().any(|p| p["section"] == J::from("0.9") && !p["optional"].truthy()));
    st.ck("스냅샷 기록", default_snapshot_path(&payload)?.is_file());
    st.ck("사실 블록 채움(0.4 관측 NGC)", fb(&rt(&payload.join("00-hint.md"))?, "resolved")?.contains("26.07"));
    st.ck("벤치 절 = 렌더 원문", fb(&rt(&payload.join("03-benchmark.md"))?, "bench_section")? == bench_md.trim_matches('\n'));
    let t01 = rt(&payload.join("01-artifacts.md"))?;
    st.ck("재현 표: 단계별 성공 판정 칸 · 여러 줄 명령은 줄 그대로(한 칸에 접지 않는다)",
        t01.contains("| 순서 | 단계 | 성공 판정 |") && t01.contains("\npython3 render.py --materialize-configs\npython3 render.py --cluster-envfile\n")
            && t01.contains("| 러너 · env 생성 |"));
    st.ck("값의 지위표: 후보 근거와 yaml 주석을 따로 싣는다", t01.contains("| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |")
        && t01.contains("A1 기준선 — 음성대조"));
    let t03 = rt(&payload.join("03-benchmark.md"))?;
    st.ck("측정 표: 레벨 목록은 별도 표 · dict 는 키.하위키 행(JSON 덩어리 ✗)",
        t03.contains("| `level` | `status` |") && t03.contains("| `lite.gen_tps` | 33.03 |") && !fb(&t03, "measurement")?.contains("{\""));
    st.ck("03 측정 결손: 없음도 적는다", fb(&t03, "bench_missing")?.contains("**측정 결손**: _없음"));
    let unfilled = kw.run(&repo, &payload)?;
    st.ck("★<<AGENT 잔존 차단", codes(&unfilled).contains("HINT_AGENT_PLACEHOLDER_RESIDUE") && codes(&unfilled).contains("HINT_SECTION_UNAUTHORED"));
    st.ck("★미저작 봉인 거부", raises(seal_prompts(&payload, &all_docs), "HINT_AGENT_PLACEHOLDER_RESIDUE"));
    st.ck("★brief 미저작", raises(brief(&payload), "HINT_BRIEF_ABSENT"));
    fill(&payload, &base_facts)?;
    let rr: BTreeSet<String> = refresh(&payload)?.into_iter().collect();
    st.ck("refresh 가 파생 블록을 채움", rr == DERIVED_FACTS.iter().map(|s| s.to_string()).collect());
    st.ck("refresh 멱등", refresh(&payload)?.is_empty());
    let clean = kw.run(&repo, &payload)?;
    st.ck(&format!("저작 뒤 lint 통과: {}", hd(&clean, 4)), clean.is_empty());
    st.ck("wall 지도 파생", fb(&rt(&payload.join("00-hint.md"))?, "wall_map")?.contains("W2"));
    let subs = subsections(&payload, "01-artifacts", "1.2")?;
    st.ck(&format!("subsections: §1.2 소제목 키 · 첫 문단(인용 앞에서 끊는다)({})", J::List(subs.clone()).repr()), jeq(&J::List(subs.clone()), &j(
        r#"[{"title": "60-fixture-patch.sh", "key": "60-fixture-patch.sh", "text": "재구성 결과라 적용 미관측이다. 헤더 원문:"}]"#)));
    st.ck("★subsections: 소제목 없는 절 = 빈 목록(hint-event 펜스 안 줄은 문단이 아니다)",
        subsections(&payload, "01-artifacts", "1.3")?.is_empty());
    let dflt = LA { hooks: TemplateHooks::default(), ..kw.clone() }.st(J::List(vec![])).run(&repo, &payload)?;
    st.ck(&format!("기본 해소(lineage.resolve_stem · pii.substitute · pii_terms 파일): {}", hd(&dflt, 3)), dflt.is_empty());

    st.ck("CLI render_bench_section --verify --section 03-benchmark.md = PASS", cli_verify(&payload, &repo.join(rep_p)) == 0);
    // ── 2026-10-16 plan_26101614: 셀 등급(lite 03 미발행 · 표시 라벨) · 데이터셋 출처 절 ──
    let t03d = rt(&payload.join("03-benchmark.md"))?;
    let dsp = fb(&t03d, "dataset_provenance")?;
    st.ck("full 셀: 03 §3.3 FACT:dataset_provenance = `###` 소제목(`## ` ✗) · facts 부재면 미관측 문구",
        dsp.starts_with("### 데이터셋 출처") && !dsp.contains("\n## ") && dsp.contains("미관측")
            && tp(tpls, "03-benchmark").facts.iter().filter(|f| f.fid == "dataset_provenance").map(|f| f.chapter.clone()).collect::<Vec<_>>()
                == vec!["3.3".to_string()]);
    let t00p = rt(&payload.join("00-hint.md"))?;
    st.ck("full 셀: 00 FACT:lite_summary 는 빈 블록 · 셀 등급 행 = full · README 에 03 행",
        fb(&t00p, "lite_summary")?.is_empty() && fb(&t00p, "grade")?.contains("셀 등급(cell_grade)")
            && rt(&payload.join(README_NAME))?.contains(&format!("| 4 | `{BENCH_DOC}` |")));
    let ds_c = with(&facts_json, &[("dataset_provenance", j(r#"{
        "dataset_kind": "parquet_file", "dataset_id": "abisee/cnn_dailymail", "dataset_revision": "@a40@",
        "dataset_provenance": "measured(사이드카)", "status": "recorded", "source": "bench_report(x.md) 측정 구성 표"}"#))]);
    let ds_i = with(&facts_json, &[("dataset_provenance", j(r#"{"dataset_kind": "synthetic", "status": "inferred-synthetic",
        "source": "inferred(부재 = synthetic)"}"#))]);
    let dpc = rf(_f_dataset_provenance, &repo, &ds_c, tpls)?;
    let dpi = rf(_f_dataset_provenance, &repo, &ds_i, tpls)?;
    st.ck("데이터셋 출처 렌더: 기재 = 표(키 10 + provenance · status · source) · 부재 추론 = '부재에서 추론' 머리줄(소급 ✗)",
        dpc.contains("| `dataset_id` | abisee/cnn_dailymail |") && dpc.contains("**기재한**")
            && dpi.contains("**부재에서 추론한**") && dpi.contains("| `dataset_seed` | 미기재 |"));
    let il = |s: &str| is_lite_cell(&j(s));
    st.ck("is_lite_cell: 영속 cell_grade 우선 · 인증서 benchmark_mode lite · 측정 구성 declared-lite · ★강등(downgraded-lite) 제외 · 모름 = full",
        il(r#"{"cell_grade": "lite"}"#)? && !il(r#"{"cell_grade": "full", "measurement": {"benchmark_mode": "lite"}}"#)?
            && il(r#"{"measurement": {"benchmark_mode": "lite"}}"#)?
            && il(r#"{"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite"}}"#)?
            && il(r#"{"measurement_config": {"bench_mode": "lite"}, "publication": {"task_class": "hint_map_only"}}"#)?
            && !il(r#"{"measurement": {"benchmark_mode": "lite"},
                     "measurement_config": {"bench_mode": "lite", "bench_mode_kind": "downgraded-lite"}}"#)?
            && !il(r#"{"measurement_config": {"bench_mode": "lite"}}"#)? && !il("{}")? && !is_lite_cell(&J::Null)?);
    st.ck("payload_docs: lite = 03 제외 · full = 4종", payload_docs(&j(r#"{"cell_grade": "lite"}"#))? == strs(&PAYLOAD_DOCS[..3])
        && payload_docs(&J::dict())? == all_docs);
    st.ck("조건 조각: 이 셀 등급만 남는다 · 다른 등급 조각은 걷힌다",
        _cond_text("a [[cell_grade=full: F]][[cell_grade=lite: L]] b", &j(r#"{"cell_grade": "lite"}"#))? == "a L b"
            && _cond_text("a [[cell_grade=full: F]][[cell_grade=lite: L]] b", &J::Null)? == "a F b");
    let lite_facts = jc(&with(&base_facts, &[
        ("task_class", J::from("hint_map_only")),
        ("cell_grade", J::from("lite")),
        ("bench_section_md", J::Null),
        ("publication", with(&base_facts["publication"], &[("task_class", J::from("hint_map_only"))])),
        ("measurement", j(&format!(r#"{{"measured_utc": "2026-09-12T19:32:52Z", "decode_tps_conc1": 38.81, "benchmark_mode": "lite",
                     "verdict": "{MAP_ONLY_MARKER}", "sources": {{"verdict": "lite-only(인증서 비발행)"}},
                     "lite": {{"gen_tps": 33.03, "cold_ttft_ms": 2156.6}}}}"#))),
        ("measurement_config", j(r#"{"bench_mode": "lite", "bench_mode_kind": "declared-lite", "repeats": 1,
                            "source": "bench_report(fixture)"}"#)),
    ]));
    let lpay = scaffold("lite", &lite_facts)?;
    let l00 = rt(&lpay.join("00-hint.md"))?;
    st.ck("★lite 셀 스캐폴드: 03-benchmark.md 를 쓰지 않는다 · 00 FACT:lite_summary = lite 수치 + 측정 구성 + 도구 원문",
        !lpay.join(BENCH_DOC).exists() && fb(&l00, "lite_summary")?.contains("38.81") && fb(&l00, "lite_summary")?.contains("**측정 구성**"));
    let g00 = fb(&l00, "grade")?;
    let lreadme = rt(&lpay.join(README_NAME))?;
    st.ck("lite 표시 라벨: 00 등급 · 판정 행 · 배너 = `lite(기동 가능)` + 성능 판정 아님(OBSERVATION-ONLY 표지 유지) · README 판정 줄 · 03 행 없음",
        g00.contains(&format!("`{LITE_LABEL}`")) && _verdict_text(&lite_facts)?.contains("성능 판정 아님")
            && g00.contains(&format!("**{LITE_LABEL}** · **{MAP_ONLY_MARKER}**"))
            && lreadme.contains(&format!("> **판정** `{LITE_LABEL}`")) && !lreadme.contains(&format!("| 4 | `{BENCH_DOC}` |"))
            && lreadme.contains("| — | `PAYLOAD.json`") && !lreadme.contains('\x00'));
    st.ck("lite 셀 PROMPT: full 조건 조각(03 결정론 표 · 03 §3.4)은 걷히고 lite 조각이 남는다",
        l00.contains("이 파일 머리의 lite 측정 블록(lite 셀 — 03 없음)") && !l00.contains("03-benchmark.md 의 결정론 표")
            && !rt(&lpay.join("01-artifacts.md"))?.contains("03 §3.4"));
    fill(&lpay, &lite_facts)?;
    refresh(&lpay)?;
    let lkw = kw.clone().man(j(r#"{"task_class": "hint_map_only"}"#)).br(None);
    let lclean = lkw.run(&repo, &lpay)?;
    st.ck(&format!("lite 셀 저작 뒤 lint 통과(03 부재 = 정상 · brief 수치는 00 lite 블록이 근거): {}", hd(&lclean, 3)), lclean.is_empty());
    let stray = root.join("lite-stray").join("payload");
    copytree(&lpay, &stray)?;
    wt(&stray.join(BENCH_DOC), "# 03\n")?;
    st.ck("★lite 셀에 03 이 있으면 HINT_DOCUMENT_UNEXPECTED", lkw.clone().f(&lite_facts).codes(&repo, &stray)?.contains("HINT_DOCUMENT_UNEXPECTED"));
    st.ck("★lite 페이로드를 기본 문서 목록으로 봉인 = 03 부재로 거부(조용히 건너뛰지 않는다)",
        raises(seal_prompts(&lpay, &all_docs), "HINT_DOCUMENT_ABSENT"));
    seal_prompts(&lpay, &payload_docs(&lite_facts)?)?;
    let lsealed = lkw.clone().sealed().run(&repo, &lpay)?;
    st.ck(&format!("lite 셀 봉인 뒤 lint 통과: {}", hd(&lsealed, 3)), lsealed.is_empty());
    st.ck("★full facts 로 lite 페이로드를 린트하면 03 부재 = HINT_DOCUMENT_ABSENT(모름 = full 쪽)",
        lkw.clone().f(&with(&lite_facts, &[("cell_grade", J::from("full"))])).codes(&repo, &lpay)?.contains("HINT_DOCUMENT_ABSENT"));

    let variant = |tag: &str, f: &dyn Fn(&Path) -> HR<()>| -> HR<PathBuf> {
        let dst = root.join("v").join(tag).join("payload");
        copytree(&payload, &dst)?;
        f(&dst)?;
        Ok(dst)
    };
    let kwf = kw.clone().f(&facts_json);
    let lint_v = |p: &Path| kwf.codes(&repo, p);

    st.ck("★PROMPT 잔존(봉인 검사)", kwf.clone().sealed().codes(&repo, &payload)?.contains("HINT_PROMPT_RESIDUE"));
    st.ck("★필수 필드 부재", lint_v(&variant("f", &ed("02-narrative.md", "원인: 명시한 백엔드가 가드와 충돌\n", ""))?)?.contains("HINT_EVENT_FIELD_MISSING"));
    let prep_body = "### 가중치 획득\n관리 NAS 모드다 — 다운로드 없음(근거 plan_26090918 §1).\n\n\
### 스테이징\n해당 없음 — PLE 없는 체크포인트(근거 plan_26090918 §2).\n\n\
### 네트워크 단계\n베이스 이미지 pull · pip wheel — 오프라인이면 미리 받아 둔다(추론 — 실행 검증 ✗).";
    let empty_prep: Vec<J> = kwf.run(&repo, &variant("prep", &ed("01-artifacts.md", prep_body, ""))?)?
        .into_iter()
        .filter(|i| i["code"] == J::from("HINT_SECTION_UNAUTHORED"))
        .collect();
    st.ck("★R-e 1.6 기동 전 준비 절이 비면 차단(HINT_SECTION_UNAUTHORED · §1.6) · 채우면 그 코드 없음",
        empty_prep.iter().any(|i| i["message"].display().contains("§1.6"))
            && !kwf.run(&repo, &payload)?.iter().any(|i| i["code"] == J::from("HINT_SECTION_UNAUTHORED") && i["message"].display().contains("§1.6")));
    let p16 = prompt_of(tpls, "01-artifacts", "1.6");
    st.ck("R-e 1.6 PROMPT 필수 필드 = 가중치 획득 경로 · 스테이징 생성 절차 · 네트워크 단계(명령 수준 · 출처) · 조건·선택 ✗",
        match p16 {
            Some(p) => {
                let ff = p.fields.get_str("필수 필드").unwrap_or("");
                ["가중치 획득 경로", "스테이징 생성 절차", "네트워크가 필요한 단계", "명령 수준", "출처"].iter().all(|w| ff.contains(w))
                    && !_directives(&p.fields)?["optional"].truthy() && _directives(&p.fields)?["condition"].is_null()
            }
            None => false,
        });
    st.ck("★발췌 1자 변조", lint_v(&variant("x", &ed("02-narrative.md", "> GB10 두 노드에서", "> GB11 두 노드에서"))?)?.contains("HINT_EXCERPT_MISMATCH"));
    let lin_wo_plan = with(&lineage, &[("documents", J::List(lineage["documents"].items().iter().filter(|d| d["path"] != J::from(plan_p)).cloned().collect()))]);
    st.ck("★LINEAGE 밖 발췌", kwf.clone().lin(&lin_wo_plan).codes(&repo, &payload)?.contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    st.ck("★LINEAGE 밖 출처", lint_v(&variant("s", &ed("02-narrative.md", "출처: [testlog_26091009 §2]", "출처: [testlog_29999999 §2]"))?)?
        .contains("HINT_EVENT_SOURCE_OUTSIDE_LINEAGE"));
    let v1 = vs_block(1, base_facts["value_status_candidates"].at(0), "inherited");
    st.ck("★value-status 커버리지 결손", lint_v(&variant("g", &ed("01-artifacts.md", &v1, ""))?)?.contains("HINT_VALUE_STATUS_GAP"));
    st.ck("★value-status 값 불일치", lint_v(&variant("gv", &ed("01-artifacts.md", "값: 262144", "값: 262000"))?)?.contains("HINT_VALUE_STATUS_VALUE_MISMATCH"));
    st.ck("★OBSERVATION-ONLY 마커 부재(기준 부재 full 셀 — manifest 만 선언)", kwf.clone()
        .man(j(r#"{"task_class": "full_benchmark", "benchmark": {"verdict": "OBSERVATION-ONLY"}}"#)).codes(&repo, &payload)?
        .contains("HINT_OBSERVATION_ONLY_MARKER_MISSING"));
    st.ck("★배너 삭제", lint_v(&variant("b", &ed("00-hint.md", &format!("> {}\n", REVALIDATION_BANNERS[1]), ""))?)?.contains("HINT_BANNER_ABSENT"));
    st.ck("★배너를 주석 안으로", lint_v(&variant("bc", &ed("00-hint.md", &format!("> {}", REVALIDATION_BANNERS[1]),
        &format!("<!-- {} -->", REVALIDATION_BANNERS[1])))?)?.contains("HINT_BANNER_ABSENT"));

    let drop_walls = |p: &Path| -> HR<()> {
        let f = p.join("02-narrative.md");
        let t = Re::new(r"(?s)```hint-event\nid: W\d+\nkind: wall\n.*?```", 0).sub_literal(&rt(&f)?, "");
        wt(&f, &t)?;
        refresh(p)?;
        Ok(())
    };
    st.ck("★wall 블록 0", lint_v(&variant("w", &drop_walls)?)?.contains("HINT_JOURNEY_WALL_ABSENT"));
    let long_ex: String = std::iter::once("> [원문] testlog_26090921 §4\n".to_string())
        .chain((1..42).map(|k| format!("> 줄 {k:02} 관측값 기록\n")))
        .collect();
    st.ck("★발췌 40줄 상한", lint_v(&variant("l", &ed("02-narrative.md", "- moe-backend 를 명시하지 마라",
        &format!("{long_ex}\n- moe-backend 를 명시하지 마라")))?)?.contains("HINT_EXCERPT_SOURCE_LIMIT"));
    st.ck("★사실 블록 재저작(F7 · NGC)", lint_v(&variant("d", &ed("00-hint.md", "| 26.07 |", "| 26.05 |"))?)?.contains("HINT_FACT_DRIFT"));
    st.ck("★파생 블록 낡음", lint_v(&variant("st", &ed("02-narrative.md", "id: W2", "id: W3"))?)?.contains("HINT_DERIVED_BLOCK_STALE"));
    let bench_row = "| 1 ★판정점 | 38.81 |";
    let bn_codes = lint_v(&variant("bn", &ed("03-benchmark.md", bench_row, "| 1 ★판정점 | 38.18 |"))?)?;
    st.ck("★벤치 표 숫자 1개(부하 곡선 영역)", bn_codes.contains("HINT_FACT_DRIFT") && bn_codes.contains("HINT_BENCH_SECTION_DRIFT"));
    let tampered_rep = root.join("bench_report_26091304_fixture_GB10_0.29.0.md");
    wt(&tampered_rep, &rep_body.replace("38.81", "39.81"))?;
    st.ck("★벤치 원천 불일치", kwf.clone().br(Some(tampered_rep.clone())).codes(&repo, &payload)?.contains("HINT_BENCH_SECTION_DRIFT"));
    // An old immutable payload may retain its historical caption; source numbers still rerender exactly.
    let legacy_payload = variant("legacy-bench", &ed("03-benchmark.md", rbs::CURRENT_CAPTION, rbs::LEGACY_CAPTION))?;
    let legacy_facts = with(&facts_json, &[("bench_section_md", J::from(facts_json["bench_section_md"].display().replacen(rbs::CURRENT_CAPTION, rbs::LEGACY_CAPTION, 1)))]);
    let lkf = kw.clone().f(&legacy_facts);
    st.ck("historical sealed verify preserves old caption with identical source numbers",
        !lkf.clone().sealed().hist().codes(&repo, &legacy_payload)?.contains("HINT_BENCH_SECTION_DRIFT"));
    st.ck("fresh draft and new sealing reject old caption",
        lkf.codes(&repo, &legacy_payload)?.contains("HINT_BENCH_SECTION_DRIFT")
            && lkf.clone().sealed().codes(&repo, &legacy_payload)?.contains("HINT_BENCH_SECTION_DRIFT"));
    let legacy_bad = root.join("v").join("legacy-bad").join("payload");
    copytree(&legacy_payload, &legacy_bad)?;
    edit(&legacy_bad, "03-benchmark.md", bench_row, "| 1 ★판정점 | 38.18 |")?;
    st.ck("historical caption cannot hide numeric tampering",
        lkf.clone().sealed().hist().codes(&repo, &legacy_bad)?.contains("HINT_BENCH_SECTION_DRIFT"));
    let legacy_text = root.join("v").join("legacy-text").join("payload");
    copytree(&legacy_payload, &legacy_text)?;
    edit(&legacy_text, "03-benchmark.md", rbs::LEGACY_CAPTION, &format!("{} EXTRA", rbs::LEGACY_CAPTION))?;
    st.ck("historical caption cannot hide extra text",
        lkf.clone().sealed().hist().codes(&repo, &legacy_text)?.contains("HINT_BENCH_SECTION_DRIFT"));
    seal_prompts(&legacy_payload, &all_docs)?;
    let old_sealed_issues = lkf.clone().sealed().hist().run(&repo, &legacy_payload)?;
    st.ck(&format!("historical sealed payload with unchanged benchmark source passes: {}", hd(&old_sealed_issues, 3)),
        old_sealed_issues.is_empty());
    st.ck("★벤치 원천 부재", kwf.clone().br(None).codes(&repo, &payload)?.contains("HINT_BENCH_SECTION_UNVERIFIABLE"));
    let no_table = root.join("bench_report_26091304_notable_GB10_0.29.0.md");
    wt(&no_table, "# bench report\n\n표가 없다.\n")?;
    st.ck("★벤치 원천에 표 없음(빈 표로 접지 않는다)", kwf.clone().br(Some(no_table.clone())).codes(&repo, &payload)?.contains("HINT_BENCH_SECTION_UNVERIFIABLE"));
    let m_txt = fb(&rt(&payload.join("03-benchmark.md"))?, "measurement")?;
    st.ck("측정값 원문 그대로(반올림 ✗ · 재계산 ✗)", m_txt.contains("| `decode_tps_conc1` | 38.81 |") && m_txt.contains("| `measured_utc` | 2026-09-12T19:32:52Z |"));
    let exs = excerpts(&rt(&payload.join("02-narrative.md"))?)?;
    st.ck("excerpts() 공개 함수", exs.iter().take(2).map(|e| e["stem"].display()).collect::<Vec<_>>() == strs(&["plan_26090918", "testlog_26090921"])
        && !exs.is_empty() && exs[0]["line_count"] == J::Int(1) && exs[0]["chapter"] == J::from("2.1"));
    st.ck("★서명 1자 변조", lint_v(&variant("sg", &ed("02-narrative.md", "is disabled…", "is disabeld…"))?)?.contains("HINT_SIGNATURE_MISMATCH"));
    st.ck("★서명 조각 12자 미만(D-c)", lint_v(&variant("sgs", &ed("02-narrative.md", "서명: \"당시 판정: hang 은 모델 결함이다\"", "서명: \"hang\""))?)?
        .contains("HINT_SIGNATURE_TOO_SHORT"));
    st.ck("★서명 앞 조각만 짧아도 차단(…로 끼운 한두 글자 조각)", lint_v(&variant("sgs2", &ed("02-narrative.md", "is disabled…configuration.", "is disabled…on."))?)?
        .contains("HINT_SIGNATURE_TOO_SHORT"));
    st.ck("★발췌 §절이 출처 제목 줄과 다름(D-c)", lint_v(&variant("secx", &ed("02-narrative.md", "> [원문] plan_26090918 §1", "> [원문] plan_26090918 §9"))?)?
        .contains("HINT_EXCERPT_SECTION_UNKNOWN"));
    st.ck("발췌 §절 = 제목 줄 앞부분(번호만 · 번호+제목) 통과", !lint_v(&variant("secok", &ed("02-narrative.md", "> [원문] plan_26090918 §1",
        "> [원문] plan_26090918 §1. 목표"))?)?.contains("HINT_EXCERPT_SECTION_UNKNOWN"));
    st.ck("비-마크다운 출처(.sh 헤더)는 §절 대조 대상 밖", !_section_in_headings("헤더", "#!/bin/sh\n# x\n", &J::Null)?
        && !lint_v(&payload)?.contains("HINT_EXCERPT_SECTION_UNKNOWN"));
    st.ck("★코드 펜스 안 `#` 줄은 제목이 아니다", !_section_in_headings("x", "```\n# x\n```\n", &J::Null)?
        && _section_in_headings("x", "# x\n", &J::Null)?);
    // ── 2026-09-22 S2 round 2(F8): 출처 정규화 · 비-마크다운 `§L<a>-<b>` ──
    st.ck("normalize_source: CRLF · \\r 진행 조각 · ANSI · 줄 수 보존",
        normalize_source("a\r\nb 1%\rb 100%\n\x1b[36mc\x1b[0m\n")? == "a\nb 100%\nc\n" && normalize_source(&normalize_source("x\ry\n")?)? == "y\n");
    let osc = "a\x1b]0;title\nb\nc\x07d\n";
    let nosc = normalize_source(osc)?;
    st.ck("★닫히지 않은 OSC 가 줄바꿈을 삼키지 않는다(정규화 뒤 줄 수 = 원본 `\\n` 수)",
        count(&nosc, "\n") == count(osc, "\n") && nosc.split('\n').nth(1) == Some("b"));
    let log_txt = substituted_source_with(&repo, &payload, "master_failure", &lineage, &J::Null, None, &hooks)?;
    let log_rows: Vec<&str> = log_txt.split('\n').collect();
    st.ck(&format!("발췌 도우미 = 정규화된 원문(줄 번호 = \\n 개수 · ANSI ✗ · \\r 마지막 조각)({})",
        J::from(log_rows.iter().take(4).map(|s| s.to_string()).collect::<Vec<_>>()).repr()),
        log_rows.len() == 5 && log_rows[1] == "(RayWorkerProc pid=7) worker ready" && log_rows[2] == "Loading 100%" && !log_txt.contains('\x1b'));
    let log_q = "> Loading 100%\n> AttributeError: 'tuple' object has no attribute 'start'";
    let base_14 = "빌드 뒤 health 200 과 추론 1회로 판정한다. 기동 소요는 미관측이다.";
    let with_log_excerpt = |tag: &str, label: &str, quote: &str| -> HR<PathBuf> {
        variant(tag, &ed("01-artifacts.md", base_14, &format!("{base_14}\n\n> [원문] master_failure {label}\n{quote}")))
    };
    let ok_log = with_log_excerpt("lg", "§L3-4", log_q)?;
    st.ck(&format!("비-마크다운 출처 발췌 §L3-4 통과: {}", set_repr(&lint_v(&ok_log)?)), lint_v(&ok_log)?.is_empty());
    st.ck("★비-마크다운 출처에 제목식 표지(§엔진 로그) = HINT_EXCERPT_SECTION_SHAPE",
        lint_v(&with_log_excerpt("lg-shape", "§엔진 로그", log_q)?)?.contains("HINT_EXCERPT_SECTION_SHAPE"));
    st.ck("★줄 범위 밖의 인용(원문에는 있다) = HINT_EXCERPT_OUTSIDE_LINES",
        lint_v(&with_log_excerpt("lg-out", "§L1-2", log_q)?)?.contains("HINT_EXCERPT_OUTSIDE_LINES"));
    st.ck("★줄 범위가 원문 줄 수를 넘는다 = HINT_EXCERPT_LINES_OUT_OF_RANGE",
        lint_v(&with_log_excerpt("lg-oor", "§L3-9", log_q)?)?.contains("HINT_EXCERPT_LINES_OUT_OF_RANGE"));
    st.ck("★줄 범위가 인용보다 지나치게 넓다(인용 2줄 · 범위 5줄) = HINT_EXCERPT_LINES_TOO_WIDE",
        lint_v(&with_log_excerpt("lg-wide", "§L1-5", log_q)?)?.contains("HINT_EXCERPT_LINES_TOO_WIDE"));
    st.ck("★ANSI 제어열을 그대로 옮긴 인용 = HINT_EXCERPT_MISMATCH(정규화가 지운 바이트)", lint_v(&with_log_excerpt(
        "lg-ansi", "§L2-2", "> \x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready")?)?.contains("HINT_EXCERPT_MISMATCH"));
    st.ck("★`\\r` 로 덮인 진행 조각(Loading 50%)을 인용 = HINT_EXCERPT_MISMATCH", lint_v(&with_log_excerpt(
        "lg-cr", "§L3-3", "> Loading 50%")?)?.contains("HINT_EXCERPT_MISMATCH"));
    st.ck("artifacts(.sh) 발췌도 §L 표지: §L2-2 통과 · ★§헤더 = SECTION_SHAPE", !lint_v(&payload)?.contains("HINT_EXCERPT_SECTION_SHAPE")
        && lint_v(&variant("sh-hdr", &ed("01-artifacts.md", "> [원문] 60-fixture-patch.sh §L2-2", "> [원문] 60-fixture-patch.sh §헤더"))?)?
            .contains("HINT_EXCERPT_SECTION_SHAPE"));
    // ── 2026-09-22 S2 round 3: 측정 도구 스냅샷(draft 상대 `inputs/sources/<name>@<rev12>`)이 발췌 · 출처로 해소된다 ──
    let snap_name = "run_bench.sh@0123456789ab";
    let snap_rel = format!("{SNAPSHOT_DIR}/{snap_name}");
    let snap_body = "#!/bin/bash\nset -e\nvllm bench serve --random-range-ratio 0 --ignore-eos --temperature 0\n";
    let snap_rev = x("0123456789ab@c28@");
    let snap_path = ".claude/skills/adversarial-benchmark/scripts/run_bench.sh";
    let mut lin_snap = lineage.clone();
    pm(&mut lin_snap, &["evidence_candidates"]).push(crate::jd! {"path" => snap_rel.as_str(), "kind" => "tool-source@rev",
        "origin" => format!("git:{snap_rev}:{snap_path}")});
    let snap_q = "> vllm bench serve --random-range-ratio 0 --ignore-eos --temperature 0";
    let snap_origin = |rev: &str, path: &str| -> HR<Option<Vec<u8>>> {
        Ok(if rev == snap_rev && path == snap_path { Some(snap_body.as_bytes().to_vec()) } else { None })
    };
    let sorg: Org = &snap_origin;
    let kws = kwf.clone().lin(&lin_snap).org(Some(sorg));
    let lint_s = |p: &Path| kws.codes(&repo, p);
    let with_snapshot = |tag: &str, label: &str, token: &str, write: bool, quote: &str| -> HR<PathBuf> {
        let (label, token, quote) = (label.to_string(), token.to_string(), quote.to_string());
        let snap_rel = snap_rel.clone();
        variant(tag, &move |p: &Path| {
            if write {
                let par = p.parent().unwrap();
                mkdirs(&par.join(SNAPSHOT_DIR))?;
                wt(&par.join(&snap_rel), snap_body)?;
            }
            edit(p, "01-artifacts.md", base_14, &format!("{base_14}\n\n> [원문] {token} {label}\n{quote}"))
        })
    };
    let sn_ok = with_snapshot("sn-ok", "§L3-3", snap_name, true, snap_q)?;
    st.ck(&format!("R3 스냅샷 발췌(`<name>@<rev12>` §L3-3) 통과: {}", set_repr(&lint_s(&sn_ok)?)), lint_s(&sn_ok)?.is_empty());
    st.ck("R3 스냅샷 발췌: `inputs/sources/…` 경로 토큰도 같은 파일", lint_s(&with_snapshot("sn-path", "§L3-3", &snap_rel, true, snap_q)?)?.is_empty());
    let shooks = TemplateHooks { snapshot_origin: Some(sorg), ..hooks };
    st.ck("R3 발췌 도우미: 스냅샷 = draft 상대 바이트(저장소 파일 아님)",
        substituted_source_with(&repo, &sn_ok, snap_name, &lin_snap, &J::Null, None, &shooks)?.contains("--ignore-eos"));
    st.ck("★R3 LINEAGE 에 등재되지 않은 스냅샷 파일 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE(디스크에 있어도)",
        kws.clone().lin(&lineage).codes(&repo, &sn_ok)?.contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    st.ck("★R3 등재됐으나 파일이 없는 스냅샷 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE(저장소 · artifacts 로 흘리지 않는다)",
        lint_s(&with_snapshot("sn-nofile", "§L3-3", snap_name, false, snap_q)?)?.contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    st.ck("★R3 스냅샷 발췌 머리가 줄 범위가 아니다 = HINT_EXCERPT_SECTION_SHAPE",
        lint_s(&with_snapshot("sn-shape", "§헤더", snap_name, true, snap_q)?)?.contains("HINT_EXCERPT_SECTION_SHAPE"));
    st.ck("★R3 스냅샷 발췌 1자 변조 = HINT_EXCERPT_MISMATCH",
        lint_s(&with_snapshot("sn-mm", "§L3-3", snap_name, true, &snap_q.replace("--ignore-eos", "--ignore-eoz"))?)?.contains("HINT_EXCERPT_MISMATCH"));
    st.ck("★R3 다른 리비전 토큰(@rev 불일치) = 밖",
        lint_s(&with_snapshot("sn-rev", "§L3-3", "run_bench.sh@0123456789ac", true, snap_q)?)?.contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    let sn_tamper = with_snapshot("sn-tamper", "§L3-3", snap_name, true, &snap_q.replace("--temperature 0", "--temperature 1"))?;
    wt(&sn_tamper.parent().unwrap().join(&snap_rel), &snap_body.replace("--temperature 0", "--temperature 1"))?;
    let tam = lint_s(&sn_tamper)?;
    st.ck("★R3 스냅샷 파일 변조(발췌는 변조본과 일치) = HINT_EXCERPT_SNAPSHOT_DRIFT · 발췌 대조로는 통과했을 것",
        tam.contains("HINT_EXCERPT_SNAPSHOT_DRIFT") && !tam.contains("HINT_EXCERPT_MISMATCH")
            && raises(substituted_source_with(&repo, &sn_tamper, snap_name, &lin_snap, &J::Null, None, &shooks), "HINT_EXCERPT_SNAPSHOT_DRIFT"));
    let none_origin = |_r: &str, _p: &str| -> HR<Option<Vec<u8>>> { Ok(None) };
    st.ck("★R3 스냅샷 origin 을 읽을 수 없다(커밋 부재 · git 아님) = HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE(통과 ✗)",
        kws.clone().org(Some(&none_origin)).codes(&repo, &sn_ok)?.contains("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE"));
    let mut lin_snap_bad = lineage.clone();
    pm(&mut lin_snap_bad, &["evidence_candidates"]).push(crate::jd! {"path" => snap_rel.as_str(), "kind" => "tool-source@rev",
        "origin" => format!("git:{}:{snap_path}", &snap_rev[..12])});
    st.ck("★R3 스냅샷 origin 모양 결함(12자 rev) = HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE",
        kws.clone().lin(&lin_snap_bad).codes(&repo, &sn_ok)?.contains("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE"));
    let sn_copy = root.join("elsewhere").join("payload");
    copytree(&sn_ok, &sn_copy)?;
    st.ck("R3 스냅샷: 다른 자리의 페이로드 사본 + draft_dir = draft 면 통과", kws.clone().draft(sn_ok.parent().unwrap()).codes(&repo, &sn_copy)?.is_empty());
    st.ck("★R3 스냅샷: 사본만 검사하고 draft_dir 을 주지 않으면 밖(사본 옆에는 스냅샷이 없다 — 명시가 필요한 이유)",
        lint_s(&sn_copy)?.contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    // ── F9: brief 수치 · declared-requirement 근거 · 사실 블록 자리표시 · 결과 출처 어휘 ──
    st.ck("★brief 수치가 사실 블록에 없다(단위 환산 20480MiB ← 21474836480) = HINT_BRIEF_NUMBER_UNGROUNDED",
        lint_v(&variant("bn20480", &ed("00-hint.md", "KV 21474836480 바이트는", "KV 20480MiB 는"))?)?.contains("HINT_BRIEF_NUMBER_UNGROUNDED"));
    st.ck("★brief 수치 반올림(38.81 → 38.8) = HINT_BRIEF_NUMBER_UNGROUNDED",
        lint_v(&variant("bnround", &ed("00-hint.md", "decode 38.81 t/s", "decode 38.8 t/s"))?)?.contains("HINT_BRIEF_NUMBER_UNGROUNDED"));
    st.ck("brief 수치 대조: 약칭(262k) · 버전(v0.29.0rc6) · 한 자리 서수는 건너뛴다",
        _brief_numbers("262k · v0.29.0rc6 · TP=2 · W1 · 0.29.0 · 동시성 1")?.is_empty());
    st.ck("★brief 반올림 소수에 글자 단위를 붙여도(38.8t/s) = HINT_BRIEF_NUMBER_UNGROUNDED",
        _brief_numbers("decode 38.8t/s · 2.5x")? == strs(&["38.8", "2.5"])
            && lint_v(&variant("bnunit", &ed("00-hint.md", "decode 38.81 t/s", "decode 38.8t/s"))?)?.contains("HINT_BRIEF_NUMBER_UNGROUNDED"));
    let wall_only_number = |p: &Path| -> HR<()> {
        edit(p, "02-narrative.md", "증상: 기동 중 MoE 백엔드 선택에서 실패\n", "증상: 기동 중 MoE 백엔드 선택에서 실패(777.7 초)\n")?;
        edit(p, "00-hint.md", "decode 38.81 t/s", "decode 38.81 t/s · 벽 777.7 초")?;
        refresh(p)?;
        Ok(())
    };
    let bn_wall = variant("bnwall", &wall_only_number)?;
    st.ck("★brief 수치가 저작자 hint-event(파생 wall_map)에만 있다 = HINT_BRIEF_NUMBER_UNGROUNDED(저작 수치가 스스로를 근거 짓지 않는다)",
        fb(&rt(&bn_wall.join("00-hint.md"))?, "wall_map")?.contains("777.7") && lint_v(&bn_wall)?.contains("HINT_BRIEF_NUMBER_UNGROUNDED"));
    let dr_old = "지위: inherited\n근거: 픽스처 근거\n출처: [plan_26090918 §2]";
    let dr = "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED";
    st.ck("★declared-requirement 인데 W id · artifacts 출처가 없다 = HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED", lint_v(&variant("dr0", &ed(
        "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: yaml 주석 필수\n출처: [plan_26090918 §2]"))?)?.contains(dr));
    st.ck("★declared-requirement 의 W id 가 이 페이로드에 없는 벽(W9) = 근거 아님", lint_v(&variant("dr9", &ed(
        "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: W9 가 다시 난다\n출처: [plan_26090918 §2]"))?)?.contains(dr));
    st.ck("declared-requirement 근거 = W id(있는 벽) 통과", !lint_v(&variant("drw", &ed(
        "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: 빼면 W1 이 다시 난다\n출처: [plan_26090918 §2]"))?)?.contains(dr));
    st.ck("declared-requirement 근거 = 실패를 기록한 artifacts 파일 출처 통과(계보 밖 기록)", !lint_v(&variant("dra", &ed(
        "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: 패치 헤더가 실패를 적었다\n출처: [60-fixture-patch.sh §L2-2]"))?)?.contains(dr));
    let with_triplet = |p: &Path| -> HR<()> {
        mkdirs(&p.join("artifacts").join("triplet"))?;
        wt(&p.join("artifacts").join("triplet").join("fixture-cell.yaml"), "async-scheduling: false        # 명시 필수\n")?;
        edit(p, "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: yaml 주석이 명시 필수라 적었다\n출처: [fixture-cell.yaml §L1-1]")
    };
    st.ck("★declared-requirement 의 출처가 트리플렛(값을 선언한 yaml 자신)뿐 = HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED",
        lint_v(&variant("drt", &with_triplet)?)?.contains(dr));
    let mut ph_facts = jc(&facts_json);
    *pm(&mut ph_facts, &["reproduce_steps", "1", "command"]) = J::from("adversarial-benchmark 셀 스윕 — 명령 원문은 03-benchmark.md §3.x");
    st.ck("★사실 블록의 절 자리표시(§3.x) = HINT_FACT_PLACEHOLDER", kw.clone().f(&ph_facts).codes(&repo, &payload)?.contains("HINT_FACT_PLACEHOLDER"));
    let mut rs_facts = jc(&facts_json);
    *pm(&mut rs_facts, &["applied_set", "patches", "0", "result_source"]) = J::from("guessed");
    st.ck("★결과 출처 어휘 밖 = HINT_RESULT_SOURCE_UNKNOWN", kw.clone().f(&rs_facts).codes(&repo, &payload)?.contains("HINT_RESULT_SOURCE_UNKNOWN"));
    let t00 = rt(&payload.join("00-hint.md"))?;
    let t01x = rt(&payload.join("01-artifacts.md"))?;
    let t02x = rt(&payload.join("02-narrative.md"))?;
    let t03x = rt(&payload.join("03-benchmark.md"))?;
    let c40 = "c".repeat(40);
    st.ck("0.2 · 0.4: HF revision 행 · 양자화 폴백 라벨", fb(&t00, "context")?.contains(&c40)
        && fb(&t00, "resolved")?.contains(&c40) && fb(&t00, "context")?.contains("명명 축 q(체크포인트 config 파생)"));
    st.ck("0.7 메타: 현행 full 충족(아니오 · 반복 1) · 사실 검증 미실시", fb(&t00, "meta")?.contains("아니오 · 이 측정의 반복 1")
        && fb(&t00, "meta")?.contains("미실시"));
    let w_open = _factcheck_line(&j(r#"{"status": "waived", "reason_code": "HINT_FACTCHECK_OPEN", "open": ["F1"], "waiver": "x"}"#))?;
    let w_abs = _factcheck_line(&j(r#"{"status": "waived", "reason_code": "HINT_FACTCHECK_ABSENT", "open": [], "waiver": "x"}"#))?;
    st.ck(&format!("★사람 면제 문구 = 면제한 사유대로(열린 지적을 면제하면 '검증 없이' 라 적지 않는다)({})", core::head_chars(&w_open, 40)),
        w_open.contains("열린 채 발행") && !w_open.contains("없이") && w_abs.contains("독립 검증 보고 없이 발행"));
    st.ck("03 §3.5: 현행 full 정의 원문 그대로 + 충족 아니오", fb(&t03x, "bench_definition")?.contains("> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)")
        && fb(&t03x, "bench_definition")?.contains("| 현행 full 정의 충족 | 아니오 |"));
    let bdx = rf(_f_bench_definition, &repo, &with(&facts_json, &[("bench_definition", merge(&facts_json["bench_definition"], &j(r#"{
        "source_line": 12, "required_repeats": 3, "bench_tool": "vllm-bench-serve", "bench_tool_source": "측정 구성 표",
        "reasons": ["판정점 반복 1 < 현행 정의 3"]}"#)))]), tpls)?;
    st.ck("03 §3.5: evidence 의 확장 키(요건 · 도구 · 사유 · 정의 줄)도 싣는다", [".claude/rules/docs.md:12", "| 현행 정의의 반복 요건 | 3 |",
        "| 이 측정의 도구 | vllm-bench-serve |", "판정점 반복 1 < 현행 정의 3"].iter().all(|s| bdx.contains(s)));
    let ev01 = fb(&t01x, "event_timeline")?;
    st.ck("01 §1.4: 이벤트 5행 + 기동 시도 2(첫 시도 renew 없음 · 둘째 1회)", ev01.contains("블랙박스 이벤트 5행")
        && ev01.contains("| 1 | main | `smoke-fixture-cell` | 2026-09-12T07:43:46Z | 없음 | 없음 | `budget_clear` 2026-09-12T08:14:38Z |")
        && ev01.contains("| 2 | main | `smoke-fixture-cell` | 2026-09-12T18:54:37Z | 1회 · 첫 2026-09-12T19:24:49Z | 없음 |"));
    st.ck("02 §2.2: 기동 시도 요약(행 전문은 01)", fb(&t02x, "event_attempts")?.contains("이 셀의 기동 시도")
        && !fb(&t02x, "event_attempts")?.contains("블랙박스 이벤트 5행"));
    st.ck("이벤트 없으면 '미관측' 명시(침묵 ✗)", rf(_f_event_timeline, &repo, &with(&facts_json, &[("event_timeline", J::List(vec![]))]), tpls)?.contains("미관측")
        && rf(_f_event_attempts, &repo, &with(&facts_json, &[("event_timeline", J::Null)]), tpls)?.contains("미관측"));
    // ── FACT_FIX2 G8 · G2 · G5(2026-09-29) ──
    let spans_ = j(r#"[{"node": "main", "first_utc": "2026-09-01T00:00:00Z", "last_utc": "2026-09-30T00:00:00Z", "files": ["m.jsonl"],
               "covers_measurement": true, "measurement_end_utc": "2026-09-23T01:57:09Z"},
              {"node": "sub", "first_utc": "2026-09-03T00:00:00Z", "last_utc": "2026-09-11T11:10:38Z", "files": ["s.jsonl"],
               "covers_measurement": false, "measurement_end_utc": "2026-09-23T01:57:09Z"}]"#);
    let s0 = spans_.at(0).clone();
    let s1 = spans_.at(1).clone();
    let et = |spans: J| rf(_f_event_timeline, &repo, &with(&facts_json, &[("event_ledger_spans", spans)]), tpls);
    let ea = |spans: J| rf(_f_event_attempts, &repo, &with(&facts_json, &[("event_ledger_spans", spans)]), tpls);
    let et8 = et(spans_.clone())?;
    st.ck("G8 이벤트 표에 원장 관측 범위 · 범위 끝 < 측정 끝인 노드 = '관측 범위 밖'(없음 ✗) · 기동 시도 요약에도",
        et8.contains("원장 관측 범위") && et8.contains("sub = 2026-09-03T00:00:00Z → 2026-09-11T11:10:38Z") && et8.contains("관측 범위 밖")
            && ea(spans_.clone())?.contains("관측 범위 밖"));
    let spans_l = J::List(vec![with(&s0, &[("live_local", J::Bool(false))]), with(&s1, &[("live_local", J::Bool(true))])]);
    let et8l = et(spans_l.clone())?;
    st.ck("F9 live_local 노드 = '마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님)' · 측정 끝 사실은 그대로",
        et8l.contains("마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님") && !et8l.contains("원장 미회수")
            && et8l.contains("sub(마지막 행 2026-09-11T11:10:38Z < 측정 끝 2026-09-23T01:57:09Z)"));
    st.ck("★F9 live_local False(회수본) 노드 문구는 그대로 '관측 범위 밖 · 원장 미회수'",
        et8.contains("원장 미회수") && !et8.contains("미회수 아님")
            && et(J::List(vec![spans_l.at(0).clone(), with(&s1, &[("live_local", J::Bool(false))])]))?.contains("원장 미회수"));
    let sh_ = _ledger_span_lines(&crate::jd! {"event_ledger_spans" => J::List(vec![with(&s1, &[("live_local", J::Bool(false)),
        ("shadowed_local_copies", j(r#"["docs/logs/sub/events/2026-09.jsonl"]"#))])])})?;
    st.ck("F8 회수 미러가 가린 옛 로컬 사본은 범위 줄에 기재(조용히 버리지 않는다)",
        sh_.join("\n").contains("회수 미러가 가린 옛 로컬 사본 `docs/logs/sub/events/2026-09.jsonl`"));
    let echo_ = j(r#"{"log": "docs/simlog/r/serve.log", "basis": "같은 기동 판별", "rows": [
        {"ts": "2026-09-23T00:10:03Z", "kind": "budget_honored", "label": "smoke-x", "source": "docs/simlog/r/serve.log:26"}]}"#);
    let spans_e = J::List(vec![s0.clone(), with(&s1, &[("smoke_echo", echo_.clone())])]);
    let et8e = et(spans_e.clone())?;
    st.ck("F10 미러 밖 노드의 스모크 로그 echo = 보조 관측 줄(출처 L 번호 · '원장 미러 밖 — 스모크 로그 echo') · 그 밖 사건은 여전히 관측 범위 밖",
        et8e.contains("보조 관측 · sub") && et8e.contains("docs/simlog/r/serve.log:26") && et8e.contains("원장 미러 밖 — 스모크 로그 echo")
            && et8e.contains("관측된 것은 `budget_honored` 뿐") && et8e.contains("사살 · 트립 · 갱신 · clear)은 여전히 **관측 범위 밖**")
            && ea(spans_e.clone())?.contains("보조 관측 · sub"));
    st.ck("★F10 음성대조: echo 없음 · 측정 끝을 덮는 노드의 echo = 보조 관측 줄 없음",
        !et8.contains("보조 관측") && !et(J::List(vec![with(&s0, &[("smoke_echo", echo_.clone())])]))?.contains("보조 관측"));
    st.ck("★G8 음성대조: 모든 노드가 측정 끝을 덮으면 경고 없음 · 범위 입력이 없으면 줄 없음",
        !et(J::List(vec![s0.clone()]))?.contains("관측 범위 밖") && !et(J::Null)?.contains("원장 관측 범위"));
    let q2 = j(r#"{"health_200": true, "inference_observed": true, "sources": ["output/multi/benchlog/serve_proof_x.json"],
          "scope": {"written_utc": "2026-09-28T01:31:48Z", "written_utc_source": "파일 mtime", "timing": "after-measurement",
                    "timing_note": "작성 2026-09-28T01:31:48Z > 측정 끝 2026-09-28T00:35:13Z — 측정 뒤 작성된 스모크 산출물(측정 시점·같은 기동 여부는 시각만으로 미검증)"}}"#);
    st.ck("G2 발행 자격 근거(FACT:grade · 재현 끝 줄)에 serve_proof 시점 범위",
        _qualification_line(&crate::jd! {"qualification" => q2.clone()})?.contains("측정 뒤 작성된 스모크 산출물(측정 시점·같은 기동 여부는 시각만으로 미검증)")
            && _qual_scope_text(&q2)?.contains("측정 뒤 작성된 스모크 산출물(측정 시점·같은 기동 여부는 시각만으로 미검증)"));
    st.ck("★G2 음성대조: 범위가 없으면 근거 칸에 덧붙이지 않는다", _qual_scope_text(&with(&q2, &[("scope", J::Null)]))?.is_empty());
    let rl5 = _revision_line("compose", &j(r#"{"path": "x.sh", "commit": "@a40@", "method": "worktree", "basis": "mtime≤measured",
                                     "verified": null, "executed": false}"#))?;
    st.ck("G5 실행 안 된 경로 = '쓰인 바이트임을 관측으로 확인 해당 없음(실행 안 됨)'", rl5.contains("확인 해당 없음(실행 안 됨)") && !rl5.contains("확인 예"));
    st.ck("★G5 음성대조: 실행된 경로(verified 불리언)는 예/아니오 그대로",
        _revision_line("compose", &j(r#"{"path": "x.sh", "method": "worktree", "verified": true}"#))?.contains("확인 예"));
    let empty_ev = rf(_f_event_timeline, &repo, &with(&facts_json, &[("event_timeline", J::List(vec![]))]), tpls)?;
    st.ck("★빈 타임라인 = '구분하지 않는다' 로 정직하게(옛 'evidence 가 가른다' ✗)",
        empty_ev.contains("구분하지 않는다") && !empty_ev.contains("evidence 가 가른다"));
    st.ck("선언 행 없이 시작한 시도도 행으로(끊긴 원장)", _attempts_table(&j(
        r#"[{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "watchdog_trip", "label": null}]"#), &J::Null)?.join("\n").contains("—(선언 행 없음)"));
    let exact = j(r#"[{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
             {"utc": "2026-01-01T00:02:00Z", "node": "main", "kind": "budget_declare_rejected", "label": "smoke-x"},
             {"utc": "2026-01-01T00:03:00Z", "node": "main", "kind": "thermal_gpu_stale_clear", "label": null},
             {"utc": "2026-01-01T00:04:00Z", "node": "main", "kind": "watchdog_kill_ack", "label": null},
             {"utc": "2026-01-01T00:05:00Z", "node": "main", "kind": "budget_clear", "label": null},
             {"utc": "2026-01-01T01:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
             {"utc": "2026-01-01T01:30:00Z", "node": "main", "kind": "budget_expired", "label": null}]"#);
    let ae = _attempts(&exact)?;
    let tbl = _attempts_table(&exact, &J::Null)?.join("\n");
    st.ck(&format!("★거부된 선언(budget_declare_rejected)은 새 시도가 아니다 · stale_clear 는 닫힘이 아니다 · 사살은 clear 에 덮이지 않는다 · \
expired = 닫힘({})", J::List(ae.clone()).repr()), ae.len() == 2
        && jeq(&ae[0]["alerts"], &j(r#"{"budget_declare_rejected": 1, "watchdog_kill_ack": 1}"#))
        && ae[0]["end"] == J::from("budget_clear") && ae[0]["other"] == J::Int(1) && ae[1]["end"] == J::from("budget_expired")
        && tbl.contains("`budget_declare_rejected` 1 · `watchdog_kill_ack` 1"));
    let mixed = j(r#"[{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
             {"utc": "2026-01-01T00:05:00Z", "node": "main", "kind": "engine_init_done", "label": null},
             {"utc": "2026-01-01T00:09:00Z", "node": "main", "kind": "budget_clear", "label": null},
             {"utc": null, "node": "main", "kind": "ledger_unparsable_lines", "label": null}]"#);
    let am = _attempts(&mixed)?;
    st.ck(&format!("★라벨 없는 clear · 엔진 표지 · 판독 불가 행은 같은 노드의 현재 시도에 붙는다(라벨로 쪼개지 않는다)({})", J::List(am.clone()).repr()),
        am.len() == 1 && am[0]["end"] == J::from("budget_clear") && am[0]["other"] == J::Int(2) && am[0]["label"] == J::from("smoke-x"));
    let cross = j(r#"[{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
             {"utc": "2026-01-01T00:01:00Z", "node": "cluster", "kind": "engine_log_window_start", "label": null},
             {"utc": "2026-01-01T00:09:00Z", "node": "main", "kind": "budget_clear", "label": null},
             {"utc": "2026-01-01T01:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
             {"utc": "2026-01-01T01:05:00Z", "node": "cluster", "kind": "engine_init_done", "label": null},
             {"utc": "2026-01-01T01:06:00Z", "node": "main", "kind": "budget_renew", "label": "smoke-x"}]"#);
    let ac = _attempts(&cross)?;
    st.ck(&format!("★선언 없는 노드(cluster 엔진 표지)의 행은 그 시각의 최근 시도에 붙는다 — 셋째 '시도' ✗(2026-09-22 태그2 실측)({})", J::List(ac.clone()).repr()),
        ac.len() == 2 && ac.iter().map(|a| a["other"].clone()).collect::<Vec<_>>() == vec![J::Int(1), J::Int(1)]
            && ac[1]["renew"] == j(r#"["2026-01-01T01:06:00Z"]"#));
    let cr = cross.items();
    let mut meas = vec![j(r#"{"utc": "2025-12-31T00:00:00Z", "node": null, "kind": "measurement", "label": "measurement (no budget event recorded)"}"#)];
    meas.extend(cr[..3].iter().cloned());
    meas.push(j(r#"{"utc": "2026-01-01T00:30:00Z", "node": null, "kind": "measurement", "label": "measurement (ledger unobserved)"}"#));
    meas.extend(cr[3..].iter().cloned());
    let meas = J::List(meas);
    let amx = _attempts(&meas)?;
    st.ck(&format!("★측정 기록 행은 가짜 시도를 열지 않고 시도의 '그 밖 행' 에도 붙지 않는다({})", J::List(amx.clone()).repr()),
        amx.len() == 2 && amx.iter().map(|a| a["other"].clone()).collect::<Vec<_>>() == vec![J::Int(1), J::Int(1)]
            && amx.iter().all(|a| a["node"] == J::from("main")) && !_attempts_table(&meas, &J::Null)?.join("\n").contains("measurement ("));
    st.ck("★측정 기록 행만 있으면 시도 표 = 미관측(가짜 시도 ✗)",
        _attempts_table(&J::List(meas.items()[..1].to_vec()), &J::Null)?.join("\n").contains("기동 시도 미관측"));
    let f5 = j(&format!(r#"[{{"utc": "2026-01-01T00:00:00Z", "node": "sub", "kind": "budget_declare", "label": "serve-x-v030",
           "label_scope": "{o}", "campaign_scope": "prior-campaign"}},
          {{"utc": "2026-01-01T00:01:00Z", "node": "sub", "kind": "serve_start", "label": null, "campaign_scope": "prior-campaign"}},
          {{"utc": "2026-01-01T00:10:00Z", "node": "sub", "kind": "watchdog_trip", "label": null,
           "label_scope": "{o}", "campaign_scope": "prior-campaign"}},
          {{"utc": "2026-01-02T02:30:00Z", "node": "sub", "kind": "budget_declare", "label": "smoke-x-35b-v030",
           "campaign_scope": "this-campaign"}}]"#, o = _LABEL_SCOPE_OTHER));
    let b5 = j(r#"{"utc": "2026-01-02T00:00:00Z"}"#);
    let t5 = _attempts_table(&f5, &b5)?.join("\n");
    let e5 = rf(_f_event_timeline, &repo, &with(&facts_json, &[("event_timeline", f5.clone()), ("event_campaign_boundary", b5.clone())]), tpls)?;
    st.ck("★F5 라벨 다른 창으로 연 앞선 시도 = 선언 · 트립 셈 · label 칸 '라벨 다름' 표지('선언 행 없음' · '사살 없음' ✗) · 머리 개수",
        t5.contains("| 앞선-1 | sub | `serve-x-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-01-01T00:00:00Z | 없음 | `watchdog_trip` 1 |")
            && !t5.contains("선언 행 없음") && t5.contains("| 1 | sub | `smoke-x-35b-v030` |")
            && !t5.split("앞선-1").next().unwrap_or("").contains("(라벨 다름")
            && e5.contains("라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2행"));
    st.ck("custom label summary limits exclusion to label matching, not execution attribution",
        e5.contains("셀 라벨 일치 집계에서 제외(세션·호출 기록의 귀속 확인과 별도)") && !e5.contains("이 셀의 선언으로 세지 않는다")
            && e5.contains("라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2행"));
    let mut f6v = vec![j(r#"{"utc": "2025-12-31T23:00:00Z", "node": "sub", "kind": "budget_declare", "label": "smoke-x-35b-v030",
           "campaign_scope": "prior-campaign"}"#)];
    f6v.extend(f5.items().iter().cloned());
    let f6 = J::List(f6v.clone());
    let t6 = _attempts_table(&f6, &b5)?.join("\n");
    st.ck("★F6 부표에 라벨 다름 시도가 있으면 머리 = 경계 이전 기동 N회(이 셀 라벨 1회 · 라벨 다름 1회) · '라벨이 같아' ✗",
        t6.contains("**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-01-02T00:00:00Z 이전 — 이 셀 라벨로 묶인 1회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 \
시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다") && !t6.contains("라벨이 같아")
            && !t5.contains("라벨이 같아") && t6.contains("| 앞선-2 | sub | `serve-x-v030`(라벨 다름"));
    let mut f6b = vec![f6v[0].clone()];
    f6b.extend(f5.items()[3..].iter().cloned());
    st.ck("★F6 음성대조: 같은 라벨 시도뿐이면 종전 머리('같은 셀 이름 · 앞선 캠페인 · 라벨이 같아')",
        _attempts_table(&J::List(f6b), &b5)?.join("\n").contains("같은 셀 이름 · 앞선 캠페인의 기동 1회"));
    let bnd_ = j(r#"{"utc": "2026-01-01T00:30:00Z", "observed": true, "campaign_id": "camp-x", "source": "declared_utc …"}"#);
    let bu = bnd_["utc"].display();
    let scope_rows = |pre: &str| -> J {
        J::List(cr.iter().map(|r| with(r, &[("campaign_scope", J::from(if r["utc"].display() < bu { pre } else { "this-campaign" }))])).collect())
    };
    let scoped = scope_rows("prior-campaign");
    let tb = _attempts_table(&scoped, &bnd_)?.join("\n");
    st.ck("캠페인 경계: 이번 캠페인 시도만 번호 · 앞선 캠페인 시도는 '같은 셀 이름 · 앞선 캠페인' 표로 따로",
        tb.contains("| 1 | main |") && !tb.contains("| 2 | main |") && tb.contains("같은 셀 이름 · 앞선 캠페인의 기동 1회") && tb.contains("| 앞선-1 | main |"));
    st.ck("★음성대조: 경계 미관측(재생)이면 현 규칙(라벨 전부) · 분리 표 ✗ · 경계 미관측 줄은 FACT 에",
        !_attempts_table(&scoped, &j(r#"{"utc": null, "source": "재생"}"#))?.join("\n").contains("앞선")
            && _attempts_table(&scoped, &J::Null)?.join("\n").contains("| 2 | main |")
            && rf(_f_event_attempts, &repo, &with(&facts_json, &[("event_campaign_boundary", j(r#"{"utc": null, "source": "재생 모드"}"#))]), tpls)?
                .contains("캠페인 경계 미관측"));
    st.ck("★이번 캠페인 시도가 0 이면 그렇게 적는다(앞선 캠페인 시도를 이 셀 시도로 올리지 않는다)",
        _attempts_table(&J::List(scoped.items()[..3].to_vec()), &bnd_)?.join("\n").contains("이번 캠페인(경계 2026-01-01T00:30:00Z 이후)의 기동 시도 미관측"));
    let sc14 = scope_rows("same-cell-pre-boundary");
    let tb14 = _attempts_table(&sc14, &bnd_)?.join("\n");
    let ev14 = rf(_f_event_timeline, &repo, &with(&facts_json, &[("event_timeline", sc14.clone()), ("event_campaign_boundary", bnd_.clone())]), tpls)?;
    st.ck("★F14 same-cell-pre-boundary 시도 = 이 셀 시도(번호 1·2) · '앞선 캠페인' 표 ✗ · 머리 개수에 '캠페인 선언 전 · 같은 셀 기동'",
        tb14.contains("| 1 | main |") && tb14.contains("| 2 | main |") && !tb14.contains("앞선")
            && ev14.contains("캠페인 선언 전 · 같은 셀 기동") && !ev14.split('\n').next().unwrap_or("").contains("앞선 캠페인 "));
    let id16 = if facts_json["identity"].is_dict() { facts_json["identity"].clone() } else { J::dict() };
    let id16 = with(&id16, &[("source", with(&id16["source"], &[("quant", J::from("naming-axis(q) — 측정 identity quantization = none(관측)(x) — …"))]))]);
    let c16 = rf(_f_context, &repo, &with(&facts_json, &[("identity", id16)]), tpls)?;
    st.ck("★F16 측정 none 관측 표지 → 양자화 행에 '인증서 quantization 이 N/A' 덧말 ✗", c16.contains("측정 identity quantization = none(관측)")
        && !c16.contains("인증서 quantization 이 N/A"));
    st.ck("1.1 결과 출처 범례(쓰인 값만)", t01x.contains("**결과 출처 범례**") && t01x.contains("`reconstructed(docker-history+gate)` —")
        && !t01x.contains("`image-probe(script-sha+marker)`"));
    st.ck("0.5 파생 요약: 지위별 개수", fb(&t00, "knob_status")?.contains("`inherited` 2 · `negative-control` 1"));
    let mut obs_facts = jc(&facts_json);
    *pm(&mut obs_facts, &["reproduce_steps", "1", "observed"]) = j(r#"{"start_utc": "2026-09-12T18:54:37Z", "end_utc": "2026-09-12T19:25:31Z",
                                                   "seconds": 1854, "bound": "upper"}"#);
    let reproduce_header = rf(_f_reproduce, &repo, &facts_json, tpls)?;
    st.ck("재현 명령 출처: 관측·파일 설정·추론을 구분하고 과거 실행으로 단정하지 않는다",
        reproduce_header.contains("관측, 파일 선언·설정 또는 추론")
            && reproduce_header.contains("설정만으로 과거 compose build 실행 여부는 알 수 없다")
            && !reproduce_header.contains("괄호 안 관측(docker history build-arg · compose build"));
    st.ck("1.4 재현 표: 관측 구간 칸(start → end (bound))",
        rf(_f_reproduce, &repo, &obs_facts, tpls)?.contains("2026-09-12T18:54:37Z → 2026-09-12T19:25:31Z (upper)")
            && reproduce_header.contains("| 관측 구간(UTC) |"));
    st.ck("★PII 리터럴", lint_v(&variant("pii", &ed("02-narrative.md", "- moe-backend 를", &format!("- {term} 가 moe-backend 를")))?)?.contains("HINT_PII"));
    st.ck("★brief 4줄", lint_v(&variant("b4", &ed("00-hint.md", "복사하지 마라.", "복사하지 마라.\n넷째 줄."))?)?.contains("HINT_BRIEF_TOO_LONG"));
    st.ck("★brief 인용 시작", lint_v(&variant("bq", &ed("00-hint.md", "fixture-model 을 GB10", "> fixture-model 을 GB10"))?)?.contains("HINT_BRIEF_SHAPE"));
    st.ck("★PROMPT 지시 변조", lint_v(&variant("pt", &ed("02-narrative.md", "블록: wall>=1\n", ""))?)?.contains("HINT_PROMPT_TAMPERED"));
    st.ck("★kind 자리 위반", lint_v(&variant("km", &ed("02-narrative.md", "- moe-backend 를 명시하지 마라",
        "```hint-event\nid: R9\nkind: rejected\n시도: a\n기각사유: b\n출처: [testlog_26090921 §2]\n```\n- moe-backend 를 명시하지 마라"))?)?
        .contains("HINT_EVENT_KIND_MISPLACED"));
    st.ck("★id 중복", lint_v(&variant("dup", &ed("02-narrative.md", "id: W2", "id: W1"))?)?.contains("HINT_EVENT_ID_DUPLICATE"));
    st.ck("★id 모양(kind 접두)", lint_v(&variant("ids", &ed("02-narrative.md", "id: R1\nkind: rejected\n시도: moe", "id: Q7\nkind: rejected\n시도: moe"))?)?
        .contains("HINT_EVENT_ID_SHAPE"));
    st.ck("★필수 발췌 부재", lint_v(&variant("er", &ed("02-narrative.md",
        "> [원문] plan_26090918 §1\n> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.", "첫 plan 요약."))?)?.contains("HINT_EXCERPT_REQUIRED"));
    st.ck("★모르는 챕터", lint_v(&variant("sec", &ed("02-narrative.md", "## 2.7 열린 물음", "## 2.8 추가\n\n## 2.7 열린 물음"))?)?.contains("HINT_SECTION_UNKNOWN"));
    st.ck("★README 손수정", lint_v(&variant("rd", &ed(README_NAME, "## 읽는 순서", "## 읽는 차례"))?)?.contains("HINT_README_DRIFT"));
    st.ck("★스냅샷 부재", kw.codes(&repo, &variant("ns", &noop())?)?.contains("HINT_FACT_SNAPSHOT_ABSENT"));
    st.ck("★PII 용어 파일 부재", kwf.clone().pii(Some(None)).codes(&repo, &payload)?.contains("HINT_MISSING_PII_TERMS"));
    st.ck("★치환 함수 부재", kwf.clone().sub(None).st(J::Null).codes(&repo, &payload)?.contains("HINT_EXCERPT_VERIFIER_ABSENT"));
    // ── 2026-09-22 리뷰 추가 음성대조 ──
    let bn_dir = variant("bn-cli", &ed("03-benchmark.md", bench_row, "| 1 ★판정점 | 38.18 |"))?;
    st.ck("★CLI --verify 도 숫자 변조를 잡는다", cli_verify(&bn_dir, &repo.join(rep_p)) == 1);
    st.ck("★절 제목이 앞쪽에 한 번 더(CLI 가 첫 줄을 잡는다)", lint_v(&variant("bt2", &ed("03-benchmark.md", "## 3.1 측정 결과\n",
        &format!("## 3.1 측정 결과\n{}\n", bench_md.split('\n').next().unwrap_or(""))))?)?.contains("HINT_BENCH_SECTION_DRIFT"));
    let no_cands = with(&facts_json, &[("value_status_candidates", J::List(vec![]))]);
    st.ck("★value-status 후보 0개(트리플렛 부재 미기재) = 공허 통과 ✗",
        kw.clone().f(&no_cands).codes(&repo, &payload)?.contains("HINT_VALUE_STATUS_CANDIDATES_ABSENT"));
    st.ck("value-status 후보 0개 + 트리플렛 부재 기재 = 커버리지 면제", !kw.clone().f(&with(&no_cands, &[("missing",
        j(r#"{"HINT_MISSING_TRIPLET": "트리플렛 부재"}"#))])).codes(&repo, &payload)?.contains("HINT_VALUE_STATUS_CANDIDATES_ABSENT"));
    st.ck("★미등록 결손 코드", kw.clone().f(&with(&facts_json, &[("missing", j(r#"["HINT_NOT_A_CODE"]"#))])).codes(&repo, &payload)?
        .contains("HINT_MISSING_CODE_UNREGISTERED"));
    st.ck("★task_class facts 누락(manifest 만 선언)", kw.clone().f(&with(&facts_json, &[("task_class", J::Null),
        ("publication", j(r#"{"topic": "camp_fixture"}"#))])).codes(&repo, &payload)?.contains("HINT_TASK_CLASS_CONFLICT"));
    st.ck("★brief 문단 안의 주석 경계(봉인 뒤 tag 가 거부하기 전에 lint 가 막는다)", lint_v(&variant("bcm", &ed("00-hint.md",
        "복사하지 마라.", "복사하지 마라. <!-- 메모 -->"))?)?.contains("HINT_BRIEF_SHAPE"));
    let bl = variant("bcl", &ed("00-hint.md", "t/s.\n가장 비싼 벽은", "t/s.\n<!-- 메모 -->\n가장 비싼 벽은"))?;
    st.ck("brief 는 주석 줄에서 끝난다(주석을 brief 로 캐지 않는다)", brief(&bl)?.ends_with("t/s.") && !brief(&bl)?.contains("<!--"));
    let bmc = rf(_f_bench_missing, &repo, &with(&facts_json, &[("missing", j(r#"["HINT_MISSING_CERTIFICATE"]"#))]), tpls)?;
    st.ck("03 인증서 부재 = '판정되지 않았다' 배너 + 측정 결손 표(옛 render_item3 · selftest_hint_gate ①)",
        bmc.contains("판정되지 않았다") && bmc.contains("`HINT_MISSING_CERTIFICATE`"));
    let warn_facts = with(&facts_json, &[
        ("slots", j(r#"{"build_recipe": {"files": ["artifacts/build_recipe/Dockerfile.source-build"],
                                         "evidence": {"kind": "file", "ref": "cell-env",
                                                      "recipe_vs_image": [{"path": "output/multi/Dockerfile.source-build",
                                                                           "warning": "작업트리 수정본(미커밋)"}],
                                                      "context_not_shipped": [{"source": "x/*", "why": "글롭"}],
                                                      "selected_revision": {"commit": "@d40@", "method": "docker-history-match",
                                                                            "history_match": {"matched": 12, "total": 12}}}},
                     "compose": {"files": ["artifacts/compose/.env.interconnect.template"],
                                 "evidence": {"kind": "file", "ref": "render",
                                              "post_measurement_regenerated": ["output/multi/envs/.env.interconnect"]}}}"#)),
        ("applied_set", with(&facts_json["applied_set"], &[("probes", j(r#"[{"tier": "attestation", "result": "absent"},
                                                                           {"tier": "local-image", "image": "sha256:ab",
                                                                            "result": "no-ledger(cat rc=1)"}]"#))])),
    ]);
    let ws = rf(_f_slots, &repo, &warn_facts, tpls)?;
    let wa = rf(_f_applied_set, &repo, &warn_facts, tpls)?;
    st.ck("슬롯 표: 레시피 대 이미지 경고 · 싣지 못한 COPY 원천을 본문에 싣는다", ws.contains("⚠ `build_recipe`")
        && ws.contains("작업트리 수정본") && ws.contains("COPY 원천 `x/*` 미실림"));
    st.ck("적용 집합: 원장 탐침 표(조용한 강등 ✗)", wa.contains("**원장 탐침**") && wa.contains("no-ledger(cat rc=1)"));
    st.ck("슬롯 표: 선택 리비전(docker history 대조) · 측정 뒤 재생성 파일 경고(S2 round 2)",
        ws.contains(&format!("커밋 `{}` · 선택 방법 `docker-history-match` · docker history 대조 12/12", "d".repeat(12)))
            && ws.contains("측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: `output/multi/envs/.env.interconnect`"));
    st.ck("경고·탐침 없으면 표만(기존 본문 불변)", !rf(_f_slots, &repo, &facts_json, tpls)?.contains("레시피 경고")
        && !rf(_f_applied_set, &repo, &facts_json, tpls)?.contains("원장 탐침"));
    // ── 2026-09-22 · plan_26092119 S2 round 3: 공유 사실 계약의 새 키 렌더 ──
    let mut r3 = jc(&facts_json);
    r3.set("slots", j(r#"{
        "build_recipe": {"files": ["artifacts/build_recipe/Dockerfile.source-build", "artifacts/build_recipe/requirements.txt"],
                         "evidence": {"kind": "file", "ref": "cell-env",
                                      "selected_revision": {"path": "output/multi/Dockerfile.source-build", "commit": "@d40@",
                                                            "method": "git log -1 --before", "history_match": {"matched": 16, "total": 16}},
                                      "selected_revisions": [
                                          {"path": "output/multi/Dockerfile.source-build", "commit": "@d40@", "method": "git log -1 --before",
                                           "history_match": {"matched": 16, "total": 16}, "shipped": "git:@d40@", "verified": true},
                                          {"path": "output/multi/requirements.txt", "commit": null, "method": "worktree(추적 개정과 동일)",
                                           "history_match": null, "shipped": "worktree", "verified": false}]}},
        "compose": {"files": ["artifacts/compose/docker-compose.yaml", "artifacts/compose/serve_runner.sh"],
                    "evidence": {"kind": "file", "ref": "output/multi/docker-compose.yaml",
                                 "selected_revisions": [{"path": "output/multi/docker-compose.yaml", "commit": "@e40@",
                                                         "method": "git log -1 --before=2026-09-12T19:32:52Z", "history_match": null,
                                                         "shipped": "git:@e40@", "verified": null,
                                                         "basis": "git-rev-before-measurement",
                                                         "commits_after_measurement": ["@a40@", "@b40@"],
                                                         "warning": "작업트리가 측정 뒤 수정됐다(주석)"}],
                                 "revision_warnings": ["output/multi/docker-compose.yaml: 작업트리가 측정 뒤 수정됐다(주석)"],
                                 "post_measurement_regenerated": ["output/multi/envs/.env.interconnect"]},
                    "excluded": [{"file": ".env.interconnect", "reason": "post-measurement-regenerated",
                                  "mtime_utc": "2026-09-17T10:57:56Z", "measured_source": "certificate.measured_utc",
                                  "post_measurement_regenerated": true},
                                 {"file": "arm_patch.sh", "why": "not-armed(런타임 패치 없음)"}]},
        "build_patch_pre": {"files": ["artifacts/build_patch_pre/60-fixture-patch.sh"],
                            "evidence": {"kind": "reconstruction", "ref": "no-build-ledger"},
                            "excluded": [{"phase": "pre", "file": ".gitkeep", "why": "known-non-patch(패치가 아님 — 명시적 건너뜀)"},
                                         {"phase": "pre", "file": "files/", "why": "known-non-patch(패치가 아님 — 명시적 건너뜀)"},
                                         {"phase": "pre", "file": "50-skip-patch.sh", "why": "skipped(SM12X_PORT=0)"}]}}"#));
    let r3as = with(&r3["applied_set"], &[("probes", j(r#"[{"tier": "attestation", "ref": "output/multi/benchlog/attestation_build.json",
                                                         "result": "no-ledger"}]"#))]);
    r3.set("applied_set", r3as);
    {
        let p0 = pm(&mut r3, &["applied_set", "patches", "0"]);
        p0.set("evidence", j(r#"{"static": ["skip 경로 없음"], "loop": "docker history 루프 `/tmp/p/*.sh … || exit 1`",
                  "markers": {"status": "all-found", "declared": 2, "found": 2}}"#));
        p0.set("script_identity", "image-probe(sha256 일치 — 실린 바이트 = 이미지에 구워진 바이트)");
        p0.set("image_script_sha256", "f".repeat(64));
    }
    r3.set("attestation_scope", j(r#"{"config": "fixture-build", "written_utc": "2026-09-09T20:00:00Z", "written_utc_source": "파일 mtime",
                               "bound_by": "이미지 compose 라벨 fixture-build", "phase": "smoke",
                               "path": "output/multi/benchlog/attestation_fixture-build.json",
                               "scope": "image-build/smoke — not this cell's run"}"#));
    *pm(&mut r3, &["reproduce_steps", "1", "observed"]) = j(r#"{"start_utc": "2026-09-09T13:40:00Z", "end_utc": "2026-09-09T14:24:35Z",
                                            "seconds": 2675, "bound": "layer-span",
                                            "layer_span": {"oldest_utc": "2026-09-09T13:40:00Z", "newest_utc": "2026-09-09T14:24:35Z",
                                                           "layers": 28, "source": "docker history", "note": "캐시 층 포함"}}"#);
    r3.set("measurement_env_observed", j(&format!(r#"[
        {{"key": "NCCL_IB_DISABLE", "value": "0", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_IB_DISABLE", "value": "0", "node": "sub", "source": "{log_p}:L2"}},
        {{"key": "NCCL_NET_GDR_LEVEL", "value": "SYS", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_NET_GDR_LEVEL", "value": "LOC", "node": "sub", "source": "{log_p}:L2"}},
        {{"key": "NCCL_SOCKET_IFNAME", "value": "<nic:cluster>", "node": "main", "source": "{log_p}:L1", "kind": "env-echo",
         "occurrences": 3, "seen_in_logs": 2, "value_note": "기계 치환(자리표시)"}},
        {{"key": "NCCL_DEBUG", "value": "", "node": null, "node_label": "cluster", "source": "{log_p}:L3"}}]"#)));
    r3.set("tool_snapshots", j(r#"[{"name": "run_bench.sh", "repo_path": ".claude/skills/adversarial-benchmark/scripts/run_bench.sh",
                             "git_rev": "0123456789ab@c28@", "snapshot_rel": "inputs/sources/run_bench.sh@0123456789ab",
                             "role": "bench", "why": "sweep_bench.sh 가 레벨마다 부른다", "git_rev_utc": "2026-09-05T01:00:00Z",
                             "next_rev": "@940@", "next_rev_utc": "2026-09-14T02:00:00Z",
                             "worktree_note": "측정 시각의 워킹트리는 관측 대상 밖", "driver_note": null}]"#));
    *pm(&mut r3, &["naming", "vllm_observed"]) = j(r#"{
        "engine_self_report": null, "engine_self_report_source": "같은 digest 의 엔진 로그 기동 배너 없음(benchlog 3개 탐색)",
        "engine_self_report_basis": "unobserved",
        "certificate_vllm_version": "0.29.0", "certificate_vllm_version_producer": "image-tag-line",
        "certificate_vllm_version_source": "sweep_bench.sh@8ca23d4 L337-340 — IMAGE_TAG 에서 x.y.z 로 자른 값(엔진 자기보고 아님)",
        "wheel_meta": null, "source": "인증서 vllm_version"}"#);
    let r3 = r3;
    let s3 = rf(_f_slots, &repo, &r3, tpls)?;
    let a3 = rf(_f_applied_set, &repo, &r3, tpls)?;
    let rp3 = rf(_f_reproduce, &repo, &r3, tpls)?;
    st.ck("R3 슬롯: 복수 리비전(build_recipe 2 · compose 1) 전부 · 단수는 복수에 있으면 한 번만",
        count(&s3, "`output/multi/Dockerfile.source-build` 실린 리비전") == 1 && s3.contains("`output/multi/requirements.txt` 실린 리비전")
            && s3.contains(&format!("`compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `{}`", "e".repeat(12))) && s3.contains("docker history 대조 없음"));
    st.ck("R3 슬롯: compose 추적 파일 근거(basis) · 측정 뒤 커밋 수 · 파일 경고 · 슬롯 리비전 경고",
        s3.contains("근거 `git-rev-before-measurement`") && s3.contains("측정 뒤 커밋 2건") && s3.contains("— ⚠ 작업트리가 측정 뒤 수정됐다(주석)")
            && s3.contains("- ⚠ `compose` 리비전 경고: output/multi/docker-compose.yaml: 작업트리가 측정 뒤 수정됐다(주석)"));
    st.ck("R3 슬롯: 싣지 않은 파일 — post-measurement-regenerated 뜻 · 추가 칸(mtime) · not-armed 사유",
        s3.contains("`compose` `.env.interconnect` — `post-measurement-regenerated` · 측정 뒤 재생성된 형상")
            && s3.contains("mtime_utc 2026-09-17T10:57:56Z") && s3.contains("`arm_patch.sh` — `not-armed(런타임 패치 없음)`"));
    st.ck("★R3 슬롯: 제외 행의 추가 칸은 닫힌 목록(mtime · 측정 시각)만 — 생산자 보조 칸(measured_source · 불리언)은 PAYLOAD 몫",
        !s3.contains("measured_source") && !s3.contains("post_measurement_regenerated True"));
    st.ck("★R3 슬롯: 싣지 않은 파일 목록이 말하는 재생성 파일은 경고 줄에 다시 적지 않는다(두 자리 ✗)",
        !s3.contains("측정 뒤 재생성된 파일") && count(&s3, ".env.interconnect") == 1);
    let mut r3b = jc(&r3);
    *pm(&mut r3b, &["slots", "compose", "excluded"]) = J::List(vec![]);
    let s3b = rf(_f_slots, &repo, &r3b, tpls)?;
    st.ck("R3 슬롯: 싣지 않았고 제외 목록에도 없는 재생성 파일 = '싣지 않았다' 줄(옛 '실린 형상' 문구 ✗)",
        s3b.contains("측정 뒤 재생성된 파일(mtime > 측정 시각)은 싣지 않았다: `output/multi/envs/.env.interconnect`")
            && !s3b.contains("실린 형상은 측정 당시 값과 다를 수 있다"));
    st.ck("★_cell: 목록 항목의 `|` 를 두 번 이스케이프하지 않는다", _cell(&j(r#"["a || b", "c"]"#), _MISSING_TEXT)? == "a \\|\\| b · c");
    st.ck("★R3 슬롯: 패치 아님은 개수 · 이름만 · skip 행은 적용 집합 표에만(두 자리에 싣지 않는다)",
        s3.contains("`build_patch_pre` known-non-patch 2건: `.gitkeep` · `files/`") && !s3.contains("50-skip-patch.sh"));
    st.ck("R3 적용 집합: 패치별 판정 근거(스크립트 바이트 정체 · 이미지 sha256 · 표지 · 정적 규칙) + 단계별 빌드 루프 한 줄",
        a3.contains("**판정 근거**") && a3.contains(&format!("`{}`", "f".repeat(64))) && a3.contains("all-found 2/2") && a3.contains("image-probe(sha256 일치")
            && a3.contains("- `pre` 빌드 루프: docker history 루프"));
    st.ck("★R3 적용 집합: 근거 키가 없는 패치만 있으면 판정 근거 표 없음(빈 표 ✗)", !rf(_f_applied_set, &repo, &facts_json, tpls)?.contains("**판정 근거**"));
    let att_all = [a3.clone(), rf(_f_sub_recipe, &repo, &r3, tpls)?, rf(_f_missing, &repo, &r3, tpls)?];
    st.ck("R3 attestation 범위 줄: 01 §1.1 탐침 · 01 §1.5 ABI · 00 결손(attestation 코드) 세 자리 · 생산자 scope 그대로",
        att_all.iter().all(|x| x.contains("**attestation 범위**") && x.contains("config `fixture-build`") && x.contains("**image-build/smoke — not this cell's run**")
            && x.contains("묶은 근거 이미지 compose 라벨") && x.contains("phase smoke")));
    st.ck("★R3 attestation 범위 없음 = 줄 없음 · 결손에 attestation 코드가 없으면 00 에 붙지 않는다",
        !rf(_f_sub_recipe, &repo, &facts_json, tpls)?.contains("attestation 범위")
            && !rf(_f_missing, &repo, &with(&r3, &[("missing", j(r#"{"HINT_MISSING_TRIPLET": "x"}"#))]), tpls)?.contains("attestation 범위"));
    st.ck("R3 재현: 빌드 층 CreatedAt 창(layer_span) 줄", rp3.contains("`build` 이미지 층 CreatedAt(docker history): 2026-09-09T13:40:00Z → \
2026-09-09T14:24:35Z · 층 28개") && rp3.contains("2026-09-09T13:40:00Z → 2026-09-09T14:24:35Z (layer-span)"));
    st.ck("R3 재현: 측정 뒤 재생성 형상이 있으면 render 단계 경고(지금의 렌더러 · 측정 env 관측과 대조)",
        rp3.contains("`render` 단계는 **지금의** 렌더러로") && rp3.contains("`output/multi/envs/.env.interconnect`"));
    st.ck("★R3 재현: 재생성 형상이 없으면 render 경고 없음(근거 없는 경고 ✗)", !reproduce_header.contains("지금의** 렌더러"));
    let me3 = rf(_f_measurement_env, &repo, &r3, tpls)?;
    let mn3 = rf(_f_measurement_env_nodes, &repo, &r3, tpls)?;
    let ts3 = rf(_f_tool_snapshots, &repo, &r3, tpls)?;
    st.ck("R3 측정 env 관측: 행 전부 · 값 코드 스팬(자리표시) · 빈 값 · 노드 미특정(표지) · 종류 · 횟수 · 값 주",
        me3.contains("측정 실행의 env 관측 6행") && me3.contains("| `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo |")
            && me3.contains("| `NCCL_DEBUG` | (빈 값) | 미특정(cluster) |") && me3.contains(&format!("{log_p}:L2")) && me3.contains("| 3줄 · 로그 2개 |")
            && me3.contains("- 값 주: 기계 치환"));
    st.ck("R3 노드별 env: 양 노드 같음은 이름만 · 다름 · 한쪽만 · 미특정은 행으로",
        mn3.contains("양 노드 같은 값 1개: `NCCL_IB_DISABLE`") && mn3.contains("| `NCCL_NET_GDR_LEVEL` | `SYS` | `LOC` | — | 다름 |")
            && mn3.contains("| `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | — | — | main 만 관측 |") && mn3.contains("노드 미특정 |"));
    let r3single = with(&r3, &[("identity", with(&r3["identity"], &[("topology", J::from("single"))]))]);
    st.ck("★R3 노드별 env: 값이 다른 키는 '같음' 목록에 들지 않는다 · single = 해당 없음",
        !mn3.split('\n').nth(2).unwrap_or("").contains("`NCCL_NET_GDR_LEVEL`")
            && rf(_f_measurement_env_nodes, &repo, &r3single, tpls)?.starts_with("해당 없음"));
    let r3r = with(&r3, &[("measurement_env_observed", j(&format!(r#"[
        {{"key": "NCCL_IB_HCA", "value": "=<nic:cluster>", "node": "sub", "source": "{log_p}:L2", "ray_repeated": 2, "seen_in_logs": 6}},
        {{"key": "NCCL_CROSS_NIC", "value": "1", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_ALGO", "value": "Ring", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_ALGO", "value": "Tree", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_ALGO", "value": "Tree", "node": "sub", "source": "{log_p}:L2"}},
        {{"key": "NCCL_ALGO", "value": "Ring", "node": "sub", "source": "{log_p}:L2"}},
        {{"key": "NCCL_PROTO", "value": "Simple", "node": "main", "source": "{log_p}:L1"}},
        {{"key": "NCCL_PROTO", "value": "Simple", "node": null, "node_label": "cluster", "source": "{log_p}:L3"}}]"#)))]);
    let me_r = rf(_f_measurement_env, &repo, &r3r, tpls)?;
    let mn_r = rf(_f_measurement_env_nodes, &repo, &r3r, tpls)?;
    let hca = mn_r.split('\n').find(|ln| ln.starts_with("| `NCCL_IB_HCA`")).unwrap_or("").to_string();
    st.ck("R3 측정 env: Ray 접힘은 관측 칸 + 주(접힌 사본의 노드 미관측) · 접힘 없는 행에는 없다",
        me_r.contains("| 로그 6개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |") && me_r.contains("- Ray 접힘: 출처 줄 끝의") && !me3.contains("Ray 접힘"));
    st.ck(&format!("★R3 노드별 env: Ray 가 접은 키는 '<노드> 만 관측'(다른 쪽 부재 단언)으로 적지 않는다({})", py_repr(&core::tail_chars(&hca, 60))),
        hca.contains("sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측") && !hca.contains("만 관측 |")
            && mn_r.contains("| `NCCL_CROSS_NIC` | `1` | — | — | main 만 관측 |"));
    st.ck("★R3 노드별 env: 같음 = 값 집합(노드마다 로그 순서가 달라도 같은 집합이면 '같은 값')",
        mn_r.contains("양 노드 같은 값 1개: `NCCL_ALGO`") && !mn_r.contains("다름"));
    st.ck("★R3 노드별 env: 한쪽 + 노드 미특정 줄이면 '만 관측' 으로 다른 쪽 부재를 단언하지 않는다",
        mn_r.contains("| `NCCL_PROTO` | `Simple` | — | `Simple` | main 관측 · 노드 미특정 줄 있음 — sub 여부 미관측 |"));
    let r3w = with(&r3r, &[("measurement_env_observed", j(&format!(r#"[
        {{"key": "NCCL_NET_PLUGIN", "value": "spcx", "node": "sub", "source": "{log_p}:L2",
         "log_window": "tail-800(lite_bench.sh@0df14da468f9 L125)"}},
        {{"key": "NCCL_CROSS_NIC", "value": "1", "node": "main", "source": "{log_p}:L1"}}]"#)))]);
    let me_w = rf(_f_measurement_env, &repo, &r3w, tpls)?;
    let mn_w = rf(_f_measurement_env_nodes, &repo, &r3w, tpls)?;
    st.ck("R3 측정 env: 로그 창(log_window)을 싣는다 · 창이 없으면 주도 없다",
        me_w.contains("- 로그 창(꼬리): `tail-800(lite_bench.sh@0df14da468f9 L125)`(꼬리 창 출처 로그 1개 · 이 표 행 #1)")
            && me_w.contains("창 밖에서 echo 한 줄은 이 표에 없다") && !me3.contains("로그 창"));
    st.ck("★R3 노드별 env: 꼬리 캡처 로그의 한쪽 관측 = '캡처 창에 남음 — 다른 쪽 미관측'(부재 단언 ✗) · 창 없는 로그는 '만 관측' 유지",
        mn_w.contains("| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |")
            && mn_w.contains("| `NCCL_CROSS_NIC` | `1` | — | — | main 만 관측 |"));
    let head_p = "output/multi/benchlog/sweep_x/lite_engine_head_x.log";
    let r3h = with(&r3r, &[("measurement_env_observed", j(&format!(r#"[
        {{"key": "NCCL_IB_HCA", "value": "=a", "node": "main", "source": "{head_p}:L19",
         "log_head_window": "head-600(output/multi/benchlog/sweep_x/lite_raw_x.json engine_log_head_lines)"}},
        {{"key": "NCCL_CROSS_NIC", "value": "1", "node": "main", "source": "{head_p}:L20",
         "log_head_window": "head-600(output/multi/benchlog/sweep_x/lite_raw_x.json engine_log_head_lines)"}},
        {{"key": "NCCL_NET_PLUGIN", "value": "spcx", "node": "sub", "source": "{log_p}:L2",
         "log_window": "tail-800(lite_bench.sh@0df14da468f9 L125)"}}]"#)))]);
    let me_h = rf(_f_measurement_env, &repo, &r3h, tpls)?;
    let mn_h = rf(_f_measurement_env_nodes, &repo, &r3h, tpls)?;
    st.ck("★P15 측정 env: 꼬리 창 = 그 창 행만(#3) · 머리 캡처 창 따로(#1–2 · head-600) · 꼬리 '이 표에 없다' 주의 ✗ · 가운데 공백 명시",
        me_h.contains("`tail-800(lite_bench.sh@0df14da468f9 L125)`(꼬리 창 출처 로그 1개 · 이 표 행 #3)")
            && me_h.contains("- 로그 창(머리): `head-600(output/multi/benchlog/sweep_x/lite_raw_x.json engine_log_head_lines)`(머리 캡처 출처 로그 1개 · 이 표 행 #1–2)")
            && me_h.contains("꼬리 창의 주의는 이 행에 걸리지 않는다") && !me_h.contains("창 밖에서 echo 한 줄은 이 표에 없다")
            && !me_h.contains("이 표의 출처 로그") && me_h.contains("머리 캡처와 꼬리 창 사이"));
    st.ck("★P15 노드별 env: 머리 캡처 행의 한쪽 관측도 '캡처 창에 남음(머리 캡처)'(부재 단언 ✗) · 머리말 = 머리·꼬리 캡처",
        mn_h.contains("| `NCCL_IB_HCA` | `=a` | — | — | main 만 캡처 창에 남음 — sub 여부 미관측(머리 캡처) |")
            && mn_h.contains("| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |")
            && mn_h.contains("머리·꼬리 캡처 로그(01 §1.4 로그 창)"));
    st.ck("★R3 측정 env · 도구 스냅샷 없음 = 미관측을 말한다(침묵 ✗)",
        rf(_f_measurement_env, &repo, &facts_json, tpls)?.contains("env 관측 없음")
            && rf(_f_measurement_env, &repo, &with(&facts_json, &[("measurement_env_observed", j(r#"[
                {"key": "VLLM_PLE_MMAP", "value": null, "kind": "key-present", "node": null, "source": "x.log:L44",
                 "value_note": "키 존재만 관측 · 값 미관측"}]"#))]), tpls)?.contains("(키 존재 · 값 미관측)")
            && !rf(_f_measurement_env, &repo, &with(&facts_json, &[("measurement_env_observed", j(r#"[
                {"key": "VLLM_PLE_MMAP", "value": null, "kind": "key-present", "node": null, "source": "x.log:L44"}]"#))]), tpls)?.contains("env 관측 없음")
            && rf(_f_tool_snapshots, &repo, &facts_json, tpls)?.contains("스냅샷 없음"));
    st.ck("R3 도구 스냅샷: 이름 · 역할 · 경로 · rev12(커밋 UTC) · 다음 변경 · 발췌 토큰 · git show 명령(전체 rev) · 이유 · 주",
        ts3.contains("`run_bench.sh@0123456789ab`") && ts3.contains("`0123456789ab` 2026-09-05T01:00:00Z") && ts3.contains("| bench |")
            && ts3.contains(&format!("`{}` 2026-09-14T02:00:00Z", "9".repeat(12)))
            && ts3.contains(&format!("`git show {snap_rev}:.claude/skills/adversarial-benchmark/scripts/run_bench.sh`"))
            && ts3.contains("- `run_bench.sh` — sweep_bench.sh 가 레벨마다 부른다") && ts3.contains("- 측정 시각의 워킹트리는 관측 대상 밖"));
    let ts_with = |extra: &[(&str, J)]| -> HR<String> {
        let t0 = r3["tool_snapshots"].at(0).clone();
        let mut kv: Vec<(&str, J)> = vec![("next_rev", J::Null), ("next_rev_utc", J::Null)];
        kv.extend(extra.iter().cloned());
        rf(_f_tool_snapshots, &repo, &with(&r3, &[("tool_snapshots", J::List(vec![with(&t0, &kv)]))]), tpls)
    };
    let ts_nx = ts_with(&[])?;
    st.ck("★R3 도구 스냅샷: 다음 변경 None = '관측 없음' + 뜻(옛 '없음' = 변경 없음 단언 ✗) · 다음 변경이 있으면 그 주도 없다",
        ts_nx.contains("| 관측 없음 |") && ts_nx.contains("'도구가 바뀌지 않았다' 는 뜻이 아니다") && !ts_nx.contains("| 없음 |") && !ts3.contains("관측 없음"));
    st.ck("★R3 도구 스냅샷: `다시 얻기` 는 원격 도달을 약속하지 않는다(그 커밋을 가진 클론의 명령이라고 적는다)",
        ts3.contains("원격 도달은 이 표가 판정하지 않았다"));
    let ts_same = ts_with(&[("next_rev_basis", J::from("0123456789ab..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트)"))])?;
    let ts_nd = ts_with(&[("next_rev_basis", J::from("미관측 — 지금 HEAD 가 측정 시점 커밋 0123456789ab 의 후손이 아니다(다른 브랜치에서 발행)"))])?;
    st.ck("★R3 도구 스냅샷: next_rev_basis 가 있으면 그 뜻을 싣는다(변경 없음 대 후손 아님 — 두 칸이 갈린다 · 일반 '관측 없음' 주 ✗)",
        ts_same.contains("| 0123456789ab..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) |")
            && ts_nd.contains("후손이 아니다(다른 브랜치에서 발행) |") && !format!("{ts_same}{ts_nd}").contains("| 관측 없음 |")
            && !ts_same.contains("'도구가 바뀌지 않았다' 는 뜻이 아니다"));
    let vrows = _vllm_observed_rows(&r3["naming"]["vllm_observed"])?;
    let r0: Vec<(String, String)> = vrows.iter().take(3).map(|(a, b, _)| (a.clone(), b.clone())).collect();
    let r0g = |k: &str| r0.iter().rev().find(|(a, _)| a == k).map(|(_, b)| b.clone());
    let mut rsrc: Vec<(String, String)> = Vec::new();
    for (a, _, c) in &vrows {
        match rsrc.iter_mut().find(|(k, _)| k == a) {
            Some(e) => e.1 = c.clone(),
            None => rsrc.push((a.clone(), c.clone())),
        }
    }
    let rs = |k: &str| rsrc.iter().find(|(a, _)| a == k).map(|(_, c)| c.clone()).unwrap_or_default();
    let cert_l = "인증서 `vllm_version`(강한 일치 키)";
    let eng_l = "vLLM 엔진 자기보고(엔진 로그 기동 배너)";
    let r0_repr = J::Dict(Map(r0.iter().map(|(a, b)| (a.clone(), J::from(b.as_str()))).collect())).repr();
    st.ck(&format!("R3 0.4 vLLM 관측: 키마다 생산자 · 인증서 값을 엔진 자기보고로 적지 않는다({r0_repr})"),
        r0g(eng_l).as_deref() == Some(_MISSING_TEXT) && r0g(cert_l).as_deref() == Some("0.29.0")
            && rs(cert_l).contains("IMAGE_TAG") && rs(cert_l).starts_with("생산자 `image-tag-line` · sweep_bench.sh@8ca23d4")
            && rs(eng_l).starts_with("같은 digest") && rs(eng_l).ends_with("지위 `unobserved`") && rsrc.len() == 3);
    st.ck("★R3 0.4 vLLM 관측: 빈 값에 옛 단일 source(있는 값의 출처)를 붙이지 않는다", rs("wheel 메타(`vllm_build`)") == _MISSING_TEXT);
    let cx3 = rf(_f_context, &repo, &r3, tpls)?;
    st.ck("★R3 0.2 라벨: 측정 강식별 키(옛 '엔진 자기보고 vLLM' ✗)", !cx3.contains("엔진 자기보고 vLLM") && cx3.contains("vLLM(측정 강식별 키 `vllm_version`)"));
    let mut r3v = jc(&r3);
    *pm(&mut r3v, &["naming", "vllm_observed", "certificate_vllm_version"]) = J::from("0.30.0");
    st.ck("★R3 0.2 vLLM 행: 같은 값이면 0.4 의 생산자(image-tag-line)를 가리키고 · 값이 다르면 가리키지 않는다(거짓 출처 ✗)",
        cx3.contains("생산자 `image-tag-line`(0.4 표") && !rf(_f_context, &repo, &r3v, tpls)?.contains("생산자 `image-tag-line`"));
    let legend = _legend_md()?;
    let csm = CURRENT_STATUS_MEANINGS().as_dict().cloned().unwrap_or_default();
    let t02t = &tp(tpls, "02-narrative").text;
    st.ck("R3 범례 = CURRENT_STATUS_MEANINGS(원래 주장의 지위) · 02 §2.4 PROMPT 도 같은 뜻",
        csm.iter().all(|(k, v)| legend.contains(&format!("**{k}**({})", v.display()))) && legend.contains("원래 주장")
            && csm.keys().all(|k| t02t.contains(k)) && t02t.contains("현재지위 = **원래 주장**"));
    let p35 = fields_of(tpls, "03-benchmark", "3.5");
    let p35g = p35.get_str("기재").unwrap_or("");
    st.ck("★R3 3.5: acceptance length = 출력(비로 비교) · spec 켜짐/꺼짐 = 조건 · 이전 측정 레벨 전부 나란히(옛 '상태와 acceptance length' ✗)",
        p35g.contains("acceptance length 는 비교 조건이 아니라 측정 결과(출력)다") && p35g.contains("spec decode 켜짐/꺼짐과 방식")
            && !p35g.contains("spec decode 상태와 acceptance length") && p35g.contains("레벨 전부"));
    let mech = [fields_of(tpls, "00-hint", "0.2"), fields_of(tpls, "02-narrative", "2.5")];
    st.ck("★R3 0.2 · 2.5: 버티는 기전 대 실패 기전 + 지위(확정 · 가설(강등) · 미결)", mech.iter().all(|fl|
        fl.get_str("기재").unwrap_or("").contains("확정 · 가설(강등) · 미결") && fl.get_str("질문").unwrap_or("").contains("기전")));
    st.ck("★R3 2.4: 교정 묶음은 이 셀에 영향을 준 항목을 id 로",
        fields_of(tpls, "02-narrative", "2.4").get_str("기재").unwrap_or("").contains("항목을 **id 로** 나열한다"));
    let sub_fn = |t: &str| -> HR<String> { Ok(t.replace("RoCE", "<interconnect>")) };
    let ssub: Sub = &sub_fn;
    let hk_sub = TemplateHooks { substitute: Some(ssub), ..hooks };
    let st_txt = substituted_source_with(&repo, &payload, "plan_26090918", &lineage, &J::Null, None, &hk_sub)?;
    let sub_line = st_txt.split('\n').find(|ln| ln.contains("<interconnect>")).unwrap_or("").to_string();
    let first_ex = "> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.";
    let rtp = variant("rt", &ed("02-narrative.md", first_ex, &format!("{first_ex}\n> {sub_line}")))?;
    let rtc = kwf.clone().sub(Some(ssub)).codes(&repo, &rtp)?;
    st.ck("발췌 도우미(치환 후 원문)에서 옮긴 줄은 무결성 통과", !sub_line.is_empty() && !rtc.contains("HINT_EXCERPT_MISMATCH") && !rtc.contains("HINT_PII"));
    let wrong = variant("rtw", &ed("02-narrative.md", first_ex, &format!("{first_ex}\n> {}", sub_line.replace("<interconnect>", "<host>"))))?;
    st.ck("★자리표시를 추측하면(도우미 미사용) 무결성 불일치", kwf.clone().sub(Some(ssub)).codes(&repo, &wrong)?.contains("HINT_EXCERPT_MISMATCH"));
    st.ck("★발췌 도우미: LINEAGE 밖 출처", raises(substituted_source_with(&repo, &payload, "testlog_29999999", &lineage, &J::Null, None, &hk_sub),
        "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"));
    let hk_nosub = TemplateHooks { substitute: None, ..hooks };
    st.ck("★발췌 도우미: 치환 함수 부재", raises(substituted_source_with(&repo, &payload, "plan_26090918", &lineage, &J::Null, None, &hk_nosub),
        "HINT_EXCERPT_VERIFIER_ABSENT"));
    st.ck("발췌 도우미: artifacts 파일명",
        substituted_source_with(&repo, &payload, "60-fixture-patch.sh", &lineage, &J::Null, None, &hk_sub)?.contains("PLE 게이트 완화"));
    let half = variant("half", &ed("03-benchmark.md", "## 3.5 like-with-like\n", "## 3.5 like-with-like\n<<AGENT: 미저작>>\n"))?;
    st.ck("★봉인 거부 = 부수효과 0(앞 문서도 봉인하지 않는다)", raises(seal_prompts(&half, &all_docs), "HINT_AGENT_PLACEHOLDER_RESIDUE")
        && rt(&half.join("00-hint.md"))?.contains("<!-- PROMPT"));

    // update_facts(continue 의 승인·슬롯 사유 반영) — 기계 사실 블록만 다시 쓰고 lint 는 그대로 통과한다
    let uf = variant("uf", &noop())?;
    copytree(default_snapshot_path(&payload)?.parent().unwrap(), &uf.parent().unwrap().join("inputs"))?;
    let before_00 = rt(&uf.join("00-hint.md"))?;
    let new_appr = j(r#"{"approved_by": "사용자(발화 전사) — 갱신", "approved_utc": "2026-09-21T11:11:11Z", "source": "cli:--approved-by"}"#);
    let chg = crate::jd! {"approval" => new_appr.clone()};
    let ch = update_facts(&repo, &uf, &chg, None)?;
    let after_00 = rt(&uf.join("00-hint.md"))?;
    st.ck(&format!("update_facts — 승인 변경은 meta 블록만 다시 쓴다({})", J::from(ch.clone()).repr()), ch == strs(&["meta"])
        && after_00.contains("사용자(발화 전사) — 갱신")
        && before_00.split("<!-- FACT:meta -->").next() == after_00.split("<!-- FACT:meta -->").next());
    st.ck("update_facts — 멱등(같은 변경 = 쓰기 0)", update_facts(&repo, &uf, &chg, None)?.is_empty());
    let p01 = uf.join("01-artifacts.md");
    let fresh = rt(&p01)?;
    let new_intro = fresh.lines().find(|l| l.contains("재현 입력과 확인되지 않은 참고 형상도 포함될 수 있다"))
        .ok_or_else(|| HintError::py("StopIteration", ""))?.to_string();
    let old_intro = "> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.";
    let authored = format!("독립 저작 문장 보존\n{old_intro}\n```\n{old_intro}\n```");
    wt(&p01, &format!("{}\n{authored}\n", fresh.replace(&new_intro, old_intro)))?;
    let migrated = update_facts(&repo, &uf, &chg, None)?;
    let after_migrate = rt(&p01)?;
    st.ck("refacts migrates exact old fixed introduction without changing author prose even with unchanged facts",
        migrated == strs(&["01-artifacts:fixed-introduction"]) && after_migrate.contains(&new_intro) && count(&after_migrate, old_intro) == 2
            && after_migrate.ends_with(&format!("{authored}\n")) && update_facts(&repo, &uf, &chg, None)?.is_empty());
    wt(&p01, &fresh.replacen(&new_intro, "저자가 바꾼 도입", 1))?;
    st.ck("ambiguous fixed introduction fails closed", raises(update_facts(&repo, &uf, &J::dict(), None), "HINT_SLOT_INTRO_AMBIGUOUS"));
    wt(&p01, &fresh)?;
    let fence_h3 = variant("h3", &ed("01-artifacts.md", "### 60-fixture-patch.sh", "```\n### 60-fixture-patch.sh\n```"))?;
    st.ck("★subsections: 코드 펜스 안 `###` 는 소제목이 아니다", subsections(&fence_h3, "01-artifacts", "1.2")?.is_empty());
    let uf_issues = kw.run(&repo, &uf)?;
    st.ck(&format!("update_facts 뒤 lint 통과(스냅샷 = 새 사실): {}", hd(&uf_issues, 3)), uf_issues.is_empty());
    st.ck("★update_facts 없이 스냅샷만 바꾸면 사실 드리프트(기계 블록은 스냅샷에서 재생성된다)", kw.clone().f(&with(&facts_json,
        &[("approval", with(&new_appr, &[("approved_by", J::from("다른 발화"))]))])).codes(&repo, &uf)?.contains("HINT_FACT_DRIFT"));

    // 봉인
    let sealed_dir = variant("sealed", &noop())?;
    copytree(default_snapshot_path(&payload)?.parent().unwrap(), &sealed_dir.parent().unwrap().join("inputs"))?;
    let b0 = brief(&sealed_dir)?;
    seal_prompts(&sealed_dir, &all_docs)?;
    seal_prompts(&sealed_dir, &all_docs)?;
    let mut stx: Vec<(String, String)> = Vec::new();
    for n in PAYLOAD_DOCS {
        stx.push((n.to_string(), rt(&sealed_dir.join(n))?));
    }
    let s00 = stx[0].1.clone();
    let qline = format!("{SEALED_PREFIX}이 셀은 무엇을 서빙했고");
    st.ck("봉인 = PROMPT 0 · 질문 줄", stx.iter().all(|(_, t)| !t.contains("<!-- PROMPT")) && s00.contains(&qline));
    st.ck("봉인 멱등 · brief 불변", brief(&sealed_dir)? == b0 && b0.lines().count() == 3);
    let s00l: Vec<&str> = s00.split('\n').collect();
    st.ck("봉인 줄 뒤 빈 줄(인용문 게으른 계속 방지)", s00.contains(&qline)
        && s00l.iter().zip(s00l.iter().skip(1)).all(|(ln, nxt)| ln.trim().is_empty() || nxt.trim().is_empty() || !ln.starts_with(SEALED_PREFIX))
        && count(&rt(&sealed_dir.join("00-hint.md"))?, SEALED_PREFIX) == 7);
    let sealed_issues = kw.clone().sealed().run(&repo, &sealed_dir)?;
    st.ck(&format!("봉인 뒤 lint 통과: {}", hd(&sealed_issues, 3)), sealed_issues.is_empty());
    st.ck("★봉인 뒤 PROMPT 재삽입", kw.clone().sealed().codes(&repo, &variant("rs", &noop())?)?.contains("HINT_PROMPT_RESIDUE"));
    let mut pre_ok = true;
    for n in PAYLOAD_DOCS {
        pre_ok &= rt(&payload.join(n))?.contains("<!-- SELFCHECK");
    }
    st.ck("봉인 = SELFCHECK 0(저작 자기점검은 배포되지 않는다) · 봉인 전에는 있다", stx.iter().all(|(_, t)| !t.contains("SELFCHECK"))
        && pre_ok && !s00.split("## 0.1").next().unwrap_or("").contains("\n\n\n"));
    let prompt_field = Re::new(r"^([^\s:][^:]*?):\s*(.*)$", 0);
    let receiver_zip = |p: &Path| -> HR<(bool, String)> {
        for n in TEMPLATE_FILES {
            let text = rt(&p.join(format!("{n}.md")))?;
            let rows: Vec<&str> = text.split('\n').collect();
            if text.contains("<!-- PROMPT") || text.contains("SELFCHECK") || text.contains(AGENT_MARK) {
                return Ok((false, format!("{n}: PROMPT · SELFCHECK · <<AGENT 잔존")));
            }
            for tpp in &tp(tpls, n).prompts {
                if !_condition_holds(&_directives(&tpp.fields)?["condition"], &facts_json)?.0 {
                    continue;
                }
                let q = format!("{SEALED_PREFIX}{}", _question(&tpp.fields)?);
                if rows.iter().filter(|r| **r == q).count() != 1 {
                    return Ok((false, format!("{n} §{}: 질문 줄이 정확히 1개가 아니다", tpp.chapter)));
                }
                let leak: Vec<&String> = tpp.raw.iter().filter(|ln| {
                    prompt_field.match_at_start(ln).and_then(|m| group(&m, 1).map(|g| PROMPT_KEYS.contains(&g))).unwrap_or(false)
                        && rows.contains(&ln.as_str())
                }).collect();
                if let Some(l) = leak.first() {
                    return Ok((false, format!("{n} §{}: 지시 필드 줄 잔존 {}", tpp.chapter, py_repr(&core::head_chars(l, 40)))));
                }
            }
        }
        Ok((true, String::new()))
    };
    let rz = receiver_zip(&sealed_dir)?;
    st.ck(&format!("★R3 수신자 zip = 봉인: 템플릿 PROMPT 마다 질문 줄 1개 · 지시 필드 0 · SELFCHECK 0({})", rz.1), rz.0);
    st.ck("★R3 수신자 zip 술어의 음성대조: 봉인 전 payload 는 같은 술어가 거부한다", !receiver_zip(&payload)?.0);
    let sc_back = root.join("v").join("sc-back").join("payload");
    copytree(&sealed_dir, &sc_back)?;
    wt(&sc_back.join("03-benchmark.md"), &format!("{}\n<!-- SELFCHECK\n1. 수치의 귀속\n-->\n", rt(&sc_back.join("03-benchmark.md"))?))?;
    st.ck("★봉인 뒤 SELFCHECK 재삽입 = HINT_SELFCHECK_RESIDUE", kwf.clone().sealed().codes(&repo, &sc_back)?.contains("HINT_SELFCHECK_RESIDUE"));
    let one = root.join("v").join("one-line").join("payload");
    copytree(&sealed_dir, &one)?;
    wt(&one.join("02-narrative.md"), &format!("{}\n<!-- PROMPT 질문: 한 줄 -->\n", rt(&one.join("02-narrative.md"))?))?;
    st.ck("★봉인 뒤 한 줄짜리 PROMPT 주석(branch.PROMPT_RESIDUE_RE 와 같은 판정)", kwf.clone().sealed().codes(&repo, &one)?.contains("HINT_PROMPT_RESIDUE"));
    st.ck("봉인 뒤에도 CLI --verify PASS", cli_verify(&sealed_dir, &repo.join(rep_p)) == 0);

    // map_only · single 토폴로지(1.5 조건 불성립) · 기준 부재 full 셀(OBSERVATION-ONLY) 양성
    let mut mo = jc(&base_facts);
    mo.set("task_class", "hint_map_only");
    mo.set("sub_recipe", J::Null);
    mo.set("identity", with(&base_facts["identity"], &[("topology", J::from("single")), ("tp", J::Int(1))]));
    mo.set("publication", with(&base_facts["publication"], &[("task_class", J::from("hint_map_only"))]));
    let pmo = scaffold("maponly", &mo)?;
    st.ck("single: 1.5 해당 없음 · PROMPT 없음", rt(&pmo.join("01-artifacts.md"))?.contains("해당 없음 — 이 절의 조건 `topology=multi`")
        && !prompts(&pmo)?.iter().any(|p| p["section"] == J::from("1.5")));
    fill(&pmo, &mo)?;
    refresh(&pmo)?;
    let mkw = kw.clone().man(j(r#"{"task_class": "hint_map_only"}"#));
    let ml = mkw.run(&repo, &pmo)?;
    st.ck(&format!("map_only lint 통과: {}", hd(&ml, 3)), ml.is_empty());
    let dst = root.join("v").join("mo-strip").join("payload");
    copytree(&pmo, &dst)?;
    wt(&dst.join("00-hint.md"), &rt(&pmo.join("00-hint.md"))?.replace(MAP_ONLY_MARKER, "OBSERVATION"))?;
    st.ck("★OBSERVATION-ONLY 부재(map_only)", mkw.clone().f(&jc(&mo)).codes(&repo, &dst)?.contains("HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING"));
    let t = rt(&dst.join("00-hint.md"))?;
    wt(&dst.join("00-hint.md"), &format!("{t}\n<!-- {MAP_ONLY_MARKER} -->\n"))?;
    st.ck("★주석 속 OBSERVATION-ONLY 는 마커가 아니다", mkw.clone().f(&jc(&mo)).codes(&repo, &dst)?.contains("HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING"));
    let mut ob = jc(&base_facts);
    let obm = merge(&ob["measurement"], &j(&format!(r#"{{"verdict": "{MAP_ONLY_MARKER}",
        "sources": {{"verdict": "sweep verdict.json(fixture) (NEEDS_RUBRIC → OBSERVATION-ONLY · 기준 c·E·expected 부재)"}}}}"#)));
    ob.set("measurement", obm);
    let po = scaffold("observation", &ob)?;
    fill(&po, &ob)?;
    refresh(&po)?;
    let okw = kw.clone().man(j(&format!(r#"{{"task_class": "full_benchmark", "benchmark": {{"verdict": "{MAP_ONLY_MARKER}"}}}}"#)));
    let t_o = rt(&po.join("00-hint.md"))?;
    let ol = okw.run(&repo, &po)?;
    st.ck(&format!("기준 부재 full 셀: 00 머리 OBSERVATION-ONLY 배너(판정 주장 ✗) · lint 통과: {}", hd(&ol, 3)),
        t_o.contains(&format!("> **{MAP_ONLY_MARKER}** — 이 셀은 **기준(")) && !t_o.contains(&format!("> **{REFUTE_MARKER}**")) && ol.is_empty());
    let dst = root.join("v").join("ob-strip").join("payload");
    copytree(&po, &dst)?;
    wt(&dst.join("00-hint.md"), &t_o.replace(MAP_ONLY_MARKER, "OBSERVATION"))?;
    st.ck("★OBSERVATION-ONLY 부재(기준 부재 full 셀)", okw.clone().f(&jc(&ob)).codes(&repo, &dst)?.contains("HINT_OBSERVATION_ONLY_MARKER_MISSING"));
    st.ck("★기존 저작물 덮지 않음", raises(render_scaffold(&repo, &base_facts, &payload, None), "HINT_SCAFFOLD_EXISTS"));
    let bad_t = with(&base_facts, &[("tag", J::from("not-a-hint"))]);
    st.ck("★facts.tag 모양", raises(render_scaffold(&repo, &bad_t, &root.join("bt").join("payload"), None), "HINT_FACTS_SHAPE"));
    let no_utc = with(&base_facts, &[("generated_utc", J::from("2026-09-21 10:21"))]);
    st.ck("★주입 시각 아님", raises(render_scaffold(&repo, &no_utc, &root.join("nu").join("payload"), None), "HINT_TIME_NOT_INJECTED"));
    // ── v7(2026-09-29 · plan_26092908 §4.1~§4.6) ──
    let mut rfx = jc(&base_facts);
    {
        let m = pm(&mut rfx, &["measurement"]);
        m.set("verdict", REFUTE_MARKER);
        m.set("verdict_reasons", j(r#"["decode 20.98 < floor 25.33"]"#));
        m.set("sources", j(r#"{"verdict": "sweep verdict.json(fixture)", "decode_tps_conc1": "sweep verdict.json(fixture)"}"#));
    }
    let g = rf(_f_grade, &repo, &rfx, tpls)?;
    st.ck("판정 행: FACT:grade 에 `REFUTE` + 출처 · 00 머리 REFUTE 배너(사유 포함)", g.contains("| 성능 판정(measurement.verdict) | `REFUTE` — 출처 \
sweep verdict.json(fixture) |") && g.contains(&format!("> **{REFUTE_MARKER}** —")) && g.contains("floor 25.33"));
    st.ck("판정 행: 판정 원천 없으면 '미관측 — <사유>'(침묵 ✗)", rf(_f_grade, &repo, &facts_json, tpls)?.contains("성능 판정(measurement.verdict) | 미관측 —"));
    let bmr = rf(_f_bench_missing, &repo, &rfx, tpls)?;
    st.ck("★비발행 판정(REFUTE) = 무인증서 결손 배너 ✗ · 비발행 설명 줄", bmr.contains("인증서 비발행 판정(결손 아님)") && !bmr.contains("인증서가 없다"));
    let mt = rf(_f_measurement, &repo, &rfx, tpls)?;
    st.ck("측정 표: 키별 출처는 출처 열(sources.<키> 행 ✗)", mt.contains("| `verdict` | REFUTE | sweep verdict.json(fixture) |") && !mt.contains("`sources."));
    st.ck("README 판정 줄", _render_readme(&ctx(&repo, &rfx, tpls))?.contains("> **판정** `REFUTE`"));
    let mut nf = jc(&base_facts);
    nf.set("base_tag", nf["tag"].clone());
    nf.set("tail_candidates", j(r#"[{"axis": "graph", "token": "eager", "meaning": "실행 모드 eager", "evidence": null, "source": "yaml"}]"#));
    let nft = rf(_f_name_tail, &repo, &nf, tpls)?;
    st.ck("0.9 후보 표(확정 전) · 기본 이름 · 근거 미특정 표기", nft.contains("**기본 이름**(꼬리 전)") && nft.contains("`eager`") && nft.contains("근거 미특정"));
    let mut cf = jc(&nf);
    *pm(&mut cf, &["naming", "tail"]) = j(r#"[{"token": "eager", "meaning": "eager 실행", "evidence": {"file": "c.yaml", "key": "enforce-eager",
                                                                                 "value": "true"}}]"#);
    *pm(&mut cf, &["naming", "timestamp"]) = J::from("t2609291230");
    cf.set("tag", format!("{}-eager-t2609291230", nf["tag"].display()));
    let nt = rf(_f_name_tail, &repo, &cf, tpls)?;
    let ctx_b = rf(_f_context, &repo, &cf, tpls)?;
    st.ck("0.9 확정 꼬리(토큰 · 뜻 · 근거 · timestamp) · 0.2 명명 축에 꼬리 · timestamp 행 · ple·spec·graph 결정론 행 ✗",
        nt.contains("**확정 이름**") && nt.contains("`enforce-eager` = `true`") && nt.contains("t2609291230") && ctx_b.contains("꼬리 `eager`")
            && ctx_b.contains("| timestamp |") && ctx_b.contains("결정론부(vllm · model · arch · q · len · kv") && !ctx_b.contains("전량 파생했다"));
    let ng = name_guide_md(&cf)?;
    st.ck("README 이름 읽는 법: 세그먼트 · 꼬리 뜻 · timestamp · native · -bare · 세대 표(v7 · v6 · 옛 5·4)",
        ["`<vllm>`", "꼬리 `eager`", "t2609291230", "`native` = **실제 하드웨어", "`-bare` = **Docker 없이**", "| `v7` |", "| `v6` |", "옛 5세그먼트", "옛 4세그먼트"]
            .iter().all(|s| ng.contains(s)));
    let mut mf = jc(&base_facts);
    mf.set("applied_set", J::Null);
    mf.set("sub_recipe", J::Null);
    mf.set("missing", j(r#"["HINT_MISSING_APPLIED_SET", "HINT_MISSING_SUB_RECIPE", "HINT_MISSING_NATIVE_LAUNCH"]"#));
    mf.set("missing_reasons", j(r#"{"HINT_MISSING_NATIVE_LAUNCH": "producer native_launch_error: OSError: disk full"}"#));
    let fm = rf(_f_missing, &repo, &mf, tpls)?;
    st.ck("01 결손 문구 = 결손 표에서 파생(적용 집합 · 서브 레시피 — 코드 · 뜻) · 결손 표 '이 셀의 사유' 열",
        rf(_f_applied_set, &repo, &mf, tpls)?.contains("`HINT_MISSING_APPLIED_SET`") && rf(_f_sub_recipe, &repo, &mf, tpls)?.contains("`HINT_MISSING_SUB_RECIPE`")
            && fm.contains("| 코드 | 뜻 | 이 셀의 사유 |") && fm.contains("disk full"));
    st.ck("★결손 코드가 없으면 01 이 '결손' 이라 말하지 않는다(침묵 ≠ 없음 — 부재 사실만)",
        !rf(_f_applied_set, &repo, &with(&mf, &[("missing", J::List(vec![]))]), tpls)?.contains("결손 `"));
    let mut qf = jc(&base_facts);
    *pm(&mut qf, &["identity", "quant_composition"]) = j(r#"[{"scope": "quantization_config 선언 대상", "dtype": "fp8", "source": "config.json"},
                                          {"scope": "routed experts", "dtype": "fp4", "source": "config.json expert_dtype"}]"#);
    *pm(&mut qf, &["build", "driver_conflict"]) = j(r#"{"observed": "580.178.04", "declared": "580.173.02", "sources": {}}"#);
    *pm(&mut qf, &["slots", "build_patch_pre", "file_records"]) = j(r#"[
        {"path": "artifacts/build_patch_pre/60-fixture-patch.sh", "sha256": "@064@", "verification": "verified",
         "generated_by": "cell-run", "verification_basis": "빌드 원장", "relevance": "required", "relevance_basis": "① ∋ ② 발화"}]"#);
    let qf = qf;
    let rs_ = rf(_f_resolved, &repo, &qf, tpls)?;
    st.ck("0.4 양자화 구성 표(선언 fp8 + routed expert fp4) · 필요한 패치 요약(required 목록) · 드라이버 관측 ≠ 선언 행",
        rs_.contains("| routed experts | `fp4` |") && rs_.contains("required `60-fixture-patch.sh`") && rs_.contains("⚠ 드라이버 관측 ≠ 선언"));
    let mut af = jc(&base_facts);
    for sn in ["build_patch_pre", "build_patch_post"] {
        if let Some(s) = af.get_mut("slots").and_then(|s| s.get_mut(sn)) {
            if s.is_dict() {
                s.set("file_records", J::List(vec![]));
            }
        }
    }
    af.set("applied_set", j(r#"{"status": "observed", "source": "attestation:output/multi/benchlog/attestation_x.json#main",
                         "patches": [{"phase": "inline", "file": "strip-hoist", "result": "skipped", "result_source": "build-ledger"}],
                         "node_asymmetry": {"representative": "main", "note": "노드별 원장이 다르다(픽스처)", "nodes": {
                             "main": {"source": "attestation:x#main", "patches": [
                                 {"phase": "inline", "file": "strip-hoist", "result": "skipped"}]},
                             "sub": {"source": "attestation:x#sub", "patches": [
                                 {"phase": "pre", "file": "50-port.sh", "result": "skipped"},
                                 {"phase": "post", "file": "10-deepgemm.sh", "result": "applied"}]}}}}"#));
    let rq = _required_patches_line(&af)?;
    let aq = rf(_f_applied_set, &repo, &af, tpls)?;
    st.ck("★F2 00 §0.4: 대표(main) 원장 기준임을 밝히고 노드별 원장(sub applied/skipped)을 붙인다 · '필요 판정 아님'",
        rq.contains("대표(main) 측정 이미지 원장 기준 실린 빌드 패치 없음") && rq.contains("**노드 비대칭**")
            && rq.contains("sub 원장: applied 10-deepgemm.sh") && rq.contains("sub 원장: skipped 50-port.sh") && rq.contains("필요 판정이 아니다"));
    st.ck("★F2 01 §1.1 적용 집합: 노드별 원장 표(노드마다 행 · 출처 #노드)",
        aq.contains("**노드별 원장**") && aq.contains("| `sub` | post | `10-deepgemm.sh` | applied | `attestation:x#sub` |")
            && aq.contains("| `main` | inline | `strip-hoist` | skipped |"));
    pop(pm(&mut af, &["applied_set"]), "node_asymmetry");
    st.ck("★F2 음성대조: 비대칭 표지가 없으면 종전 문장 · 노드별 표 없음",
        _required_patches_line(&af)?.starts_with("**이 모델에 필요한 패치**: 실린 빌드 패치 없음")
            && !rf(_f_applied_set, &repo, &af, tpls)?.contains("**노드별 원장**"));
    let mut xf = jc(&base_facts);
    xf.set("build_context_map", j(r#"[{"slot_path": "artifacts/build_patch_pre/60-fixture-patch.sh",
                                "context_path": "build_patches_src/60-fixture-patch.sh", "basis": "Dockerfile COPY build_patches_src/"},
                               {"slot_path": "artifacts/build_recipe/pip-freeze-main.txt", "context_path": null, "basis": "입력 아님"}]"#));
    xf.set("env_shapes", j(r#"[{"file": "envs/.env.interconnect", "template": "artifacts/compose/.env.interconnect.template", "derived": [
        {"field": "interconnect.nccl_transport", "keys": ["NCCL_IB_DISABLE", "NCCL_NET"],
         "placeholders": {"NCCL_IB_DISABLE": "<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>",
                          "NCCL_NET": "<derived:interconnect.nccl_transport→NCCL_NET>"},
         "rule_text": "rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB", "source": "render_dockerfile.NCCL_TRANSPORTS"}]}]"#));
    let xf = xf;
    let rp_ = rf(_f_reproduce, &repo, &xf, tpls)?;
    st.ck("01 §1.4: 슬롯 → 빌드 컨텍스트 매핑 표(입력 아님 = —) · env 파생 키 자리표시 · 실효값 규칙",
        rp_.contains("| `artifacts/build_patch_pre/60-fixture-patch.sh` | `build_patches_src/60-fixture-patch.sh` |")
            && rp_.contains("| `artifacts/build_recipe/pip-freeze-main.txt` | — |") && rp_.contains("<derived:interconnect.nccl_transport→NCCL_NET>")
            && rp_.contains("rdma → NCCL_IB_DISABLE=0"));
    let mut xf13 = jc(&xf);
    pm(&mut xf13, &["build_context_map"]).push(j(r#"{"slot_path": null, "context_path": "build_patches/", "create": "empty-dir",
                                      "inventory": [".gitkeep"], "basis": "Dockerfile COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리"}"#));
    let mut xf15 = jc(&xf13);
    xf15.set("build_context_map", j(r#"[
        {"slot_path": "artifacts/build_recipe/Dockerfile.source-build", "context_path": "Dockerfile.source-build",
         "context_root": "output/single", "basis": "docker build -f output/single/Dockerfile.source-build · 컨텍스트 output/single"},
        {"slot_path": "artifacts/triplet/c.yaml", "context_path": "configs/c.yaml", "basis": "compose volumes ./configs:/app/configs"},
        {"slot_path": null, "context_path": "build_patches/", "create": "empty-dir", "context_root": "output/single",
         "basis": "빈 디렉터리"}]"#));
    let rp15 = rf(_f_reproduce, &repo, &xf15, tpls)?;
    let sm15 = _context_map_summary(&xf15)?;
    st.ck("★NVFP4 F15 레시피 통로가 다르면 매핑 표 · README 가 빌드 레시피 · 빌드 패치 행에 그 컨텍스트 뿌리(2단계와 같은 자리) · 트리플렛은 종전",
        rp15.contains("| `artifacts/build_recipe/Dockerfile.source-build` | `output/single/Dockerfile.source-build` |")
            && rp15.contains("| `artifacts/triplet/c.yaml` | `configs/c.yaml` |") && rp15.contains("다른 컨텍스트 `output/single/`")
            && rp15.contains("`output/single/build_patches/`") && sm15.contains("`output/single/`") && sm15.contains("다른 컨텍스트 `output/single/`")
            && sm15.contains("`configs/`"));
    let mut xf15_new = jc(&xf15);
    if let Some(l) = pm(&mut xf15_new, &["build_context_map"]).as_list_mut() {
        for row in l.iter_mut() {
            if row["context_root"].truthy() {
                row.set("context_scope", "reconstruction_example");
            }
        }
    }
    let rp_new = rf(_f_reproduce, &repo, &xf15_new, tpls)?;
    let sm_new = _context_map_summary(&xf15_new)?;
    st.ck("new context scope is example-only while old snapshots retain historical wording",
        rp_new.contains("재구성 예시 통로 `output/single/`") && sm_new.contains("재구성 예시 통로 `output/single/`")
            && rp_new.contains("별도 실행 기록 확인") && !rp_new.contains("이 이미지의 레시피 통로")
            && rp15.contains("이 이미지의 레시피 통로") && sm15.contains("이 이미지의 레시피 통로"));
    let sm13 = _context_map_summary(&xf13)?;
    st.ck("F13 빈 디렉터리 생성 행 = 01 매핑 표 · README 요약 둘 다",
        rf(_f_reproduce, &repo, &xf13, tpls)?.contains("| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` |")
            && sm13.contains("빈 디렉터리로 만들 컨텍스트 자리") && sm13.contains("`build_patches/`"));
    let sl15 = j(r#"{"build_recipe": {"files": ["artifacts/build_recipe/Dockerfile.source-build"], "evidence": {"recipe_vs_image": [
        {"path": "output/single/Dockerfile.source-build", "checkout_path": "output/multi/Dockerfile.source-build",
         "last_commit_utc": "2026-10-14T03:07:26Z", "last_commit_kind": "실린 개정 커밋", "image_created_utc": "2026-10-14T01:45:19Z",
         "warning": "실린 바이트(git:x)가 … 전수 일치하지 않는다(16/17)"}]}}}"#);
    let n15 = rf(_f_slots, &repo, &with(&xf, &[("slots", sl15)]), tpls)?;
    st.ck("F15 레시피 경고 줄 = 실린 파일 경로 · 실린 개정 커밋 시각 · 이 체크아웃 통로가 아님 명시",
        n15.contains("`output/single/Dockerfile.source-build` — 실린 바이트") && n15.contains("실린 개정 커밋 2026-10-14T03:07:26Z")
            && n15.contains("이 체크아웃 통로 `output/multi/Dockerfile.source-build` 의 파일이 아니다"));
    // ── F3 · F6 · F5 · F2 · F1(2026-09-29 · plan_26092908 §4.5 FACT 생산자 교정) ──
    let mut xf6 = jc(&xf);
    {
        let d6 = pm(&mut xf6, &["env_shapes", "0", "derived", "0"]);
        pop(d6, "rule_text");
        d6.set("effective_values", j(r#"{"rdma": {"NCCL_IB_DISABLE": "0", "NCCL_NET": "IB"}, "socket": {"NCCL_IB_DISABLE": "1", "NCCL_NET": "Socket"}}"#));
    }
    pm(&mut xf6, &["env_shapes", "0", "derived"]).push(j(r#"{"field": "interconnect.socket_iface", "keys": ["NCCL_SOCKET_IFNAME"],
                                            "placeholders": {}, "value_rule": {"NCCL_SOCKET_IFNAME": "{interconnect.socket_iface}"},
                                            "source": "탐침"}"#));
    let rp6 = rf(_f_reproduce, &repo, &xf6, tpls)?;
    st.ck("F6 rule_text 없는 파생 키 = 렌더러 표(effective_values · value_rule)에서 규칙 · '규칙 미관측' ✗ · 원시 JSON ✗",
        rp6.contains("rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB \\| socket → NCCL_IB_DISABLE=1 · NCCL_NET=Socket")
            && rp6.contains("NCCL_SOCKET_IFNAME={interconnect.socket_iface}") && !rp6.contains("규칙 미관측") && !rp6.contains("{\"NCCL_SOCKET"));
    pop(pm(&mut xf6, &["env_shapes", "0", "derived", "0"]), "effective_values");
    st.ck("★F6 음성대조: 렌더러 표도 없으면 '규칙 미관측'(지어내지 않는다)", rf(_f_reproduce, &repo, &xf6, tpls)?.contains("규칙 미관측"));
    let mut qf3 = jc(&qf);
    *pm(&mut qf3, &["build", "driver_by_node"]) = j(r#"{"main": "580.178.04", "sub": "580.178.04"}"#);
    *pm(&mut qf3, &["build", "driver_conflict"]) = j(r#"{"observed": "580.178.04", "declared": "580.173.02",
                                       "sources": {"observed": "attestation parity.driver(fx)", "declared": "manifest(fx)"}}"#);
    {
        let b = pm(&mut qf3, &["build"]);
        if !b["source"].is_dict() {
            b.set("source", J::dict());
        }
        pm(b, &["source"]).set("driver_by_node", "attestation parity.driver(fx)");
    }
    let rs3 = rf(_f_resolved, &repo, &qf3, tpls)?;
    st.ck("F3 드라이버 두 행: 값 = 읽는 모양 · 출처 = 생산자 출처(원시 JSON ✗ · 출처 미기재 ✗)",
        rs3.contains("| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(fx) |")
            && rs3.contains("| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: attestation parity.driver(fx) · 선언: manifest(fx) |")
            && !rs3.split("⚠ 드라이버").nth(1).unwrap_or("").split('\n').next().unwrap_or("").contains("{\""));
    let mut qf4 = jc(&qf3);
    pop(pm(&mut qf4, &["build"]), "driver_conflict");
    *pm(&mut qf4, &["build", "driver_check"]) = j(r#"{"verdict": "agree", "observed": "580.178.04", "declared": "580.178.04",
                                    "sources": {"observed": "attestation parity.driver(fx)", "declared": "manifest(fx)"}}"#);
    let qf4 = qf4;
    let rs4 = rf(_f_resolved, &repo, &qf4, tpls)?;
    let mut qf9 = jc(&qf4);
    *pm(&mut qf9, &["naming", "vllm_build_input"]) = j(r#"{"kind": "release", "ref": "v0.31.0", "sha": null, "prev_release": null}"#);
    *pm(&mut qf9, &["build", "vllm_describe"]) = J::from("v0.31.0");
    *pm(&mut qf9, &["build", "vllm_prev_pin"]) = J::from("v0.30.0");
    {
        let s = pm(&mut qf9, &["build", "source"]);
        s.set("vllm_describe", "빌드 원장(docker-image) vllm.describe");
        s.set("vllm_prev_pin", "output/single/resolved.json upstream_delta.from_ref");
    }
    let qf9 = qf9;
    let rs9 = rf(_f_resolved, &repo, &qf9, tpls)?;
    let mut qfr = jc(&qf9);
    *pm(&mut qfr, &["build", "vllm_sha"]) = J::Null;
    *pm(&mut qfr, &["naming", "vllm_observed"]) = j(r#"{
        "runtime_source_checkout": "@e40@",
        "runtime_source_checkout_source": "runtime_identity.json source_head · 컴파일 바이트 미검증",
        "runtime_source_describe": "v0.31.0",
        "runtime_source_describe_source": "runtime_identity.json source_describe · 관측 기록",
        "runtime_version": "0.31.1.dev0+gfixture.cu133",
        "runtime_version_source": "runtime_identity.json vllm_version · 수집 명령 기록: runtime_identity.json#/collection_argv",
        "wheel_meta": null, "wheel_meta_source": "배포 메타 미관측"}"#);
    let rsr = rf(_f_resolved, &repo, &qfr, tpls)?;
    st.ck("F3 runtime source/version observations render separately from absent build/distribution provenance",
        rsr.contains("vLLM 빌드 계보 SHA(40자) | 미관측") && rsr.contains("컴파일 바이트 증명은 별개")
            && rsr.contains(&format!("vLLM runtime source checkout SHA(컴파일 바이트 증명 아님) | {}", "e".repeat(40)))
            && rsr.contains("vLLM runtime source describe(관측 기록) | v0.31.0")
            && rsr.contains("vLLM runtime 버전(관측 기록 · 배포 메타 여부 미판정) | 0.31.1.dev0+gfixture.cu133")
            && rsr.contains("wheel 메타(`vllm_build`) | 미관측 | 배포 메타 미관측") && rsr.contains("runtime_identity.json#/collection_argv"));
    {
        let o = pm(&mut qfr, &["naming", "vllm_observed"]);
        o.set("wheel_meta", "0.31.0+distribution");
        o.set("wheel_meta_source", "빌드 원장 vllm.dist_version");
    }
    let rsrd = rf(_f_resolved, &repo, &qfr, tpls)?;
    st.ck("F3 genuine distribution and generic runtime versions render in distinct rows with distinct sources",
        rsrd.contains("wheel 메타(`vllm_build`) | 0.31.0+distribution | 빌드 원장 vllm.dist_version")
            && rsrd.contains("vLLM runtime 버전(관측 기록 · 배포 메타 여부 미판정) | 0.31.1.dev0+gfixture.cu133"));
    let different_dist = _vllm_observed_rows(&j(r#"{"wheel_meta": null, "wheel_meta_basis": "observed-node-difference",
        "wheel_meta_source": "attestation main=1/sub=2 · 값을 고르지 않는다"}"#))?;
    st.ck("노드별 설치 배포메타 차이는 미관측 대신 공통값 미선택으로 표시하고 실제값은 고르지 않는다",
        different_dist.iter().any(|(label, value, source)| label.contains("wheel") && value.contains("공통값 미선택") && source.contains("main=1/sub=2")));
    let mut qf9c = jc(&qf9);
    {
        let b = pm(&mut qf9c, &["naming", "vllm_build_input"]);
        b.set("kind", "commit");
        b.set("prev_release", J::Null);
    }
    let rs9c = rf(_f_resolved, &repo, &qf9c, tpls)?;
    st.ck("★F9 릴리스 태그 빌드의 직전 릴리스 = '해당 없음'(미관측 ✗) + 관측 둘(원장 describe · from_ref)을 뜻과 함께 · 별도 행 누출 ✗ · \
커밋 핀인데 이름 입력 없음 = 미관측(관측 병기)",
        rs9.contains("| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드") && rs9.contains("빌드 원장 describe `v0.31.0`")
            && rs9.contains("업스트림 델타 from_ref `v0.30.0`") && !rs9.contains("`vllm_describe`") && !rs9.contains("`vllm_prev_pin`")
            && rs9c.contains(&format!("| 직전 릴리스(prev_release) | {_MISSING_TEXT}(이름 입력) · 관측:")));
    let mut scoped = qf4.clone();
    let sb = merge(&qf4["build"], &j(r#"{"source_coordinate_scope": "baseline only", "driver_by_node": null,
        "driver_check": {"verdict": "unpaired", "declared": "580.178.04", "observed": null,
                         "observation_scope": "bound structured input", "sources": {}}}"#));
    let sb = with(&sb, &[("source", with(&qf4["build"]["source"], &[("driver_by_node", J::from("bound input absent; other observations not excluded"))]))]);
    scoped.set("build", sb);
    scoped.set("naming", with(&qf4["naming"], &[("vllm_observed", j(r#"{"wheel_meta": "1.2.dev0", "wheel_meta_label": "설치 distribution 메타(다운로드 wheel 증명 아님)"}"#))]));
    let scoped_text = rf(_f_resolved, &repo, &scoped, tpls)?;
    st.ck("scoped source baseline, driver absence and editable distribution metadata remain distinct",
        scoped_text.contains("변경된 작업트리와 컴파일 바이트의 동일성은 별도 증거") && scoped_text.contains("이 측정에 결속된 노드별 입력 미관측")
            && scoped_text.contains("설치 distribution 메타(다운로드 wheel 증명 아님)"));
    st.ck("★F12 관측 = 선언 = '충돌 없음'(두 출처) · 미관측 ✗ · driver_check 는 별도 행으로 새지 않는다",
        rs4.contains("| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(fx) · 선언: manifest(fx) |")
            && !rs4.contains("`driver_check`"));
    let mut qf2 = jc(&qf);
    *pm(&mut qf2, &["build", "image_digest"]) = J::Null;
    {
        let b = pm(&mut qf2, &["build"]);
        if !b["source"].is_dict() {
            b.set("source", J::dict());
        }
        pm(b, &["source"]).set("image_digest_local_note", "docker image inspect fx .Id · 측정 뒤 빌드 — 측정 이미지 아님");
    }
    st.ck("F2 측정 digest 없음 + 로컬 이미지 측정 뒤 빌드 → 값 미관측 · 출처 칸이 관측을 말한다",
        rf(_f_resolved, &repo, &qf2, tpls)?.contains("| 이미지 digest | 미관측 | docker image inspect fx .Id · 측정 뒤 빌드 — 측정 이미지 아님 |"));
    let mut sf5 = jc(&base_facts);
    let sl5 = sf5["slots"].as_dict().and_then(|m| m.keys().next().map(str::to_string)).unwrap_or_default();
    {
        let s = pm(&mut sf5, &["slots", &sl5]);
        pop(s, "rationale");
        s.set("machine_reason", "셀 env 에 VARIANT 줄 없음 = stock");
    }
    st.ck("F5 slots 사유 칸: rationale 없으면 기계 파생 사유 + 표지", rf(_f_slots, &repo, &sf5, tpls)?.contains("셀 env 에 VARIANT 줄 없음 = stock(기계 파생)"));
    pm(&mut sf5, &["slots", &sl5]).set("rationale", "저작자 선언");
    let s5 = rf(_f_slots, &repo, &sf5, tpls)?;
    st.ck("★F5 음성대조: 저작자 rationale 이 있으면 그것이 먼저(기계 사유가 덮지 않는다)",
        s5.contains("저작자 선언") && !s5.split(sl5.as_str()).nth(1).unwrap_or("").split('\n').next().unwrap_or("").contains("(기계 파생)"));
    let mut mf4 = jc(&base_facts);
    mf4.set("campaign", j(r#"{"id": "camp-fx", "cell": "c", "node": "cluster", "mode": "publication-replay",
                       "id_source": "봉인 페이로드 abcdef012345:PAYLOAD.json campaign.id(재생 전용 대체)"}"#));
    mf4.set("approval", J::Null);
    mf4.set("prior_approval", j(r#"{"approved_by": "사용자(fx)", "approved_utc": "2026-01-01T00:00:00Z", "source": "campaign:hint_targets",
                             "replay_source": "봉인된 발행 페이로드 abcdef012345"}"#));
    let m4 = rf(_f_meta, &repo, &mf4, tpls)?;
    st.ck("F4 재생: 캠페인 id = 봉인 기록(출처 괄호) · 승인 = 원 발행 승인(재생 전용 대체 표지)",
        rf(_f_header, &repo, &mf4, tpls)?.contains("camp-fx(봉인 페이로드 abcdef012345:PAYLOAD.json campaign.id(재생 전용 대체))")
            && m4.contains("| 승인 출처 | campaign:hint_targets(재생 전용 대체 — 원 발행의 승인") && m4.contains("2026-01-01T00:00:00Z(원 발행)"));
    mf4.set("approval", j(r#"{"approved_by": "사람(이 판)", "approved_utc": "2026-02-02T00:00:00Z", "source": "cli:--approved-by"}"#));
    let m4b = rf(_f_meta, &repo, &mf4, tpls)?;
    st.ck("★F4 음성대조: 이 판의 승인이 있으면 그것만(원 발행 승인과 섞지 않는다)", m4b.contains("| 승인 출처 | cli:--approved-by |") && !m4b.contains("원 발행"));
    let mut mf5 = jc(&mf4);
    mf5.set("campaign", j(r#"{"id": null, "cell": "c", "node": "cluster", "mode": "publication-replay",
                       "membership": {"status": "outside", "id": "_bootstrap",
                                      "source": "sweep/sweep_index.json bench_knobs.campaign = null · campaign_pins_note"}}"#));
    let h5 = rf(_f_header, &repo, &mf5, tpls)?;
    let m5 = rf(_f_meta, &repo, &mf5, tpls)?;
    pop(pm(&mut mf5, &["campaign"]), "membership");
    let h6 = rf(_f_header, &repo, &mf5, tpls)?;
    st.ck("★F13 재생 · 측정 시점 캠페인 밖(관측) = '캠페인 밖(`_bootstrap` · 출처)' — 헤더·메타 둘 다 · 음성대조: 기록 없으면 미관측",
        h5.contains("캠페인 밖(`_bootstrap` · sweep/sweep_index.json bench_knobs.campaign = null") && m5.contains("캠페인 밖(`_bootstrap`")
            && !h6.contains("캠페인 밖") && h6.contains(_MISSING_TEXT));
    let mut sr12 = jc(&base_facts);
    if sr12["sub_recipe"].is_dict() && sr12["sub_recipe"].truthy() {
        sr12.set("plane", "native");
        sr12.set("native_source", j(r#"{"cell": "src-cell"}"#));
        st.ck("F12 native sub_recipe = 'Docker 형 참고(원천 셀 · 이 셀 run 아님 · generated-unverified)' 머리",
            rf(_f_sub_recipe, &repo, &sr12, tpls)?.contains("**Docker 형 참고**(원천 셀 `src-cell` · **이 셀 run 아님** · `generated-unverified`)"));
        sr12.set("plane", "docker");
        st.ck("★F12 음성대조: Docker 셀 sub_recipe 에는 참고 표지 없음", !rf(_f_sub_recipe, &repo, &sr12, tpls)?.contains("Docker 형 참고"));
    } else {
        st.ck("F12 픽스처: base_facts 에 sub_recipe 가 있어야 한다(검사가 조용히 빠지지 않게)", false);
    }
    let pe1 = _patch_evidence_table(&j(r#"[{"file": "60-a.sh", "script_identity": "build-ledger(원장 script_sha256 = 작업트리 바이트)",
                                  "ledger_script_sha256": "@a64@"}]"#))?.join("\n");
    st.ck("F1 판정 근거 표: 원장 sha 를 출처 표지와 함께 옮긴다(이미지 사본과 구분)",
        pe1.contains(&format!("`{}`(빌드 원장 script_sha256 — 이미지 사본 아님)", "a".repeat(64))) && pe1.contains("build-ledger(원장"));
    let cm = _context_map_summary(&xf)?;
    st.ck("README 매핑 요약: zip 폴더 → 컨텍스트 폴더 · 입력 아닌 파일 수", cm.contains("| `artifacts/build_patch_pre/` | `build_patches_src/` | 1 |")
        && cm.contains("입력이 아닌 파일 1개"));
    // 검증 표시 3자리(PAYLOAD 파일 기록 · 파일 첫 줄 · 01 표) · Agent 저작 요청 · 태그 이름 리터럴
    let mut vf = jc(&base_facts);
    vf.set("base_tag", vf["tag"].clone());
    let hdr = format!("# {UNVERIFIED_MARK} — native 셀(Docker 미사용)이라 이 파일은 실행 검증되지 않았다 · 생성: renderer");
    *pm(&mut vf, &["slots", "build_patch_pre", "file_records"]) = qf["slots"]["build_patch_pre"]["file_records"].clone();
    let mut compose = j(r#"{"files": ["artifacts/compose/serve_runner.sh"], "applicable": true, "confidence": "1-signal",
                              "evidence": {"kind": "file", "ref": "renderer"},
                              "file_records": [{"path": "artifacts/compose/serve_runner.sh", "sha256": "@164@",
                                                "verification": "generated-unverified", "generated_by": "renderer",
                                                "verification_basis": "렌더러 산출물", "relevance": "required",
                                                "relevance_basis": "재현 경로 입력",
                                                "header": {"style": "comment", "line": 2, "text": null}}]}"#);
    pm(&mut compose, &["file_records", "0", "header"]).set("text", hdr.as_str());
    *pm(&mut vf, &["slots", "compose"]) = compose;
    let areq = crate::jd! {"slot" => "triplet", "name" => "native-launch.md", "path" => "artifacts/triplet/native-launch.md",
        "why" => "native 기동 기록 부재", "inputs" => crate::jl!["artifacts/triplet/"], "verification" => "generated-unverified",
        "generated_by" => "agent", "header_required" => true,
        "header" => format!("{UNVERIFIED_MARK} — Agent 가 증거에서 저작했다(실행 검증 ✗) · 생성: agent")};
    vf.set("agent_requests", J::List(vec![areq.clone()]));
    let vf = vf;
    let pv = root.join("v7marks").join("payload");
    mkdirs(&pv.join("artifacts/build_patch_pre"))?;
    wt(&pv.join("artifacts/build_patch_pre/60-fixture-patch.sh"), "#!/bin/sh\n# fixture patch: PLE 게이트 완화\nexit 0\n")?;
    mkdirs(&pv.join("artifacts/compose"))?;
    wt(&pv.join("artifacts/compose/serve_runner.sh"), &format!("#!/bin/bash\n{hdr}\necho serve\n"))?;
    render_scaffold(&repo, &vf, &pv, None)?;
    let apath = areq["path"].display();
    let aheader = areq["header"].display();
    let ag = pv.join(&apath);
    let agt = rt(&ag)?;
    let pv01 = rt(&pv.join("01-artifacts.md"))?;
    st.ck("Agent 저작 요청: 스캐폴드 = 첫 줄 경고(md = HTML 주석) + 자리표시 · 01 요청 표 · 파일별 검증 · 관련성 표",
        agt.split('\n').next() == Some(format!("<!-- {aheader} -->").as_str()) && agt.contains(AGENT_MARK)
            && pv01.contains("**Agent 저작 요청 1건**") && pv01.contains("`generated-unverified` · renderer"));
    fill(&pv, &vf)?;
    refresh(&pv)?;
    let vkw = kw.clone().f(&jc(&vf));
    let vc = vkw.codes(&repo, &pv)?;
    st.ck("★Agent 요청 파일 미저작 = HINT_AGENT_PLACEHOLDER_RESIDUE · HINT_AGENT_ARTIFACT_UNAUTHORED",
        vc.contains("HINT_AGENT_PLACEHOLDER_RESIDUE") && vc.contains("HINT_AGENT_ARTIFACT_UNAUTHORED"));
    wt(&ag, &format!("<!-- {aheader} -->\n# native 기동 절차\n1. ray head 기동\n"))?;
    let vvar = |tag: &str, f: &dyn Fn(&Path) -> HR<()>| -> HR<PathBuf> {
        let dst = root.join("v7v").join(tag).join("payload");
        copytree(&pv, &dst)?;
        f(&dst)?;
        Ok(dst)
    };
    let vl = vkw.run(&repo, &pv)?;
    st.ck(&format!("검증 표시 3자리 일치 · Agent 저작 완료 = lint 0: {}", hd(&vl, 3)), vl.is_empty());
    let ap = apath.clone();
    st.ck("★Agent 저작 파일 첫 줄 경고를 지웠다 = HINT_VERIFICATION_MARK_MISMATCH", vkw.codes(&repo, &vvar("agh",
        &move |q: &Path| wt(&q.join(&ap), "# native 기동 절차\n1. ray head 기동\n"))?)?.contains("HINT_VERIFICATION_MARK_MISMATCH"));
    st.ck("★generated-unverified 파일의 헤더를 지웠다 = HINT_VERIFICATION_MARK_MISMATCH", vkw.codes(&repo, &vvar("vmh",
        &|q: &Path| wt(&q.join("artifacts/compose/serve_runner.sh"), "#!/bin/bash\necho serve\n"))?)?.contains("HINT_VERIFICATION_MARK_MISMATCH"));
    let hdr2 = hdr.clone();
    st.ck("★verified 파일에 미검증 경고가 붙었다 = HINT_VERIFICATION_MARK_MISMATCH", vkw.codes(&repo, &vvar("vmv",
        &move |q: &Path| wt(&q.join("artifacts/build_patch_pre/60-fixture-patch.sh"), &format!("#!/bin/sh\n{hdr2}\nexit 0\n")))?)?
        .contains("HINT_VERIFICATION_MARK_MISMATCH"));
    st.ck("★01 `검증` 열을 손으로 verified 로 바꿨다 = HINT_FACT_DRIFT(셋째 자리)", vkw.codes(&repo, &vvar("vmt",
        &ed("01-artifacts.md", "`generated-unverified` · renderer", "`verified` · renderer"))?)?.contains("HINT_FACT_DRIFT"));
    st.ck("★저작 산문에 태그 이름 리터럴 = HINT_TAG_LITERAL_IN_PROSE(이름은 사실 블록 소유)", vkw.codes(&repo, &vvar("tl",
        &ed("02-narrative.md", "- moe-backend 를 명시하지 마라", &format!("- {} 의 moe-backend 를 명시하지 마라", vf["tag"].display())))?)?
        .contains("HINT_TAG_LITERAL_IN_PROSE"));
    let cs = cited_sources(&repo, &payload, &lineage, None)?;
    let csp: Vec<String> = cs["paths"].items().iter().map(J::display).collect();
    st.ck("cited_sources: 발췌 수(계보 문서) · 출처 경로(서명 · hint-event) — 봉인 스냅샷 · LINEAGE 요약 입력",
        cs["excerpt_counts"].get(plan_p).and_then(J::as_i64).unwrap_or(0) >= 1 && csp.iter().any(|p| p == tl_p) && csp.iter().any(|p| p == log_p)
            && csp.iter().all(|p| !p.starts_with("artifacts/")));
    let _ = (&t01, &t03, &l00);
    Ok(())
}
