//! Own regression of the two residuals of the P23 review (M2 self-test abbreviations, M1 stderr/stdout order). Source-derived: every expectation is the
//! CPython 3.12.3 `argparse` rule or the order of the original `main()`, taken from the stdlib and from the original SOURCE (its parser statements by AST,
//! executed against the stdlib only). The original program is NOT run: no `--self-test`, no `--self*`, no scan, no DNS/NAS/SSH; the owner is a trap and
//! the native route runs only argument shapes that end in a parser error or help before any business. Not the saved Original56 pairs.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PY: &str = r####"
import ast, sys, argparse, io, contextlib, os, json
scripts, mode = sys.argv[1], sys.argv[2]
os.environ['COLUMNS'] = '80'
sys.argv[0] = 'scan_node.py'
tree = ast.parse(open(os.path.join(scripts, 'scan_node_original.py'), encoding='utf-8').read())
main = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'main')
stmts = []
for st in main.body:
    stmts.append(st)
    if isinstance(st, ast.Assign) and isinstance(st.value, ast.Call) and getattr(st.value.func, 'attr', '') == 'parse_args':
        stmts.pop(); break
module = ast.Module(body=stmts, type_ignores=[]); ast.fix_missing_locations(module)
def parser():
    ns = {'argparse': argparse}
    exec(compile(module, 'scan_node_original.py', 'exec'), ns)
    return ns['ap']
def run_parse(argv):
    err = io.StringIO(); out = io.StringIO()
    try:
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(out):
            ns = parser().parse_args(argv)
        return 'ok', ns, err.getvalue()
    except SystemExit as stop:
        return 'exit%s' % stop.code, None, err.getvalue()
if mode == 'text':
    kind, ns, err = run_parse(sys.argv[3:])
    sys.stdout.write(json.dumps({'kind': kind, 'stderr': err}))
    sys.exit(0)
sys.path.insert(0, scripts)
import scan_node as m
if mode == 'selector':
    ap = parser()
    opts = sorted({s for a in ap._actions for s in a.option_strings if s.startswith('--')})
    problems = []
    if sorted(m._SN_LONG_OPTIONS) != opts:
        problems.append(['option list drift', sorted(m._SN_LONG_OPTIONS), opts])
    tokens = set(['--', '-', '-h', '-s', 'self-test', 'self', '--self x', ' --self', '---self', '--self-test ', '--Self', '--SELF-TEST', '--s=x', '--s', '--se=', '--self=x', '--self-test=x', '--self-testx', '--sel-', '--sub', '--su', '--su=1', '--t', '--top', '--p', '--pe', '--per', '--e', '--em', '--m', '--c', '--b', '--bw', '--bandwidth', '--h', '--he', '--help=x'])
    for o in opts:
        for k in range(2, len(o) + 1):
            tokens.add(o[:k]); tokens.add(o[:k] + '=v'); tokens.add(o[:k] + '=')
        tokens.add(o + 'x'); tokens.add(o + '-')
    mism = []
    for t in sorted(tokens):
        err = io.StringIO()
        try:
            with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
                r = parser()._parse_optional(t)
            truth = r is not None and r[0] is not None and r[0].dest == 'self_test'
        except SystemExit:
            truth = False
        got = m._sn_selects_self_test([t])
        if truth != got:
            mism.append([t, truth, got])
    # composite safety: whatever the classifier decides, the stdlib parser must agree that a selected argv can only be the self-test or a parser error,
    # and an unselected argv can never be a self-test (so the route can neither start the original's business nor miss its self-test)
    base = sorted(t for t in tokens if t.startswith('--') and len(t) <= 14)
    contexts = [[], ['--topology', 'single'], ['--peer-ssh'], ['--compose', 'c.yaml', '--'], ['-h'], ['--peer-port', '22', '--bw-floor', '1']]
    viol = []; n = 0
    for ctx in contexts:
        for t in base:
            argv = ctx + [t]; n += 1
            kind, ns, err = run_parse(argv)
            cls = m._sn_selects_self_test(argv)
            if kind == 'ok' and ns.self_test and not cls:
                viol.append(['missed self-test', argv])
            if kind == 'ok' and cls and not ns.self_test:
                viol.append(['routed a non-self-test parse', argv])
    print(json.dumps({'tokens': len(tokens), 'token_mismatches': mism, 'composites': n, 'violations': viol, 'problems': problems}))
    sys.exit(1 if (mism or viol or problems) else 0)
