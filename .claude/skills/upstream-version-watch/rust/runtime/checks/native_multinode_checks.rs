//! Focused checks of the `native_multinode_serve` part-A port: the marker-owned tree primitives, the shell quoting and the pure builders.
//! Parity of `plan` / `client` / `down` against the Python original is the campaign's own comparison.

use upstream_runtime::native_multinode::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("nm-checks-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn quoting_matches_shlex() {
    assert_eq!(shlex_quote(""), "''");
    assert_eq!(shlex_quote("a-b/c.d=e"), "a-b/c.d=e");
    assert_eq!(shlex_quote("a b'c"), "'a b'\"'\"'c'");
    assert_eq!(shlex_join(&["x".into(), "<y>".into()]), "x '<y>'");
}

#[test]
fn remote_root_and_marker() {
    assert!(validate_remote_root("/a/b/r1", "r1").is_ok());
    assert!(validate_remote_root("/a/../r1", "r1").is_err());
    assert!(validate_remote_root("a/r1", "r1").is_err());
    assert!(validate_remote_root("/a/r2", "r1").is_err());
    assert_eq!(String::from_utf8(marker_bytes("r1")).unwrap(), "{\"created_by\":\"native_multinode_serve.py/v1\",\"run_id\":\"r1\"}\n");
}

#[test]
fn owned_tree_removal_is_marker_gated() {
    let base = scratch("own");
    let root = base.join("r1");
    let runner = Local::new(root.to_str().unwrap(), "main");
    runner.init_root(false).unwrap();
    runner.put("sub/dir/f.txt", b"x", 0o600).unwrap();
    assert!(runner.put("sub/dir/f.txt", b"y", 0o600).is_err(), "put never overwrites");
    assert!(runner.put("../escape", b"y", 0o600).is_err());
    std::os::unix::fs::symlink("/nonexistent-target", root.join("sub/link")).unwrap();
    runner.remove_owned_tree("r1").unwrap();
    assert!(!root.exists());
    // an unmarked directory is refused
    let other = base.join("r2");
    std::fs::create_dir(&other).unwrap();
    assert!(safe_remove_owned_tree(&other, "r2").is_err());
    assert!(other.exists());
}

#[test]
fn argv_builders() {
    let spec = NodeSpec {
        name: "main".into(),
        host_ip: "1.2.3.4".into(),
        work_dir: "/w".into(),
        root: "/r/x".into(),
        short: "/tmp/native-mn/x".into(),
        session_py: "s.py".into(),
        renew_sh: "renew.sh".into(),
        watchdog_sh: "wd.sh".into(),
        node_dir: "/n".into(),
        wheel_tool: "t".into(),
        wheel_accept: "a".into(),
    };
    let w = watchdog_argv(&spec, "7", "99");
    assert_eq!(w.join(" "), "bash wd.sh --pgid 7 --starttime 99 --thresh-mib 10240 --interval-s 2");
    assert_eq!(renew_argv(&spec, "7", "99", 60).last().unwrap(), "60");
    assert_eq!(pip_freeze_argv("/r/x")[0], "/r/x/venv/bin/python");
    assert_eq!(wheelhouse_verify_argv("t", "/r/x", "U", "a")[..3], ["python3", "t", "verify"]);
    let env: Env = [("B".to_string(), "2".to_string()), ("A".to_string(), "1".to_string())].into();
    assert_eq!(env_argv(&env, &["cmd".into()]), ["env", "-i", "A=1", "B=2", "cmd"]);
    let mk = mkdirs_argv(&spec);
    assert_eq!(mk[..2], ["mkdir", "-p"]);
    assert!(mk.contains(&"/r/x/cache/xdg".to_string()) && mk.contains(&"/tmp/native-mn/x/ray".to_string()));
    assert_eq!(utc(1_790_000_000.0), "2026-09-21T14:13:20Z");
}
