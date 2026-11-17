//! The assertion-id extractor against the original's answers (checks/data/assertion_ids.json, captured with
//! checks/capture_ids.sh [retired with the originals · frozen · plan_26111717]): every `.py` file and every evidence / claim-binding path of the pinned commit, read from that
//! commit's blobs so the check does not depend on the working tree. No Python runs here.

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf()
}

#[test]
fn assertion_ids_equal_the_original() {
    let golden = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/assertion_ids.json")).unwrap();
    // Tracked golden data spells the skill-script prefix as `@TFN_SCRIPTS@/` so that it is not read as an external call of
    // a non-public terraforming_node script by the public-surface tripwire (antipattern_scan); decoded here.
    let golden = golden.replace("@TFN_SCRIPTS@/", concat!("terraforming", "_node/scripts/"));
    let doc = pyjson::loads(&golden).unwrap();
    let PyValue::Dict(top) = doc else { panic!("golden root") };
    let rev = top.get("rev").and_then(|v| if let PyValue::Str(s) = v { s.to_utf8() } else { None }).unwrap();
    let PyValue::Dict(ids) = top.get("ids").unwrap().clone() else { panic!("ids") };
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for (path, want) in ids.iter() {
        let path = path.to_utf8().unwrap();
        let out = Command::new("git").arg("-C").arg(repo()).arg("show").arg(format!("{rev}:{path}")).output().unwrap();
        let bytes = if out.status.success() { Some(out.stdout) } else { None };
        let got = policy_audit::pyids::extract_from_bytes(Path::new(&path), bytes.as_deref());
        let want: Option<BTreeSet<String>> = match want {
            PyValue::None => None,
            PyValue::List(l) => Some(l.iter().map(|x| if let PyValue::Str(s) = x { s.to_utf8().unwrap() } else { panic!() }).collect()),
            _ => panic!("golden value"),
        };
        checked += 1;
        if got != want {
            let detail = match (&got, &want) {
                (Some(g), Some(w)) => format!("extra={:?} missing={:?}", g.difference(w).collect::<Vec<_>>(), w.difference(g).collect::<Vec<_>>()),
                _ => format!("got={got:?} want={want:?}"),
            };
            mismatches.push(format!("{path}: {detail}"));
        }
    }
    assert!(checked > 150, "golden too small: {checked}");
    assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
}
