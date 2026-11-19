use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_verdict_clock_scan")
}

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ALLOW_REL: &str = ".claude/rust/py-boundary-guard/verdict_clock_allowlist.tsv";
const PLANTED: [(&str, &str); 6] = [
    ("planted.rs", ".claude/rust/x/src/planted.rs"),
    ("planted.sh", ".claude/x/scripts/planted.sh"),
    ("planted.ts", ".claude/skills/wiki-desk/src/planted.ts"),
    ("regression_lock.rs", ".claude/rust/x/src/regression_lock.rs"),
    ("regression_lock.sh", ".claude/x/scripts/regression_lock.sh"),
    ("regression_lock.ts", ".claude/skills/wiki-desk/src/regression_lock.ts"),
];

fn repo(name: &str) -> Repo {
    let dir = std::env::temp_dir().join(format!("vcscan-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(s.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn fixture(name: &str) -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/verdict_clock");
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    git(dir, &["add", "-A"]);
    let out = Command::new(bin()).args(args).arg("--repo").arg(dir).output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn check(dir: &Path) -> (i32, String, String) {
    run(dir, &["check"])
}

const NO_ROWS: &str = "# no rows\n";
const TWO_READS: &str = "fn f() {\n    let _ = std::time::Instant::now();\n    let _ = std::time::Instant::now();\n}\n";
const TWO_READS_REL: &str = ".claude/rust/x/src/two.rs";

fn allow_two_reads(count: usize) -> String {
    format!("# rows\n{TWO_READS_REL}\tstd::time::Instant::now()\t{count}\truntime-fact\tCK-000 test row\n")
}

#[test]
fn every_planted_read_fails_with_its_line() {
    let r = repo("planted");
    for (name, rel) in PLANTED {
        write(&r.0, rel, &fixture(name));
    }
    write(&r.0, ALLOW_REL, NO_ROWS);
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    let expected = fixture("expected.tsv");
    for row in expected.lines() {
        let [name, line, text] = row.split('\t').collect::<Vec<_>>()[..] else { panic!("{row}") };
        let rel = PLANTED.iter().find(|(n, _)| *n == name).unwrap().1;
        let want = format!("CLOCK-VIOLATION {rel}:{line}\t{text}\n");
        assert!(out.contains(&want), "missing {want:?}\n{out}");
    }
    let found = out.lines().filter(|l| l.starts_with("CLOCK-VIOLATION ")).count();
    assert_eq!(found, expected.lines().count(), "{out}");
    assert!(err.contains("하네스 판정은 벽시계를 읽지 않는다"), "{err}");
}

#[test]
fn mentions_in_comments_and_strings_are_not_reads() {
    let r = repo("mentions");
    write(&r.0, ".claude/rust/x/src/m.rs", "// SystemTime::now()\n/// Instant::now()\nfn f() {\n    let _ = \"SystemTime::now()\";\n}\nuse a::now;\n");
    write(&r.0, ALLOW_REL, NO_ROWS);
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS 0 clock reads"), "{out}");
}

#[test]
fn a_listed_count_is_exact() {
    let r = repo("exact");
    write(&r.0, TWO_READS_REL, TWO_READS);
    write(&r.0, ALLOW_REL, &allow_two_reads(2));
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS 2 clock reads within verdict_clock_allowlist.tsv (1 rows)"), "{out}");

    write(&r.0, ALLOW_REL, &allow_two_reads(1));
    let (code, out, _) = check(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("CLOCK-VIOLATION {TWO_READS_REL}:2\tstd::time::Instant::now() (row allows 1, found 2)")), "{out}");

    write(&r.0, ALLOW_REL, &allow_two_reads(3));
    let (code, out, _) = check(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("CLOCK-STALE {TWO_READS_REL}\tstd::time::Instant::now(): listed 3, found 2")), "{out}");
}

