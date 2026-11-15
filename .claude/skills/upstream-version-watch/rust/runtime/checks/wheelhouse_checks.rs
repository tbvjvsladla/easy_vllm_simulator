//! Focused checks of the `native_wheelhouse` build port: the JSON layout and digests it shares with the Python original, and one
//! synthetic installed tree built through the real program (`build-dir`). Parity against the Python original is the campaign's own comparison.

use upstream_runtime::wheelhouse_util::*;

#[test]
fn json_layout_matches_python_indent_1() {
    let v = obj(vec![("a", J::Arr(vec![J::Int(1), J::s("x\"y\n")])), ("b", J::Obj(vec![])), ("c", J::Arr(vec![])), ("d", J::Null), ("e", J::Float(1.5))]);
    assert_eq!(dumps(&v), "{\n \"a\": [\n  1,\n  \"x\\\"y\\n\"\n ],\n \"b\": {},\n \"c\": [],\n \"d\": null,\n \"e\": 1.5\n}");
    assert_eq!(loads(dumps(&v).as_bytes()).unwrap(), v);
}

#[test]
fn digests_and_paths() {
    assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(b64url(&[0xfb, 0xff]), "-_8");
    assert_eq!(normpath("/a/./b/../c//d"), "/a/c/d");
    assert_eq!(relpath("/a/b/c", "/a/x"), "../b/c");
    assert_eq!(dirname("/a/b"), "/a");
    assert!(fnmatch("/etc/ld.so.conf.d/x.conf", "/etc/ld.so.conf.d/*.conf"));
    let rows = vec![vec!["a,b".to_string(), "sha256=x".to_string(), "3".to_string()]];
    assert_eq!(csv_rows(&csv_write(&rows)), rows);
}

#[test]
fn build_dir_repacks_a_synthetic_tree() {
    let td = std::env::temp_dir().join(format!("nwh-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&td);
    let root = td.join("root");
    let site = root.join("usr/local/lib/python3.12/dist-packages");
    std::fs::create_dir_all(site.join("alpha")).unwrap();
    std::fs::create_dir_all(site.join("alpha-1.0.dist-info")).unwrap();
    std::fs::write(site.join("alpha/__init__.py"), b"VALUE = 42\n").unwrap();
    std::fs::write(site.join("alpha-1.0.dist-info/METADATA"), b"Metadata-Version: 2.1\nName: alpha\nVersion: 1.0\n").unwrap();
    let h = b64url(ring_sha256(b"VALUE = 42\n").as_slice());
    std::fs::write(site.join("alpha-1.0.dist-info/RECORD"), format!("alpha/__init__.py,sha256={h},11\nalpha-1.0.dist-info/RECORD,,\n")).unwrap();
    std::fs::create_dir_all(root.join("usr/local/cuda/bin")).unwrap();
    let meta = td.join("meta.json");
    std::fs::write(&meta, r#"{"tag":"t","id":"sha256:00","repo_digests":[],"architecture":"arm64","os":"linux","env":[]}"#).unwrap();
    let out = td.join("out");
    let args: Vec<String> = ["build-dir", "--root", root.to_str().unwrap(), "--meta-json", meta.to_str().unwrap(), "--out", out.to_str().unwrap(), "--generated-utc", "2026-01-01T00:00:00Z"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(upstream_runtime::wheelhouse_cli::run(&args), 0);
    let closure = loads(&std::fs::read(out.join("closure.json")).unwrap()).unwrap();
    assert_eq!(closure.get("status").and_then(|s| s.str()), Some("PASS"));
    assert_eq!(std::fs::read_to_string(out.join("requirements-closure.txt")).unwrap().lines().last(), Some("alpha==1.0"));
    assert!(out.join("wheelhouse/alpha-1.0-py3-none-any.whl").is_file());
    // a non-empty --out is refused (status 2) and a second run does not overwrite
    assert_eq!(upstream_runtime::wheelhouse_cli::run(&args), 2);
    let _ = std::fs::remove_dir_all(&td);
}

fn ring_sha256(data: &[u8]) -> Vec<u8> {
    let hexed = sha256_hex(data);
    (0..hexed.len() / 2).map(|i| u8::from_str_radix(&hexed[2 * i..2 * i + 2], 16).unwrap()).collect()
}

#[test]
fn verify_stops_at_a_failed_closure_without_installing() {
    let td = std::env::temp_dir().join(format!("nwv-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&td);
    std::fs::create_dir_all(&td).unwrap();
    std::fs::write(td.join("closure.json"), r#"{"status":"FAIL"}"#).unwrap();
    let args: Vec<String> = ["verify", "--wheelhouse-dir", td.to_str().unwrap(), "--venv", td.join("v").to_str().unwrap(), "--generated-utc", "2026-01-01T00:00:00Z"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(upstream_runtime::wheelhouse_cli::run(&args), 3);
    let res = loads(&std::fs::read(td.join("verify.json")).unwrap()).unwrap();
    assert_eq!(res.get("verdict").and_then(|v| v.str()), Some("FAIL"));
    assert_eq!(res.get("gates").and_then(|g| g.get("closure")).and_then(|c| c.get("result")).and_then(|r| r.str()), Some("FAIL"));
    assert!(!td.join("v").exists());
    let _ = std::fs::remove_dir_all(&td);
}
