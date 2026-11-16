//! `scan_policy_citations` / `citation_violations` against the original's answers (checks/data/citations.json, captured by
//! checks/capture_citations.sh). No Python runs here.

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use policy_audit::value::{as_str, get, list};
use std::path::Path;

#[test]
fn citations_equal_the_original() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/citations.json")).unwrap();
    let cases = pyjson::loads(&text).unwrap();
    let registry = pyjson::loads(r#"{"policies": [{"policy_id": "KNOWN", "status": "active"}, {"policy_id": "OLD", "status": "retired"}, "junk"]}"#).unwrap();
    let mut n = 0;
    for case in list(&cases).unwrap() {
        let t = as_str(get(case, "text")).unwrap();
        let scan: Vec<(usize, String)> = list(get(case, "scan"))
            .unwrap()
            .iter()
            .map(|pair| {
                let l = list(pair).unwrap();
                let line = match &l[0] {
                    PyValue::Int(i) => i.to_string().parse().unwrap(),
                    _ => panic!(),
                };
                (line, as_str(&l[1]).unwrap())
            })
            .collect();
        assert_eq!(policy_audit::scan_policy_citations(&t), scan, "scan of {t:?}");
        let got = policy_audit::citation_violations(&t, &registry, Some("src.md"));
        let want = list(get(case, "violations")).unwrap();
        assert_eq!(got.len(), want.len(), "violations of {t:?}: {got:?}");
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.reason_code, as_str(get(w, "reason_code")).unwrap(), "{t:?}");
            assert_eq!(g.message, as_str(get(w, "message")).unwrap(), "{t:?}");
            assert_eq!(&g.policy_id, get(w, "policy_id"), "{t:?}");
            assert_eq!(&g.clause_id, get(w, "clause_id"), "{t:?}");
            assert_eq!(&g.path, get(w, "path"), "{t:?}");
        }
        n += 1;
    }
    assert!(n >= 18);
}