#[test]
fn a_row_for_a_read_that_is_gone_is_stale() {
    let r = repo("gone");
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    write(&r.0, ALLOW_REL, &allow_two_reads(2));
    let (code, out, _) = check(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("CLOCK-STALE {TWO_READS_REL}\tstd::time::Instant::now(): listed 2, found 0")), "{out}");
}

#[test]
fn bad_inputs_fail_closed() {
    let r = repo("bad");
    write(&r.0, TWO_READS_REL, TWO_READS);
    for bad in [
        format!("{TWO_READS_REL}\tstd::time::Instant::now()\t2\truntime-fact\n"),
        format!("{TWO_READS_REL}\tstd::time::Instant::now()\t2\tbomb\twhy\n"),
        format!("{TWO_READS_REL}\tstd::time::Instant::now()\t0\truntime-fact\twhy\n"),
    ] {
        write(&r.0, ALLOW_REL, &bad);
        let (code, out, err) = check(&r.0);
        assert_eq!(code, 2, "{bad:?}\n{out}{err}");
    }
    write(&r.0, ALLOW_REL, &allow_two_reads(2));
    let (code, _, err) = run(&r.0, &["check", "--allowlist", "/nonexistent/allow.tsv"]);
    assert_eq!(code, 2, "{err}");
    write(&r.0, ".claude/rust/x/src/broken.rs", "fn f( {\n");
    let (code, _, err) = check(&r.0);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("broken.rs"), "{err}");
}

#[test]
fn a_transitional_row_is_refused() {
    let r = repo("transitional");
    write(&r.0, TWO_READS_REL, TWO_READS);
    write(&r.0, ALLOW_REL, &format!("{TWO_READS_REL}\tstd::time::Instant::now()\t2\ttransitional\tc24 stage B\n"));
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("class must be one of"), "{err}");
}

#[test]
fn usage_errors_are_exit_2() {
    for args in [&["check", "--repo"][..], &["scan"][..], &["--self-test", "x"][..], &[][..]] {
        let out = Command::new(bin()).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn self_test_passes() {
    let out = Command::new(bin()).arg("--self-test").output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(String::from_utf8_lossy(&out.stdout).contains("--self-test PASS 140 planted reads"));
}

#[test]
fn a_missing_default_allowlist_is_an_input_error() {
    let r = repo("noallow");
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("cannot read allowlist") && err.contains("verdict_clock_allowlist.tsv"), "{err}");
}

#[test]
fn an_unreadable_tracked_source_is_an_input_error() {
    let r = repo("unreadable");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    for tracked in [".claude/x/scripts/locked", ".claude/rust/x/src/locked.rs"] {
        write(&r.0, tracked, "#!/bin/bash\necho\n");
        git(&r.0, &["add", "-A"]);
        let path = r.0.join(tracked);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let out = Command::new(bin()).args(["check", "--repo"]).arg(&r.0).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{err}");
        assert!(err.contains(&format!("cannot read tracked file {tracked}: Is a directory")), "{err}");
        std::fs::remove_dir(&path).unwrap();
        git(&r.0, &["rm", "-q", "--cached", tracked]);
    }
}

#[test]
fn a_gitlink_is_not_read() {
    let r = repo("gitlink");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    git(&r.0, &["add", "-A"]);
    for (gitlink, work_tree_dir) in [("vendor/sub", true), ("vendor/gone.sh", false)] {
        git(&r.0, &["update-index", "--add", "--cacheinfo", &format!("160000,{},{gitlink}", "5e9e0a08aca11df66d6a7d950594226d3b46bad2")]);
        if work_tree_dir {
            std::fs::create_dir_all(r.0.join(gitlink)).unwrap();
        }
    }
    let out = Command::new(bin()).args(["check", "--repo"]).arg(&r.0).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("scanned 1 .rs, 0 shell, 0 .ts · 0 deleted from the work tree"), "{stdout}");
}

