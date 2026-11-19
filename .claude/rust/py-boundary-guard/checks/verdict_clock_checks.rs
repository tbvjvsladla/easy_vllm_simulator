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
const PLANTED: [(&str, &str); 3] = [
    ("planted.rs", ".claude/rust/x/src/planted.rs"),
    ("planted.sh", ".claude/x/scripts/planted.sh"),
    ("planted.ts", ".claude/skills/wiki-desk/src/planted.ts"),
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
    write(&r.0, ".claude/rust/x/src/m.rs", "// SystemTime::now()\n/// Instant::now()\nfn f() {\n    let _ = \"SystemTime::now()\";\n}\n");
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
    assert!(String::from_utf8_lossy(&out.stdout).contains("--self-test PASS 21 planted reads"));
}

#[test]
fn the_real_tree_passes() {
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(bin()).arg("check").arg("--repo").arg(&real).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("[verdict_clock_scan] PASS "), "{stdout}");
}
