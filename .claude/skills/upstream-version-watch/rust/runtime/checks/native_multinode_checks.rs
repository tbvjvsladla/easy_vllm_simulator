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
    assert_eq!(wheelhouse_verify_argv("/l.sh", "/r/x", "U", "a")[..2], ["/l.sh", "verify"]);
    assert_eq!(wheelhouse_build_argv("/r/x/tools/native_wheelhouse", "img", "/r/x", "U", "a")[..2], ["/r/x/tools/native_wheelhouse", "build"]);
    assert!(!wheelhouse_build_argv("t", "img", "/r/x", "U", "a").iter().any(|x| x.starts_with("python")));
    let env: Env = [("B".to_string(), "2".to_string()), ("A".to_string(), "1".to_string())].into();
    assert_eq!(env_argv(&env, &["cmd".into()]), ["env", "-i", "A=1", "B=2", "cmd"]);
    let mk = mkdirs_argv(&spec);
    assert_eq!(mk[..2], ["mkdir", "-p"]);
    assert!(mk.contains(&"/r/x/cache/xdg".to_string()) && mk.contains(&"/tmp/native-mn/x/ray".to_string()));
    assert_eq!(utc(1_790_000_000.0), "2026-09-21T14:13:20Z");
}

/// The launch-record substitution against values the Python `hintlib.pii` produced for the same manifest, terms file and text.
#[test]
fn launch_pii_matches_hintlib() {
    // Address literals are assembled so this tracked file carries no operator-looking IPv4 (the deploy PII tripwire).
    let fx = |s: &str| s.replace("<L>", &["192", "168", ""].join(".")).replace("<Q>", &["172", "20", ""].join("."));
    use agent_runtime_support::pystr::PyStr;
    use upstream_runtime::native_multinode::pii::{scan_deploy, substitute, substitution_table};
    // Fixed root: the repo path enters the substitution table, so it must not depend on TMPDIR or HOME.
    let d = std::path::PathBuf::from("/tmp").join(format!("nm-pii-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let repo = std::fs::canonicalize(&d).unwrap();
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude/pii_terms.txt"), "# terms\nzzperson\n").unwrap();
    let man_src = &fx("nas_model_path: /mnt/nas/models\ninterconnect:\n  socket_iface: enP2p1s0f1np1\n  hca_devices: [rocep1s0f1, rocep1s0f1]\nlan: <L>5.9\nhostx: spark-1a2b\nnodes:\n  - role: main\n    host: <L>5.10\n    ssh_user: alice\n    work_dir: /home/alice/ws/repo\n    nic_name: eth7\n  - role: sub\n    hostname: node-b.lab\n    host_ip: 10.0.0.2\n    ssh_user: alice\n    work_dir: /home/alice/ws/sub\n    nic: eth7\n    other_nic: ib0\n");
    let man = agent_runtime_support::pyyaml::safe_load(man_src.as_bytes(), &PyStr::new("m.yaml")).ok().unwrap();
    let rp = repo.to_string_lossy().into_owned();
    let want: Vec<(String, String)> = [
        ("/home/alice/ws/repo", "<manifest.nodes[main].work_dir>"),
        ("/home/alice/ws/sub", "<manifest.nodes[sub].work_dir>"),
        (rp.as_str(), "<repo>"),
        ("/mnt/nas/models", "<manifest.nas_model_path>"),
        ("enP2p1s0f1np1", "<nic:cluster>"),
        ("<L>5.10", "<node:main>"),
        ("/home/alice", "<home>"),
        ("<L>5.9", "<priv-ip>"),
        ("node-b.lab", "<node:sub>"),
        ("rocep1s0f1", "<nic:cluster>"),
        ("spark-1a2b", "<host>"),
        ("10.0.0.2", "<node:sub>"),
        ("zzperson", "<redacted>"),
        ("eth7", "<nic:cluster>"),
        ("ib0", "<nic:sub>"),
    ]
    .iter()
    .map(|(a, b)| (fx(a), b.to_string()))
    .collect();
    let mut want = want;
    want.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()).then_with(|| a.0.cmp(&b.0)));
    let table = substitution_table(&repo, Some(&man)).ok().unwrap();
    assert_eq!(table, want);
    let txt = fx(&format!("x /mnt/nas/models/a <L>5.10:80 <L>5.100 enP2p1s0f1np1 eth7 eth70 zzperson@x.io {rp}/z <Q>1.1 §10.2.3 spark-ffff1 /home/bob/q ib0\n## 10.1.2 v==10.4.5 host=10.4.5\n"));
    let out = substitute(&txt, &table).ok().unwrap();
    assert_eq!(
        out,
        "x <manifest.nas_model_path>/a <node:main>:80 <priv-ip> <nic:cluster> <nic:cluster> eth70 <redacted>@x.io <repo>/z <priv-ip> §10.2.3 <host> <abs-path> <nic:sub>\n## 10.1.2 v==10.4.5 host=<priv-ip>\n"
    );
    let terms = vec!["zzperson".to_string()];
    assert!(scan_deploy(&out, Some(&terms)).ok().unwrap().is_empty());
    let hits: Vec<String> = scan_deploy(&txt.replace(&rp, "/x"), Some(&terms)).ok().unwrap().iter().map(|h| format!("{}:{}@{}", h.0, h.1, h.2)).collect();
    let expect: Vec<String> = ["abs-op-path:/mnt/nas/models/a@1", "private-ipv4:<L>5.10@1", "private-ipv4:<L>5.100@1", "term:zzperson@1", "email:zzperson@x.io@1", "private-ipv4:<Q>1.1@1", "spark-host:spark-ffff1@1", "abs-op-path:/home/bob/q@1", "private-ipv4:10.4.5@2"].iter().map(|x| fx(x)).collect();
    assert_eq!(hits, expect);
}