#[test]
fn a_tracked_path_that_is_not_utf8_is_an_input_error() {
    use std::os::unix::ffi::OsStrExt;
    let r = repo("nonutf8");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    let name = std::ffi::OsStr::from_bytes(b"s/bad\xff.sh");
    std::fs::create_dir_all(r.0.join("s")).unwrap();
    std::fs::write(r.0.join(name), "echo ok\n").unwrap();
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("a tracked path is not UTF-8: s/bad\u{fffd}.sh"), "{err}");
}

#[test]
fn a_typescript_source_left_open_is_an_input_error() {
    let r = repo("tsopen");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/skills/wiki-desk/src/open.ts", "const a = 1;\nconst t = `x\n${a}\n");
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("cannot read .claude/skills/wiki-desk/src/open.ts as TypeScript (line 2: a template literal is not closed"),
        "{err}"
    );
}

#[test]
fn other_inputs_that_cannot_be_read_are_errors() {
    let r = repo("inputs");
    write(&r.0, ".claude/rust/py-boundary-guard/src/a.rs", "fn f() { let _ = std::time::SystemTime::now(); }\n");
    write(&r.0, ALLOW_REL, NO_ROWS);
    git(&r.0, &["add", "-A"]);
    let crate_dir = r.0.join(".claude/rust/py-boundary-guard");
    let dir_allowlist = r.0.join("allow.d");
    std::fs::create_dir_all(&dir_allowlist).unwrap();
    let outside = std::env::temp_dir().join(format!("vcscan-nogit-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("a.rs"), "fn f() {}\n").unwrap();
    let allowlist = r.0.join(ALLOW_REL);
    let cases: [(&[&std::ffi::OsStr], &str); 3] = [
        (&[crate_dir.as_os_str()], "cannot read allowlist"),
        (&[r.0.as_os_str(), "--allowlist".as_ref(), dir_allowlist.as_os_str()], "cannot read allowlist"),
        (&[outside.as_os_str(), "--allowlist".as_ref(), allowlist.as_os_str()], "git ls-files failed"),
    ];
    for (args, message) in cases {
        let out = Command::new(bin()).args(["check", "--repo"]).args(args).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {err}");
        assert!(err.contains(message), "{args:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn nothing_to_scan_is_an_input_error() {
    let r = repo("empty");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, "README.md", "# x\n");
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("no tracked .rs, shell or .ts source to scan"), "{err}");
}

#[test]
fn a_deleted_tracked_file_is_skipped_and_counted() {
    let r = repo("deleted");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    write(&r.0, ".claude/rust/x/src/gone.rs", "fn g() { let _ = std::time::Instant::now(); }\n");
    write(&r.0, ".claude/x/scripts/sparse.sh", "echo sparse\n");
    git(&r.0, &["add", "-A"]);
    git(&r.0, &["update-index", "--skip-worktree", ".claude/x/scripts/sparse.sh"]);
    std::fs::remove_file(r.0.join(".claude/rust/x/src/gone.rs")).unwrap();
    std::fs::remove_file(r.0.join(".claude/x/scripts/sparse.sh")).unwrap();
    let out = Command::new(bin()).args(["check", "--repo"]).arg(&r.0).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("scanned 1 .rs, 0 shell, 0 .ts · 2 deleted from the work tree"), "{stdout}");
    assert!(stdout.contains("DELETED-FROM-WORK-TREE .claude/rust/x/src/gone.rs\n"), "{stdout}");
    assert!(stdout.contains("DELETED-FROM-WORK-TREE .claude/x/scripts/sparse.sh\n"), "{stdout}");
}

#[test]
fn a_symlink_is_not_read() {
    let r = repo("symlink");
    write(&r.0, ALLOW_REL, NO_ROWS);
    write(&r.0, ".claude/rust/x/src/clean.rs", "fn f() {}\n");
    let target = std::env::temp_dir().join(format!("vcscan-link-target-{}.rs", std::process::id()));
    std::fs::write(&target, TWO_READS).unwrap();
    std::os::unix::fs::symlink(&target, r.0.join(".claude/rust/x/src/link.rs")).unwrap();
    std::os::unix::fs::symlink("/nonexistent/target.rs", r.0.join(".claude/rust/x/src/broken.rs")).unwrap();
    let (code, out, err) = check(&r.0);
    let _ = std::fs::remove_file(&target);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("scanned 1 .rs, 0 shell, 0 .ts · 0 deleted from the work tree"), "{out}");
}

#[test]
fn test_scripts_under_checks_data_are_scanned() {
    let r = repo("checksdata");
    write(&r.0, ALLOW_REL, NO_ROWS);
    let case = ".claude/rust/policy-audit/checks/data/cli/PA-x/case.sh";
    write(&r.0, case, &fixture("planted.sh"));
    write(&r.0, ".claude/rust/parity/data/S1/run.sh", &fixture("planted.sh"));
    write(&r.0, ".claude/rust/py-boundary-guard/checks/data/verdict_clock/planted.sh", &fixture("planted.sh"));
    let scanned_planes = [
        case,
        ".claude/rust/policy-audit/checks/cases/PA-x/case.sh",
        ".claude/skills/adversarial-benchmark/rust/checks/data/sweep_meta/zz/input/x.sh",
        ".claude/rust/policy-audit/checks/data/verdict_clock/zz.sh",
    ];
    for rel in &scanned_planes[1..] {
        write(&r.0, rel, &fixture("planted.sh"));
    }
    write(&r.0, ".claude/rust/policy-audit/checks/data/zz.rs", "fn f() { let _ = std::time::Instant::now(); }\n");
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    let planted_sh = fixture("expected.tsv").lines().filter(|row| row.starts_with("planted.sh\t")).count();
    for rel in scanned_planes {
        assert_eq!(out.lines().filter(|l| l.starts_with(&format!("CLOCK-VIOLATION {rel}:"))).count(), planted_sh, "{rel}\n{out}");
    }
    assert!(out.contains("CLOCK-VIOLATION .claude/rust/policy-audit/checks/data/zz.rs:1\tstd::time::Instant::now()\n"), "{out}");
    assert_eq!(out.lines().filter(|l| l.starts_with("CLOCK-VIOLATION ")).count(), 4 * planted_sh + 1, "{out}");
}

#[test]
fn a_shell_script_is_known_by_its_shebang() {
    let r = repo("shebang");
    write(&r.0, ALLOW_REL, NO_ROWS);
    let planted = fixture("planted.sh");
    let body = planted.split_once('\n').expect("a shebang line").1;
    let heads = [
        (".claude/x/a", "#!/bin/bash -e"),
        (".claude/x/b", "#!/usr/bin/env bash\r"),
        (".claude/x/c", "#!/usr/bin/env -S bash -eu"),
        (".claude/x/tool.v2", "#!/usr/bin/env bash"),
        (".claude/x/d", "#!/bin/bash"),
        (".claude/x/e", "#!/usr/bin/env zsh"),
        (".claude/x/f", "#!/usr/bin/env dash"),
        (".claude/x/script.py", "#!/usr/bin/env python3"),
    ];
    for (rel, head) in heads {
        write(&r.0, rel, &format!("{head}\n{body}"));
    }
    let (code, out, err) = check(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    let planted_sh = fixture("expected.tsv").lines().filter(|row| row.starts_with("planted.sh\t")).count();
    for (rel, _) in &heads[..heads.len() - 1] {
        let found = out.lines().filter(|l| l.starts_with(&format!("CLOCK-VIOLATION {rel}:"))).count();
        assert_eq!(found, planted_sh, "{rel}\n{out}");
    }
    assert!(!out.contains("script.py"), "{out}");
}

#[test]
fn the_real_tree_passes() {
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(bin()).arg("check").arg("--repo").arg(&real).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("[verdict_clock_scan] PASS "), "{stdout}");
}