if mode == 'route':
    argv = sys.argv[3:]
    sys.argv = ['scan_node.py'] + argv
    calls = []
    def trap():
        calls.append(list(sys.argv)); return 77
    m._SN_LEGACY.main = trap
    rc = m.main()
    sys.stdout.flush()
    sys.stderr.flush()
    sys.stdout.write('\nROUTE_RESULT ' + json.dumps({'rc': rc, 'trap_calls': len(calls)}) + '\n')
"####;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root =
            std::env::temp_dir().join(format!("scan-node-residual-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        Scratch { root }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn scripts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../terraforming_node/scripts")
}

fn python(mode: &str, args: &[&str], env: &[(&str, String)]) -> (i32, String, String) {
    let mut command = Command::new("python3");
    command
        .arg("-B")
        .arg("-c")
        .arg(PY)
        .arg(scripts())
        .arg(mode)
        .args(args);
    command
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("COLUMNS", "80")
        .stdin(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("python3");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_adapters_selection_of_the_self_test_equals_the_stdlib_argparse_rule() {
    let (code, out, err) = python("selector", &[], &[]);
    assert_eq!(code, 0, "selector check failed:\n{out}\n{err}");
    assert!(
        out.contains("\"token_mismatches\": []")
            && out.contains("\"violations\": []")
            && out.contains("\"problems\": []"),
        "{out}"
    );
}

#[test]
fn every_self_test_abbreviation_and_every_other_shape_is_native_and_never_reaches_the_owner()
{
    let scratch = Scratch::new("route");
    let elf = env!("CARGO_BIN_EXE_scan_node");
    let native_cases: [(&[&str], i32); 17] = [
        // c31: `--self-test` and its abbreviations are native now; no argv shape reaches the original owner (trap_calls stays 0)
        (&["--self-test"], 0),
        (&["--self-tes"], 0),
        (&["--self-t"], 0),
        (&["--self"], 0),
        (&["--sel"], 0),
        (&["--se"], 0),
        (&["--self-test", "--bogus"], 2),
        (&["--topology", "single", "--self-tes"], 0),
        (&["--self=x"], 2),
        (&["--s"], 2),
        (&["--su"], 2),
        (&["--sub"], 2),
        (&["--sel-x"], 2),
        (&["--self-testx"], 2),
        (&["--bogus"], 2),
        (&["--help"], 0),
        (&["-h"], 0),
    ];
    for (n, (argv, rc)) in native_cases.iter().enumerate() {
        let observer = scratch.root.join(format!("obs-native-{n}"));
        let (code, out, err) = python(
            "route",
            argv,
            &[
                ("SCAN_NODE_BIN", elf.to_owned()),
                (
                    "SCAN_NODE_ROUTE_OBSERVER",
                    observer.to_string_lossy().into_owned(),
                ),
            ],
        );
        assert_eq!(code, 0, "{argv:?}: {err}");
        assert!(
            out.contains(&format!("ROUTE_RESULT {{\"rc\": {rc}, \"trap_calls\": 0}}")),
            "{argv:?}: {out}"
        );
        let seen = std::fs::read_to_string(&observer).unwrap_or_default();
        assert!(
            seen.contains("NativeCli") && !seen.contains("CompatOriginalCli"),
            "{argv:?}: {seen}"
        );
    }
}

#[test]
fn ambiguous_and_invalid_prefixes_end_in_the_stdlib_parser_errors_byte_for_byte_before_any_business(
) {
    let elf = env!("CARGO_BIN_EXE_scan_node");
    for argv in [
        vec!["--s"],
        vec!["--su"],
        vec!["--sel-x"],
        vec!["--self-testx"],
        vec!["--sub", "x", "--s"],
    ] {
        let (_, text, _) = python("text", &argv, &[]);
        let expected = text.split("\"stderr\": ").nth(1).expect("stderr field");
        let output = Command::new(elf)
            .env_clear()
            .env("COLUMNS", "80")
            .env("PATH", "/usr/bin:/bin")
            .args(&argv)
            .stdin(Stdio::null())
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        assert!(output.stdout.is_empty(), "{argv:?}: nothing on stdout");
        let native = String::from_utf8(output.stderr).expect("utf-8");
        let native_json = serde_like(&native);
        assert_eq!(
            format!("{native_json}}}"),
            expected.to_owned(),
            "{argv:?}: native stderr != stdlib argparse stderr"
        );
    }
}

/// `json.dumps(text)` with ensure_ascii, for the characters these parser messages contain.
fn serde_like(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fake_commands(scratch: &Scratch) -> String {
    let dir = scratch.root.join("own-bin");
    std::fs::create_dir_all(&dir).expect("dir");
    let files: [(&str, &str); 4] = [
        ("git", "printf 'single-node\\n'\n"),
        ("nvcc", "printf 'Cuda compilation tools, release 13.2, V\\n'\n"),
        ("nvidia-smi", "case \"$1\" in -L) printf 'GPU 0: x\\n';; *name*) printf 'NVIDIA GB10\\n';; *driver*) printf '580.95.05\\n';; *) printf '[N/A], 12.1\\n';; esac\n"),
        ("ssh", "printf 'Host key verification failed.\\n' >&2\nexit 255\n"),
    ];
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    dir.to_string_lossy().into_owned()
}

fn scanner(scratch: &Scratch, shell_suffix: &str, args: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let case = scratch.root.join("case");
    std::fs::create_dir_all(case.join("output/single")).expect("case");
    std::fs::write(
        case.join("output/single/manifest.yaml"),
        "topology: single\nmodel_source: managed\nnas_model_path: /nonexistent\n",
    )
    .expect("manifest");
    let path = fake_commands(scratch);
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(format!("exec \"$0\" \"$@\"{shell_suffix}"))
        .arg(env!("CARGO_BIN_EXE_scan_node"));
    let output = command
        .env_clear()
        .env("PATH", format!("{path}:/usr/bin:/bin"))
        .env("LOGNAME", "tester")
        .env("TZ", "UTC")
        .env("LC_ALL", "C.UTF-8")
        .current_dir(&case)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run");
    (
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
    )
}

#[test]
fn the_diagnostics_precede_the_output_on_a_merged_stream_even_when_the_output_overruns_the_block_buffer(
) {
    let scratch = Scratch::new("m1");
    let long_dir = "d".repeat(9000);
    let args = [
        "--topology",
        "single",
        "--peer-ssh",
        "user@203.0.113.11",
        "--sub-work-dir",
        long_dir.as_str(),
    ];
    // separate streams: raw data of each, untouched
    let (code, stdout, stderr) = scanner(&scratch, "", &args);
    let (stdout, stderr) = (
        String::from_utf8(stdout).expect("utf-8"),
        String::from_utf8(stderr).expect("utf-8"),
    );
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stdout.starts_with("{\n") && stdout.ends_with("}\n") && stdout.len() > 8192,
        "the output must overrun a 4096/8192 byte block: {}",
        stdout.len()
    );
    assert!(stderr.contains("[scan] 서브 HW 프로브 실패"), "{stderr}");
    // merged stream (2>&1): every diagnostic byte comes before the first output byte, as in the original (no stderr write follows the first stdout write)
    let (code, merged, _) = scanner(&scratch, " 2>&1", &args);
    let merged = String::from_utf8(merged).expect("utf-8");
    assert_eq!(code, 2);
    assert_eq!(
        merged,
        format!("{stderr}{stdout}"),
        "merged stream must be exactly stderr then stdout"
    );
}

#[test]
fn a_terminal_cannot_be_opened_under_the_guard_so_the_tty_order_is_proven_only_by_the_merged_stream_rule(
) {
    let probe = Command::new("python3")
        .arg("-B")
        .arg("-c")
        .arg("import pty\npty.openpty()")
        .output()
        .expect("python3");
    if probe.status.success() {
        eprintln!("PTY available: a real terminal run is possible but is not written in this proposal (NOT_RUN)");
    } else {
        eprintln!(
            "NOT_RUN: no pty device in this environment ({})",
            String::from_utf8_lossy(&probe.stderr)
                .lines()
                .last()
                .unwrap_or("")
        );
    }
}
