//! Own regression of the two residuals of the P23 review (M2 self-test abbreviations, M1 stderr/stdout order). The parser
//! expectations are the CPython 3.12 `argparse` texts over the parser statements of the original `main()`, frozen in
//! `checks/data/scan_node/residual_prefix_stderr.json` (captured by the retired Python mode of this file, plan_26111615
//! pass 3); the native program is run directly. The original program is NOT run: no scan, no DNS/NAS/SSH. The former
//! check of the Python importer adapter's self-test selector is retired with the adapter route (the launcher
//! `scan_node.sh` starts the native program for every argv shape).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

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
    for (argv, rc) in native_cases.iter() {
        let output = Command::new(elf)
            .env_clear()
            .env("COLUMNS", "80")
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", &scratch.root)
            .args(*argv)
            .stdin(Stdio::null())
            .output()
            .expect("run");
        // the native program itself answers every shape (there is no original owner to reach any more)
        assert_eq!(output.status.code(), Some(*rc), "{argv:?}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn ambiguous_and_invalid_prefixes_end_in_the_stdlib_parser_errors_byte_for_byte_before_any_business(
) {
    let elf = env!("CARGO_BIN_EXE_scan_node");
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/scan_node/residual_prefix_stderr.json");
    let text = std::fs::read_to_string(&golden).unwrap_or_else(|e| panic!("NOT_RUN: {}: {e}", golden.display()));
    let PyValue::Dict(doc) = loads(&text).expect("golden JSON") else { panic!("golden is an object") };
    let Some(PyValue::List(rows)) = doc.get("rows") else { panic!("golden rows") };
    assert_eq!(rows.len(), 5);
    for row in rows {
        let PyValue::Dict(row) = row else { panic!("row") };
        let argv: Vec<String> = match row.get("argv") {
            Some(PyValue::List(words)) => words.iter().map(|w| match w {
                PyValue::Str(t) => t.to_utf8().expect("UTF-8"),
                _ => panic!("argv word"),
            }).collect(),
            _ => panic!("argv"),
        };
        assert!(matches!(row.get("kind"), Some(PyValue::Str(k)) if k.equals("exit2")), "{argv:?}");
        let expected_text = match row.get("stderr") {
            Some(PyValue::Str(t)) => t.to_utf8().expect("UTF-8"),
            _ => panic!("stderr"),
        };
        let expected = format!("{}}}", serde_like(&expected_text));
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
            expected,
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
