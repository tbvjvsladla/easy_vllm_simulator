//! Parser fidelity: every case in `checks/data/pyast/cases.json` (source text + the CPython 3.12 `ast.walk` digest of
//! the node kinds the originals inspect, captured by `checks/capture_goldens.sh` [retired with the originals · frozen · plan_26111717]) must parse to the same digest.
//! `PYAST_EXTRA_FILES` (newline-separated paths) additionally reports parse errors for ad hoc corpora.
use policy_selftest::pyast::{self, Kind};

fn digest(src: &str) -> Result<Vec<String>, String> {
    let tree = pyast::parse(src)?;
    Ok(pyast::walk(&tree)
        .into_iter()
        .filter_map(|n| {
            let k = match n.kind {
                Kind::Assign => "Assign",
                Kind::Call => "Call",
                Kind::Compare => "Compare",
                Kind::FunctionDef => "FunctionDef",
                Kind::AsyncFunctionDef => "AsyncFunctionDef",
                Kind::Assert => "Assert",
                _ => return None,
            };
            Some(format!("{k}:{}:{}", n.line, n.end_line))
        })
        .collect())
}

fn b64(s: &str) -> Vec<u8> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("base64"),
        }
    };
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    out
}

#[test]
fn golden_digests() {
    let data = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/checks/data/pyast/cases.json")).unwrap();
    let v = agent_runtime_support::pyjson::loads(&data).unwrap();
    let agent_runtime_support::pyvalue::PyValue::List(cases) = v else { panic!("cases") };
    let mut bad = Vec::new();
    for c in &cases {
        let agent_runtime_support::pyvalue::PyValue::Dict(d) = c else { panic!() };
        let get = |k: &str| match d.get(k) {
            Some(agent_runtime_support::pyvalue::PyValue::Str(s)) => s.to_utf8().unwrap(),
            _ => String::new(),
        };
        let name = get("name");
        let src = String::from_utf8(b64(&get("source_b64"))).expect("utf-8 source");
        let want: Vec<String> = match d.get("walk") {
            Some(agent_runtime_support::pyvalue::PyValue::List(l)) => l
                .iter()
                .map(|x| match x {
                    agent_runtime_support::pyvalue::PyValue::Str(s) => s.to_utf8().unwrap(),
                    _ => String::new(),
                })
                .collect(),
            _ => vec!["SyntaxError".to_string()],
        };
        let got = digest(&src).unwrap_or_else(|_| vec!["SyntaxError".to_string()]);
        if got != want {
            let first = got.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len()));
            bad.push(format!("{name}: first difference at {first}: got {:?} want {:?}", got.get(first), want.get(first)));
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
    assert!(cases.len() >= 10);
}

#[test]
fn extra_files_parse() {
    let Ok(list) = std::env::var("PYAST_EXTRA_FILES") else { return };
    let mut bad = Vec::new();
    for f in list.lines().filter(|l| !l.is_empty()) {
        let src = String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned();
        if let Err(e) = pyast::parse(&src) {
            bad.push(format!("{f}: {e}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
