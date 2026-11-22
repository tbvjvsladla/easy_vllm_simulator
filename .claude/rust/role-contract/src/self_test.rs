//! The source's own `--self-test`, translated: 33 checks over the native functions of this library (no input/output, no manifest file), each with the
//! source's label, a `PASS`/`FAIL` line like the original's and the count line, then 6 checks of the manifest-pair field `sub_workspace_shared`
//! (plan_26112012 #2b, not in the source). The conditions are computed from the functions' results; nothing here is a recorded answer.

use crate::*;

/// The outcome of one run of the self-test.
pub struct SelfTest {
    /// The printed lines in order (`PASS name`, `FAIL name — detail`, then `--- P/T PASS`).
    pub lines: Vec<String>,
    pub total: usize,
    pub failures: usize,
}

struct Runner {
    lines: Vec<String>,
    ran: usize,
    failures: usize,
}

impl Runner {
    fn chk(&mut self, name: &str, condition: bool, detail: impl FnOnce() -> String) {
        self.ran += 1;
        if condition {
            self.lines.push(format!("PASS {name}"));
        } else {
            self.lines.push(format!("FAIL {name} — {}", detail()));
            self.failures += 1;
        }
    }
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn dict(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in pairs {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn node(role: &str) -> PyValue {
    dict(vec![("role", s(role))])
}

fn node_with(role: &str, key: &str, value: &str) -> PyValue {
    dict(vec![("role", s(role)), (key, s(value))])
}

fn list(items: Vec<PyValue>) -> PyValue {
    PyValue::List(items)
}

fn code<T>(result: Result<T, ContractViolation>) -> Option<&'static str> {
    result.err().map(|violation| violation.code)
}

fn show(result: &Result<Verdict, ContractViolation>) -> String {
    match result {
        Ok(verdict) => verdict.to_value().repr(),
        Err(violation) => violation.as_dict().repr(),
    }
}

fn is(verdict: &Result<Verdict, ContractViolation>, value: PyValue, source: &str) -> bool {
    matches!(verdict, Ok(v) if v.value == value && v.source.equals(source))
}

fn texts(items: &[&str]) -> PyValue {
    list(items.iter().map(|i| s(i)).collect())
}

fn violation_codes(evaluation: &Evaluation) -> Vec<&'static str> {
    evaluation.violations().iter().map(|v| v.code).collect()
}

/// Runs the checks.
pub fn run() -> SelfTest {
    let mut r = Runner {
        lines: Vec::new(),
        ran: 0,
        failures: 0,
    };
    let none = PyValue::None;
    let role_sub = s("sub");
    let rsm = resolve_sub_mode;

    // sub_mode derivation and declaration
    let v = rsm(&s("single"), &none);
    r.chk(
        "single 미선언 → a2a-agent 파생",
        is(&v, s("a2a-agent"), "derived-from-topology"),
        || show(&v),
    );
    let v = rsm(&s("multi"), &none);
    r.chk(
        "multi 미선언 → ray-worker 파생",
        is(&v, s("ray-worker"), "derived-from-topology"),
        || show(&v),
    );
    let v = rsm(&s("single"), &s("a2a-agent"));
    r.chk(
        "일치 선언 → declared-and-agrees(출처 구분)",
        is(&v, s("a2a-agent"), "declared-and-agrees"),
        || show(&v),
    );
    r.chk(
        "single + ray-worker 선언 → 불변식 A 충돌 fail-closed",
        code(rsm(&s("single"), &s("ray-worker"))) == Some("SUB_MODE_TOPOLOGY_CONFLICT"),
        String::new,
    );
    r.chk(
        "multi + a2a-agent 선언 → 충돌 fail-closed",
        code(rsm(&s("multi"), &s("a2a-agent"))) == Some("SUB_MODE_TOPOLOGY_CONFLICT"),
        String::new,
    );
    r.chk(
        "어휘 밖 선언 → SUB_MODE_UNKNOWN",
        code(rsm(&s("multi"), &s("ray_worker"))) == Some("SUB_MODE_UNKNOWN"),
        String::new,
    );
    r.chk(
        "빈 선언은 미선언과 같다(공백 허용)",
        matches!(rsm(&s("multi"), &s("   ")), Ok(v) if v.source.equals("derived-from-topology")),
        String::new,
    );
    r.chk(
        "토폴로지 미선언/추정 → TOPOLOGY_UNKNOWN(브랜치 추론 ✗)",
        code(rsm(&s("auto"), &none)) == Some("TOPOLOGY_UNKNOWN"),
        String::new,
    );

    // rank
    let nodes = list(vec![node("main"), node("sub")]);
    let v = resolve_rank(&s("multi"), &nodes, &s("main"));
    r.chk(
        "multi rank(main)=0",
        is(&v, PyValue::int(0), "manifest-nodes-index"),
        String::new,
    );
    let v = resolve_rank(&s("multi"), &nodes, &role_sub);
    r.chk(
        "multi rank(sub)=1",
        is(&v, PyValue::int(1), "manifest-nodes-index"),
        String::new,
    );
    let v = resolve_rank(&s("single"), &nodes, &role_sub);
    r.chk(
        "single rank 은 None + 음성정직 출처",
        is(&v, PyValue::None, "not-applicable:single-a2a-agent"),
        || show(&v),
    );
    r.chk(
        "multi 에 해당 role 부재 → RANK_UNRESOLVED",
        code(resolve_rank(
            &s("multi"),
            &list(vec![node("main")]),
            &role_sub,
        )) == Some("RANK_UNRESOLVED"),
        String::new,
    );
    r.chk(
        "role 슬러그 중복 → RANK_AMBIGUOUS_ROLE(조용한 첫 항목 선택 ✗)",
        code(resolve_rank(
            &s("multi"),
            &list(vec![node("main"), node("sub"), node("sub")]),
            &role_sub,
        )) == Some("RANK_AMBIGUOUS_ROLE"),
        String::new,
    );

    // identity authority and delivery plane
    r.chk("single 정체성 권위 = SSH endpoint auth + manifest role", matches!(identity_authority(&s("single")), Ok(v) if v.value == s("ssh-endpoint-auth-and-manifest-role")), String::new);
    r.chk(
        "multi 정체성 권위 = manifest-rank-and-role",
        matches!(identity_authority(&s("multi")), Ok(v) if v.value == s("manifest-rank-and-role")),
        String::new,
    );
    r.chk(
        "★ single(a2a-agent) 배달 평면 = dormant (plan_26082214 §4.4 회귀핀)",
        matches!(delivery_plane(&s("single"), &s("a2a-agent")), Ok(v) if v.value == s("dormant")),
        String::new,
    );
    r.chk(
        "multi(ray-worker) 배달 평면 = active",
        matches!(delivery_plane(&s("multi"), &s("ray-worker")), Ok(v) if v.value == s("active")),
        String::new,
    );

    // whole-manifest evaluation
    let single_with_sub = dict(vec![("topology", s("single")), ("nodes", nodes.clone())]);
    let e = evaluate_manifest(&single_with_sub, &none, &role_sub);
    r.chk(
        "★ single + nodes[main,sub] 미선언 → 위반 0 · 배달 dormant (증상1·5 회귀핀)",
        e.violations().is_empty()
            && e.delivery_plane().value == s("dormant")
            && e.sub_mode().is_some_and(|v| v.value == s("a2a-agent"))
            && e.rank().is_some_and(|v| v.value == PyValue::None),
        || e.to_value().repr(),
    );
    let multi_with_sub = dict(vec![("topology", s("multi")), ("nodes", nodes.clone())]);
    let e = evaluate_manifest(&multi_with_sub, &none, &role_sub);
    r.chk(
        "multi + nodes[main,sub] → 배달 active · rank(sub)=1",
        e.violations().is_empty()
            && e.delivery_plane().value == s("active")
            && e.rank().is_some_and(|v| v.value == PyValue::int(1)),
        || e.to_value().repr(),
    );
    let bad = dict(vec![
        ("topology", s("single")),
        (
            "nodes",
            list(vec![
                node("main"),
                node_with("sub", "sub_mode", "ray-worker"),
            ]),
        ),
    ]);
    let e = evaluate_manifest(&bad, &none, &role_sub);
    r.chk(
        "single 에 ray-worker 선언 → 위반 보고 + 배달 fail-closed dormant",
        violation_codes(&e) == ["SUB_MODE_TOPOLOGY_CONFLICT"]
            && e.delivery_plane().value == s("dormant"),
        || e.to_value().repr(),
    );
    let e = evaluate_manifest(
        &dict(vec![("topology", s("single")), ("nodes", list(vec![]))]),
        &none,
        &role_sub,
    );
    r.chk(
        "서브 미등록 → dormant(no-sub-registered) · 위반 0",
        e.violations().is_empty() && e.delivery_plane().source.equals("no-sub-registered"),
        || e.to_value().repr(),
    );
    let e = evaluate_manifest(&dict(vec![("nodes", nodes.clone())]), &none, &role_sub);
    r.chk(
        "topology 부재 → TOPOLOGY_UNKNOWN + 배달 fail-closed",
        violation_codes(&e) == ["TOPOLOGY_UNKNOWN"] && e.delivery_plane().value == s("dormant"),
        || e.to_value().repr(),
    );
    let e = evaluate_manifest(
        &dict(vec![
            ("topology", s("multi")),
            ("nodes", list(vec![node("sub"), node("main")])),
        ]),
        &none,
        &role_sub,
    );
    r.chk(
        "multi 에서 main 이 인덱스 0 이 아니면 note(위반 ✗)",
        e.violations().is_empty() && e.notes().len() == 1,
        || e.to_value().repr(),
    );

    // tool plane and self-publication, both directions
    let v = tool_plane(&s("single"), &s("a2a-agent"));
    r.chk(
        "single(a2a-agent) → 런타임 스킬 3종 + hint-publisher(2026-10-11 · plan_26101104) + wiki-desk(2026-10-19)",
        matches!(&v, Ok(v) if v.source.equals("sub-mode:a2a-agent") && v.value == texts(&["vllm-recipe-explorer", "adversarial-benchmark", "upstream-version-watch", "hint-publisher", "wiki-desk"])),
        || show(&v),
    );
    let v = tool_plane(&s("multi"), &s("ray-worker"));
    r.chk(
        "multi(ray-worker) → 사서 1종(정본 재현 + 기록 입고 · hint-publisher 는 없다)",
        matches!(&v, Ok(v) if v.value == texts(&["wiki-desk"]) && v.source.equals("sub-mode:ray-worker")),
        || show(&v),
    );
    let v = sub_self_publishes(&s("single"), &none);
    r.chk(
        "single 서브 → 자체 발행(파생 출처 tool_plane)",
        matches!(&v, Ok(v) if v.value == PyValue::Bool(true) && v.source.equals("derived:tool_plane(single,a2a-agent)")),
        || show(&v),
    );
    let v = sub_self_publishes(&s("multi"), &none);
    r.chk(
        "multi 서브 → 자체 발행 ✗(Ray 워커는 셀을 소유하지 않는다)",
        matches!(&v, Ok(v) if v.value == PyValue::Bool(false) && v.source.equals("derived:tool_plane(multi,ray-worker)")),
        || show(&v),
    );
    r.chk(
        "자체 발행 판정도 어휘 밖 토폴로지 → fail-closed",
        code(sub_self_publishes(&s("dual"), &none)) == Some("TOPOLOGY_UNKNOWN"),
        String::new,
    );
    r.chk(
        "어휘 밖 sub_mode → fail-closed",
        code(tool_plane(&s("single"), &s("worker"))) == Some("SUB_MODE_UNKNOWN"),
        String::new,
    );
    let e = evaluate_manifest(
        &dict(vec![
            ("topology", s("multi")),
            ("nodes", list(vec![node("main"), node("sub")])),
        ]),
        &none,
        &role_sub,
    );
    r.chk(
        "evaluate_manifest 가 tool_plane 을 함께 낸다(멀티=사서 1종)",
        e.tool_plane().value == texts(&["wiki-desk"]) && e.violations().is_empty(),
        || e.tool_plane().to_value().repr(),
    );
    let e = evaluate_manifest(
        &dict(vec![
            ("topology", s("single")),
            ("nodes", list(vec![node("main")])),
        ]),
        &none,
        &role_sub,
    );
    r.chk(
        "서브 미등록 → tool_plane 0종 + 출처 no-sub-registered",
        e.tool_plane().value == texts(&[]) && e.tool_plane().source.equals("no-sub-registered"),
        || e.tool_plane().to_value().repr(),
    );
    let e = evaluate_manifest(
        &dict(vec![
            ("topology", s("single")),
            (
                "nodes",
                list(vec![
                    node("main"),
                    node_with("sub", "sub_mode", "ray-worker"),
                ]),
            ),
        ]),
        &none,
        &role_sub,
    );
    r.chk(
        "정체성 위반이면 tool_plane 은 fail-closed 빈 집합(스킬을 주지 않는다)",
        e.tool_plane().value == texts(&[]) && !e.violations().is_empty(),
        || e.tool_plane().to_value().repr(),
    );
    let again = |doc: &PyValue| evaluate_manifest(doc, &none, &role_sub).to_value().repr();
    r.chk(
        "결정론(동일 입력 → 동일 출력)",
        again(&single_with_sub) == again(&single_with_sub),
        String::new,
    );

    // plan_26112012 #2b: one sub workspace shared by the two topologies (node_id and work_dir of the manifest pair).
    let pair_doc = |node_id: &str, work_dir: &str| {
        dict(vec![(
            "nodes",
            list(vec![
                node("main"),
                dict(vec![("role", s("sub")), ("node_id", s(node_id)), ("work_dir", s(work_dir))]),
            ]),
        )])
    };
    let same = pair_doc("sub-node-a", "/work/sub-a");
    let shared_is = |single: Option<&PyValue>, multi: Option<&PyValue>, want: &str| {
        let verdict = sub_workspace_shared(single, multi);
        verdict.value == s(want) && verdict.source.equals(SUB_WORKSPACE_SHARED_SOURCE)
    };
    let shown = |single: Option<&PyValue>, multi: Option<&PyValue>| sub_workspace_shared(single, multi).to_value().repr();
    r.chk(
        "sub_workspace_shared: node_id·work_dir 같음 → shared(measured:manifest-pair)",
        shared_is(Some(&same), Some(&same), "shared"),
        || shown(Some(&same), Some(&same)),
    );
    let other_id = pair_doc("sub-node-b", "/work/sub-a");
    r.chk(
        "sub_workspace_shared: node_id 다름 → distinct",
        shared_is(Some(&same), Some(&other_id), "distinct"),
        || shown(Some(&same), Some(&other_id)),
    );
    let other_dir = pair_doc("sub-node-a", "/work/sub-other");
    r.chk(
        "sub_workspace_shared: work_dir 다름 → distinct",
        shared_is(Some(&same), Some(&other_dir), "distinct"),
        || shown(Some(&same), Some(&other_dir)),
    );
    r.chk(
        "sub_workspace_shared: multi manifest 없음 → unknown",
        shared_is(Some(&same), None, "unknown"),
        || shown(Some(&same), None),
    );
    let blank = pair_doc("sub-node-a", "");
    r.chk(
        "sub_workspace_shared: 필드가 비면 → unknown",
        shared_is(Some(&same), Some(&blank), "unknown"),
        || shown(Some(&same), Some(&blank)),
    );
    let printed = [shown(Some(&same), Some(&same)), shown(Some(&same), Some(&other_dir)), shown(Some(&same), None)].concat();
    r.chk(
        "sub_workspace_shared: 출력에 경로·이름 0",
        !printed.contains("/work/") && !printed.contains("sub-node-"),
        || printed.clone(),
    );

    let total = r.ran;
    let passed = total - r.failures;
    r.lines.push(format!("--- {passed}/{total} PASS"));
    SelfTest {
        lines: r.lines,
        total,
        failures: r.failures,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_translated_self_test_runs_33_checks_and_the_6_manifest_pair_checks_and_all_pass() {
        let run = super::run();
        assert_eq!((run.total, run.failures), (39, 0), "{:#?}", run.lines);
        assert_eq!(run.lines.last().unwrap(), "--- 39/39 PASS");
    }
}
