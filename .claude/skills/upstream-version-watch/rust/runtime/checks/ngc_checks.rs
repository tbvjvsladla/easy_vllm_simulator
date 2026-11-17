//! Checks of the `resolve_ngc_tag.py` port: the API functions through the JSON-lines evaluation on a scripted Docker boundary (the only stand-in:
//! the external `docker` tool), and the real command as a process against a literal `docker` stub script on a private `PATH`.
//!
//! The inputs are the typed cases of the saved original capture (`ids` in the comments), written out as literals; the expected values are the
//! saved originals' typed values and exception texts, never a recomputation by another implementation. The saved capture is not replayed here.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use upstream_runtime::ngc_api::evaluate_with;
use upstream_runtime::ngc_inspect::{Docker, Finished, ProcessDocker};

struct Scripted {
    answers: RefCell<Vec<Option<Finished>>>,
    argvs: RefCell<Vec<Vec<Vec<u8>>>>,
}

impl Scripted {
    fn new(answers: Vec<Option<Finished>>) -> Scripted {
        Scripted {
            answers: RefCell::new(answers),
            argvs: RefCell::new(Vec::new()),
        }
    }
}

impl Docker for Scripted {
    fn run(&self, argv: &[Vec<u8>]) -> Option<Finished> {
        self.argvs.borrow_mut().push(argv.to_vec());
        self.answers.borrow_mut().remove(0)
    }
}

fn reply(status: i32, stdout: &[u8], stderr: &[u8]) -> Option<Finished> {
    Some(Finished {
        status,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    })
}

fn request(id: &str, function: &str, args: &str) -> String {
    format!(
        r#"{{"id": "{id}", "operation": "resolve_ngc_tag.{function}", "args_dto": ["list", {args}]}}"#
    )
}

fn answer(docker: &Scripted, id: &str, function: &str, args: &str) -> String {
    let bytes = evaluate_with(request(id, function, args).as_bytes(), docker).expect("rendered");
    compact(&String::from_utf8(bytes).expect("utf-8 record"))
}

/// The record without the layout spaces after `:` and `,` outside strings (the renderer separates with `", "` and `": "`).
fn compact(text: &str) -> String {
    let (mut out, mut in_string, mut escaped, mut skip) = (String::new(), false, false, false);
    for ch in text.chars() {
        if skip && ch == ' ' {
            skip = false;
            continue;
        }
        skip = false;
        out.push(ch);
        if in_string {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == ':' || ch == ',' {
            skip = true;
        }
    }
    out
}

fn pure(id: &str, function: &str, args: &str) -> String {
    answer(&Scripted::new(Vec::new()), id, function, args)
}

fn dto_str(text: &str) -> String {
    format!(r#"["str", {}]"#, serde_like(text))
}

/// A JSON string literal (ASCII escapes only).
fn serde_like(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[test]
fn release_and_norm_follow_the_str_pattern_and_keep_the_original_object() {
    // api-release-004 (NBSP + Arabic-Indic digits), -009 (upper V), -013 (int), -015 (bytes repr), -016 (4301 digits)
    let record = pure(
        "r4",
        "release_tuple",
        &format!("[{}]", dto_str("\u{a0}v\u{662}.\u{661}\u{661}")),
    );
    assert!(
        record.contains(r#""value":["tuple",[["int","2"],["int","11"]]]"#),
        "{record}"
    );
    assert!(
        pure("r9", "release_tuple", &format!("[{}]", dto_str("V2.11")))
            .contains(r#""value":["none"]"#)
    );
    assert!(pure("r13", "release_tuple", r#"[["int", "211"]]"#)
        .contains(r#"["tuple",[["int","211"]]]"#));
    assert!(
        pure("r15", "release_tuple", r#"[["bytes", "322e3131"]]"#).contains(r#""value":["none"]"#)
    );
    let long = pure(
        "r16",
        "release_tuple",
        &format!("[{}]", dto_str(&"7".repeat(4301))),
    );
    assert!(
        long.contains(r#""type":"ValueError""#) && long.contains("Exceeds the limit (4300 digits)"),
        "{long}"
    );
    // api-norm-002 / -003 / -005 (the original list comes back as an alias of the input) / -008
    assert!(pure("n2", "norm", &format!("[{}]", dto_str("v0002.0011")))
        .contains(r#""value":["str","2.11"]"#));
    let alias = pure("n5", "norm", r#"[["list", [["str", "2.11"]]]]"#);
    assert!(
        alias.contains(r#""value":["list",[["str","2.11"]]]"#)
            && alias.contains(r#""input_ref":[["args"],["item",0]]"#),
        "{alias}"
    );
    assert!(pure("n8", "norm", r#"[["int", "211"]]"#).contains(r#""value":["str","211"]"#));
}

#[test]
fn prefix_follows_python_equality_len_and_slicing() {
    // api-prefix-003 / -004 / -007 / -008 / -009
    let two_eleven = r#"["tuple", [["int", "2"], ["int", "11"]]]"#;
    let with_zero = r#"["tuple", [["int", "2"], ["int", "11"], ["int", "0"]]]"#;
    assert!(pure(
        "p3",
        "is_tuple_prefix",
        &format!("[{two_eleven}, {with_zero}]")
    )
    .contains(r#""value":["bool",true]"#));
    assert!(pure(
        "p4",
        "is_tuple_prefix",
        &format!("[{with_zero}, {two_eleven}]")
    )
    .contains(r#""value":["bool",true]"#));
    assert!(pure(
        "p2",
        "is_tuple_prefix",
        &format!("[{two_eleven}, {two_eleven}]")
    )
    .contains(r#""value":["bool",false]"#));
    assert!(pure(
        "p7",
        "is_tuple_prefix",
        r#"[["tuple", [["bool", true]]], ["tuple", [["int", "1"], ["int", "0"]]]]"#
    )
    .contains(r#""value":["bool",true]"#));
    assert!(pure(
        "p8",
        "is_tuple_prefix",
        &format!("[[\"list\", [[\"int\", \"2\"], [\"int\", \"11\"]]], {with_zero}]")
    )
    .contains(r#""value":["bool",false]"#));
    let error = pure(
        "p9",
        "is_tuple_prefix",
        r#"[["int", "1"], ["tuple", [["int", "1"]]]]"#,
    );
    assert!(
        error.contains("object of type 'int' has no len()"),
        "{error}"
    );
}

#[test]
fn months_count_down_and_fail_in_the_originals_order() {
    // api-months-001 / -002 / -003 / -006 / -008 / -009
    let list = pure(
        "m1",
        "gen_candidates",
        &format!("[{}, [\"int\", \"3\"]]", dto_str("26.01")),
    );
    assert!(
        list.contains(
            r#""value":["list",[["str","26.01-py3"],["str","25.12-py3"],["str","25.11-py3"]]]"#
        ),
        "{list}"
    );
    assert!(pure(
        "m2",
        "gen_candidates",
        &format!("[{}, [\"int\", \"2\"]]", dto_str("00.01"))
    )
    .contains(r#"["str","-1.12-py3"]"#));
    assert!(pure(
        "m3",
        "gen_candidates",
        r#"[["bytes", "32362e3035"], ["int", "2"]]"#
    )
    .contains("a bytes-like object is required, not 'str'"));
    assert!(pure(
        "m6",
        "gen_candidates",
        &format!("[{}, [\"int\", \"0\"]]", dto_str("bad"))
    )
    .contains("invalid literal for int() with base 10: 'bad'"));
    assert!(pure(
        "m8",
        "gen_candidates",
        &format!("[{}, [\"float\", \"1.0\"]]", dto_str("26.05"))
    )
    .contains("'float' object cannot be interpreted as an integer"));
    let unicode = pure(
        "m9",
        "gen_candidates",
        &format!(
            "[{}, [\"int\", \"2\"]]",
            dto_str("\u{662}\u{666}.\u{660}\u{665}")
        ),
    );
    assert!(
        unicode.contains(r#"[["str","26.05-py3"],["str","26.04-py3"]]"#),
        "{unicode}"
    );
}

fn inspect(docker: &Scripted, id: &str, tag: &str) -> String {
    answer(
        docker,
        id,
        "inspect_env",
        &format!("[{tag}, {}]", dto_str("arm64")),
    )
}

#[test]
fn inspect_env_keeps_the_originals_dictionary_and_failure_semantics() {
    let tag = dto_str("26.01-py3");
    let json = |stdout: &str| Scripted::new(vec![reply(0, stdout.as_bytes(), b"")]);
    // api-inspect-001: insertion order, the last duplicate wins, first '=' splits, an empty key is kept, an entry without '=' is skipped
    let docker =
        json(r#"{"config":{"Env":["A=first","B=b","A=last","=empty-key","NOVAL","N=a=b","X="]}}"#);
    let record = inspect(&docker, "i1", &tag);
    assert!(record.contains(r#""value":["dict",[[["str","A"],["str","last"]],[["str","B"],["str","b"]],[["str",""],["str","empty-key"]],[["str","N"],["str","a=b"]],[["str","X"],["str",""]]]]"#), "{record}");
    // the command is exactly the original's argv
    let argv: Vec<String> = docker.argvs.borrow()[0]
        .iter()
        .map(|word| String::from_utf8(word.clone()).unwrap())
        .collect();
    assert_eq!(
        argv,
        [
            "docker",
            "buildx",
            "imagetools",
            "inspect",
            "nvcr.io/nvidia/pytorch:26.01-py3",
            "--format",
            "{{json .Image}}"
        ]
    );
    // api-inspect-002 (selected key, lone surrogate kept), -003 (empty selected object falls back)
    let selected = inspect(
        &json(
            r#"{"linux/arm64":{"config":{"Env":["PYTORCH_BUILD_VERSION=2.11.0"]}},"config":{"Env":["IGNORED=yes"]}}"#,
        ),
        "i2",
        &tag,
    );
    assert!(
        selected.contains(r#"[["str","PYTORCH_BUILD_VERSION"],["str","2.11.0"]]"#)
            && !selected.contains("IGNORED"),
        "{selected}"
    );
    assert!(inspect(
        &json(r#"{"linux/arm64":{},"config":{"Env":["F=fallback"]}}"#),
        "i3",
        &tag
    )
    .contains(r#"[["str","F"],["str","fallback"]]"#));
    // api-inspect-004 / -005 / -007: uncaught AttributeError with name and obj
    let null_root = inspect(&json("null"), "i4", &tag);
    assert!(
        null_root.contains("'NoneType' object has no attribute 'get'")
            && null_root.contains(r#""attributes":{"name":["str","get"],"obj":["none"]}"#),
        "{null_root}"
    );
    assert!(inspect(&json("[]"), "i5", &tag).contains("'list' object has no attribute 'get'"));
    assert!(inspect(&json(r#"{"config":[1]}"#), "i7", &tag)
        .contains("'list' object has no attribute 'get'"));
    // api-inspect-006 (null config falls back), -008 (a string Env is iterated by character), -009, -010
    assert!(inspect(&json(r#"{"config":null}"#), "i6", &tag).contains(r#""value":["dict",[]]"#));
    // "A=B" iterated by character: only the lone "=" splits, into an empty key and an empty value
    assert!(inspect(&json(r#"{"config":{"Env":"A=B"}}"#), "i8", &tag)
        .contains(r#""value":["dict",[[["str",""],["str",""]]]]"#));
    assert!(inspect(&json(r#"{"config":{"Env":[null]}}"#), "i9", &tag)
        .contains("argument of type 'NoneType' is not iterable"));
    assert!(
        inspect(&json(r#"{"config":{"Env":[{"=":"x"}]}}"#), "i10", &tag)
            .contains("'dict' object has no attribute 'split'")
    );
    // api-inspect-011 (malformed JSON), -013 (non-zero with valid JSON), -016 / -017 (undecodable stream), run failure: all None
    let none = r#""value":["none"]"#;
    assert!(inspect(&json("{"), "i11", &tag).contains(none));
    assert!(inspect(
        &Scripted::new(vec![reply(7, br#"{"config":{"Env":["A=B"]}}"#, b"warn")]),
        "i13",
        &tag
    )
    .contains(none));
    assert!(inspect(&Scripted::new(vec![reply(0, &[0xff], b"")]), "i16", &tag).contains(none));
    assert!(inspect(
        &Scripted::new(vec![reply(0, br#"{"config":{"Env":[]}}"#, &[0xff])]),
        "i17",
        &tag
    )
    .contains(none));
    assert!(inspect(&Scripted::new(vec![None]), "i14", &tag).contains(none));
    // api-inspect-019 (embedded NUL: refused before any process), -020 (bytes tag is its repr in the reference)
    let never = Scripted::new(Vec::new());
    assert!(
        inspect(&never, "i19", &dto_str("bad\u{0}tag")).contains(none)
            && never.argvs.borrow().is_empty()
    );
    let bytes_tag = Scripted::new(vec![reply(0, br#"{"config":{"Env":[]}}"#, b"")]);
    inspect(&bytes_tag, "i20", r#"["bytes", "32362e30312d707933"]"#);
    assert_eq!(
        bytes_tag.argvs.borrow()[0][4],
        b"nvcr.io/nvidia/pytorch:b'26.01-py3'"
    );
    // universal newlines are applied to stdout before the JSON is read
    assert!(
        inspect(&json("{\r\n\"config\":{\"Env\":[\"K=V\"]}}\r"), "nl", &tag)
            .contains(r#"[["str","K"],["str","V"]]"#)
    );
}

fn private_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = base.join(format!("ngc-checks-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("private directory");
    dir
}

/// A `docker` stub that prints the next literal answer of `answers` (one file per call) and exits with its status.
fn stub(dir: &Path, answers: &[(&str, i32)]) {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for (at, (stdout, _)) in answers.iter().enumerate() {
        std::fs::write(dir.join(format!("out{at}")), stdout).unwrap();
    }
    let codes: Vec<String> = answers.iter().map(|(_, code)| code.to_string()).collect();
    let script = format!(
        "#!/bin/bash\n# bash builtins only: the private PATH holds nothing but this file\nn=0\nif [ -e '{d}/count' ]; then n=$(<'{d}/count'); fi\necho $((n+1)) > '{d}/count'\necho \"$@\" >> '{d}/argv'\ncodes=({c})\nif [ \"$n\" -lt {len} ]; then printf '%s' \"$(<\"{d}/out$n\")\"; exit \"${{codes[$n]}}\"; fi\nexit 9\n",
        d = dir.display(),
        c = codes.join(" "),
        len = answers.len()
    );
    let path = bin.join("docker");
    std::fs::write(&path, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Ran {
    status: i32,
    stdout: String,
    stderr: String,
}

fn command(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Ran {
    let program = dir.join("resolve_ngc_tag.sh");
    if !program.exists() {
        // a hard link, not a copy: a copy holds a writable descriptor that a parallel test's fork can inherit (`ETXTBSY` on exec)
        let built = env!("CARGO_BIN_EXE_resolve_ngc_tag");
        std::fs::hard_link(built, &program)
            .or_else(|_| std::fs::copy(built, &program).map(|_| ()))
            .unwrap();
    }
    let mut child = Command::new(&program);
    child
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", dir.join("bin"))
        .env("COLUMNS", "80")
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::null());
    for (key, value) in envs {
        child.env(key, value);
    }
    let out = child.output().expect("the command ran");
    Ran {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

const BUILD_2_11: &str =
    r#"{"config":{"Env":["PYTORCH_BUILD_VERSION=2.11.0","CUDA_VERSION=13.0"]}}"#;

#[test]
fn the_command_prints_the_first_exact_match_and_collects_prefix_matches() {
    // cli-007 shape: t0 unavailable (stub exit 8), t1 is a prefix match (2.11), t2 an exact match
    let dir = private_dir("match");
    stub(
        &dir,
        &[
            (BUILD_2_11, 8),
            (r#"{"config":{"Env":["PYTORCH_BUILD_VERSION=2.11"]}}"#, 0),
            (BUILD_2_11, 0),
        ],
    );
    let ran = command(
        &dir,
        &["2.11.0", "--arch", "arm64", "--candidates", "t0, t1,,t2"],
        &[],
    );
    assert_eq!(ran.status, 0, "{}", ran.stderr);
    assert!(ran.stderr.is_empty());
    let expected = "{\n  \"torch_pin\": \"2.11.0\",\n  \"torch_prefix\": \"2.11.0\",\n  \"matched_tag\": \"t2\",\n  \"build_version\": \"2.11.0\",\n  \"cuda_version\": \"13.0\",\n  \"image\": \"nvcr.io/nvidia/pytorch:t2\",\n  \"arch\": \"arm64\",\n  \"probed\": [\n    {\n      \"tag\": \"t0\",\n      \"status\": \"unavailable\"\n    },\n    {\n      \"tag\": \"t1\",\n      \"build_version\": \"2.11\",\n      \"prefix\": \"2.11\"\n    },\n    {\n      \"tag\": \"t2\",\n      \"build_version\": \"2.11.0\",\n      \"prefix\": \"2.11.0\"\n    }\n  ],\n  \"prefix_candidates\": [\n    {\n      \"tag\": \"t1\",\n      \"build_version\": \"2.11\",\n      \"note\": \"prefix-match — 최종 중재=스모크\"\n    }\n  ]\n}\n";
    assert_eq!(ran.stdout, expected);
    let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
    assert_eq!(argv.lines().count(), 3);
    assert!(argv
        .lines()
        .next()
        .unwrap()
        .starts_with("buildx imagetools inspect nvcr.io/nvidia/pytorch:t0 --format"));
}

#[test]
fn no_exact_match_ends_with_status_four_and_the_error_report() {
    // cli-008 shape: 2.11 pin, two prefix candidates and one without environment
    let dir = private_dir("nomatch");
    stub(
        &dir,
        &[
            (r#"{"config":{"Env":["PYTORCH_BUILD_VERSION=2.11.0"]}}"#, 0),
            (r#"{"config":{"Env":[]}}"#, 0),
        ],
    );
    let ran = command(
        &dir,
        &["2.11", "--arch", "arm64", "--candidates", "p1,none"],
        &[],
    );
    assert_eq!(ran.status, 4, "{}", ran.stderr);
    assert!(ran.stdout.is_empty());
    assert!(ran
        .stderr
        .starts_with("{\n  \"error\": \"매칭 NGC 태그 없음(완전일치)"));
    assert!(ran.stderr.contains("\"prefix_candidates\": [\n    {\n      \"tag\": \"p1\",\n      \"build_version\": \"2.11.0\",\n      \"note\": \"prefix-match — 최종 중재=스모크\""));
    assert!(ran
        .stderr
        .contains("\"build_version\": null,\n      \"prefix\": null"));
}

#[test]
fn argument_errors_and_business_exits_keep_their_streams_and_statuses() {
    let dir = private_dir("args");
    stub(&dir, &[]);
    let help = command(&dir, &["--help"], &[]);
    assert_eq!(help.status, 0);
    assert!(help.stdout.starts_with("usage: resolve_ngc_tag.sh [-h] [--arch ARCH] [--start START] [--months MONTHS]\n                          [--candidates CANDIDATES]\n                          torch_pin\n\npositional arguments:\n  torch_pin             예: 2.11.0\n"), "{}", help.stdout);
    let missing = command(&dir, &[], &[]);
    assert_eq!((missing.status, missing.stdout.as_str()), (2, ""));
    assert!(
        missing.stderr.ends_with(
            "resolve_ngc_tag.sh: error: the following arguments are required: torch_pin\n"
        ),
        "{}",
        missing.stderr
    );
    let months = command(
        &dir,
        &[
            "2.11.0",
            "--arch",
            "arm64",
            "--candidates",
            "t0",
            "--months",
            "1.5",
        ],
        &[],
    );
    assert_eq!(months.status, 2);
    assert!(
        months
            .stderr
            .ends_with("resolve_ngc_tag.sh: error: argument --months: invalid int value: '1.5'\n"),
        "{}",
        months.stderr
    );
    let arch = command(&dir, &["2.11", "--arch", "weird", "--candidates", "x"], &[]);
    assert_eq!(arch.status, 1);
    assert!(
        arch.stderr
            .starts_with("[ngc] FAIL: --arch 를 정할 수 없다(호스트 '")
            && arch.stderr.ends_with("') — arm64 | amd64 를 명시하라\n"),
        "{}",
        arch.stderr
    );
    let none = command(&dir, &["2.11", "--arch", "arm64"], &[]);
    assert_eq!(
        (none.status, none.stderr.as_str()),
        (2, "{\"error\": \"--candidates 또는 --start 필요\"}\n")
    );
    let empty = command(&dir, &["2.11", "--arch", "arm64", "--candidates", ""], &[]);
    assert_eq!(empty.status, 2);
    // a truthy separator-only --candidates wins over --start (cli-006): empty scan, status 4, nothing probed
    let separators = command(
        &dir,
        &[
            "2.11",
            "--arch",
            "amd64",
            "--candidates",
            "\u{a0}, \t ,",
            "--start",
            "BAD",
        ],
        &[],
    );
    assert_eq!(separators.status, 4);
    assert!(separators.stderr.contains("\"probed\": []"));
    assert!(!dir.join("count").exists(), "no docker call");
}

#[test]
fn uncaught_exceptions_report_the_original_frames() {
    let dir = private_dir("traceback");
    stub(&dir, &[]);
    // cli-014: gen_candidates unpack, the frames and carets of the saved traceback
    let ran = command(
        &dir,
        &[
            "2.11.0", "--arch", "arm64", "--start", "26", "--months", "0",
        ],
        &[],
    );
    assert_eq!(ran.status, 1);
    let script = dir.join("resolve_ngc_tag.sh");
    let expected = format!(
        "Traceback (most recent call last):\n  File \"{p}\", line 139, in <module>\n    main()\n  File \"{p}\", line 96, in main\n    cands = gen_candidates(a.start, a.months)\n            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"{p}\", line 68, in gen_candidates\n    yy, mm = (int(x) for x in start.split(\".\"))\n    ^^^^^^\nValueError: not enough values to unpack (expected 2, got 1)\n",
        p = script.display()
    );
    assert_eq!(ran.stderr, expected);
    // cli-013: a JSON null from the image is an AttributeError at line 56
    stub(&dir, &[("null", 0)]);
    let shape = command(
        &dir,
        &["2.11.0", "--arch", "arm64", "--candidates", "t0"],
        &[],
    );
    assert_eq!(shape.status, 1);
    let tail: Vec<&str> = shape.stderr.lines().rev().take(4).collect();
    assert_eq!(
        tail[0],
        "AttributeError: 'NoneType' object has no attribute 'get'"
    );
    assert_eq!(tail[1], "          ^^^^^^^");
    assert!(
        tail[2].starts_with("    cfg = img.get(f\"linux/{arch}\") or img"),
        "{tail:?}"
    );
}

#[test]
fn a_strict_ascii_standard_output_fails_on_a_unicode_pin_after_the_scan() {
    // cli-020
    let dir = private_dir("ascii");
    stub(&dir, &[(BUILD_2_11, 0)]);
    let ran = command(
        &dir,
        &[
            "\u{662}.\u{661}\u{661}.\u{660}",
            "--arch",
            "arm64",
            "--candidates",
            "t0",
        ],
        &[("PYTHONIOENCODING", "ascii:strict")],
    );
    assert_eq!((ran.status, ran.stdout.as_str()), (1, ""));
    assert!(ran.stderr.ends_with("    print(json.dumps({\nUnicodeEncodeError: 'ascii' codec can't encode character '\\u0662' in position 18: ordinal not in range(128)\n"), "{}", ran.stderr);
    assert!(ran.stderr.contains(", line 114, in main\n"));
}

#[test]
fn the_child_lifecycle_ends_by_kill_at_the_timeout_without_an_orphan() {
    // a generic short timeout at the parameterized boundary (the product value is 90 s and is not shortened or repeated here)
    let dir = private_dir("timeout");
    let pid_file = dir.join("pid");
    let script = dir.join("slow");
    let mut file = std::fs::File::create(&script).unwrap();
    write!(
        file,
        "#!/bin/bash\necho $$ > '{}'\nexec sleep 30\n",
        pid_file.display()
    )
    .unwrap();
    drop(file);
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let docker = ProcessDocker {
        timeout: Duration::from_millis(400),
    };
    let started = Instant::now();
    let outcome = docker.run(&[script.as_os_str().as_encoded_bytes().to_vec()]);
    assert!(outcome.is_none() && started.elapsed() < Duration::from_secs(10));
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists()
            || std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .map_or(true, |stat| stat.contains(") Z")),
        "the killed child is gone"
    );
    // a missing program is a failed start, not a panic
    assert!(docker
        .run(&[b"/nonexistent/ngc-check-docker".to_vec()])
        .is_none());
}

#[test]
fn a_malformed_start_ends_in_the_originals_error_before_the_month_cap_is_named() {
    // Source-derived order (owner lines 68 then 70): `gen_candidates` unpacks `start` before `range(months)` is sized, so only a VALID start can meet
    // the cap. The first case repeats the saved cli-014 malformed start ("26") with a count above the cap; nothing is probed.
    let dir = private_dir("start-before-cap");
    stub(&dir, &[]);
    let script = dir.join("resolve_ngc_tag.sh");
    let uncaught = |start: &str, months: &str, source: &str, message: &str| {
        let ran = command(
            &dir,
            &[
                "2.11.0", "--arch", "arm64", "--start", start, "--months", months,
            ],
            &[],
        );
        let expected = format!(
            "Traceback (most recent call last):\n  File \"{p}\", line 139, in <module>\n    main()\n  File \"{p}\", line 96, in main\n    cands = gen_candidates(a.start, a.months)\n            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"{p}\", line 68, in gen_candidates\n    {source}\nValueError: {message}\n",
            p = script.display()
        )
        .replace("@P@", &script.display().to_string());
        assert_eq!(
            (ran.status, ran.stdout.as_str(), ran.stderr.as_str()),
            (1, "", expected.as_str()),
            "--start {start} --months {months}"
        );
    };
    let unpack = "yy, mm = (int(x) for x in start.split(\".\"))\n    ^^^^^^";
    uncaught(
        "26",
        "20000001",
        unpack,
        "not enough values to unpack (expected 2, got 1)",
    );
    uncaught(
        "26",
        "30000000",
        unpack,
        "not enough values to unpack (expected 2, got 1)",
    );
    // beyond u64, still the start error first
    uncaught(
        "26",
        "99999999999999999999999999",
        unpack,
        "not enough values to unpack (expected 2, got 1)",
    );
    uncaught(
        "bad",
        "20000001",
        "yy, mm = (int(x) for x in start.split(\".\"))\n    ^^^^^^\n  File \"@P@\", line 68, in <genexpr>\n    yy, mm = (int(x) for x in start.split(\".\"))\n              ^^^^^^",
        "invalid literal for int() with base 10: 'bad'",
    );
    assert!(!dir.join("count").exists(), "no docker call");
    // a valid start above the cap is still the named refusal (status 70), at the cap boundary and far above it, and not an uncaught Python error
    for months in ["20000001", "99999999999999999999999999"] {
        let ran = command(
            &dir,
            &[
                "2.11.0", "--arch", "arm64", "--start", "26.11", "--months", months,
            ],
            &[],
        );
        assert_eq!((ran.status, ran.stdout.as_str()), (70, ""), "{months}");
        assert!(
            ran.stderr.contains("months_limit") && !ran.stderr.contains("Traceback"),
            "{}",
            ran.stderr
        );
    }
    assert!(!dir.join("count").exists(), "no docker call");
}

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

/// Kills the helper descendants a test owns (their pids are written by the test's own helper script) when the test ends, pass or fail.
struct OwnedDescendants {
    pid_file: PathBuf,
}

impl OwnedDescendants {
    fn pids(&self) -> Vec<i32> {
        std::fs::read_to_string(&self.pid_file)
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|word| word.parse().ok())
            .collect()
    }

    /// Whether the process is gone (a zombie that nobody reaps counts as gone).
    fn gone(pid: i32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_or(true, |stat| stat.contains(") Z"))
    }
}

impl Drop for OwnedDescendants {
    fn drop(&mut self) {
        for pid in self.pids() {
            // SAFETY: `pid` is a descendant this test's own helper script started and recorded; SIGKILL on it touches nothing else.
            unsafe {
                kill(pid, 9);
            }
        }
    }
}

fn helper_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let script = dir.join(name);
    std::fs::write(&script, format!("#!/bin/bash\n{body}")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn run_helper(docker: &ProcessDocker, script: &Path) -> (Option<Finished>, Duration) {
    let started = Instant::now();
    let outcome = docker.run(&[script.as_os_str().as_encoded_bytes().to_vec()]);
    (outcome, started.elapsed())
}

#[test]
fn a_descendant_holding_a_pipe_after_the_child_exits_ends_in_none_at_the_one_deadline() {
    // subprocess.run's communicate() waits for pipe EOF inside the same timeout: the direct child exits at once, an owned grandchild keeps the
    // inherited stdout and/or stderr open, and the run is `None` at the (short, generic) deadline, never an unbounded wait.
    let timeout = Duration::from_millis(400);
    let docker = ProcessDocker { timeout };
    for (name, redirect) in [
        ("both", ""),
        ("stdout-only", "2>/dev/null"),
        ("stderr-only", ">/dev/null"),
    ] {
        let dir = private_dir(&format!("held-{name}"));
        let owned = OwnedDescendants {
            pid_file: dir.join("pids"),
        };
        let script = helper_script(
            &dir,
            "parent",
            &format!(
                "echo partial\necho partial-err >&2\nsleep 30 {redirect} &\necho $! >> '{}'\nexit 0\n",
                owned.pid_file.display()
            ),
        );
        let (outcome, elapsed) = run_helper(&docker, &script);
        assert!(outcome.is_none(), "{name}: {outcome:?}");
        assert!(
            elapsed >= timeout - Duration::from_millis(50) && elapsed < Duration::from_secs(10),
            "{name}: {elapsed:?}"
        );
        // supervised cleanup of what this test owns, observed to be effective
        let pids = owned.pids();
        assert_eq!(pids.len(), 1, "{name}: the helper recorded its grandchild");
        drop(owned);
        let until = Instant::now() + Duration::from_secs(5);
        while !OwnedDescendants::gone(pids[0]) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            OwnedDescendants::gone(pids[0]),
            "{name}: grandchild cleaned up"
        );
    }
}

#[test]
fn a_descendant_that_released_both_pipes_does_not_delay_the_result() {
    // the control of the case above: same shape, but the grandchild closed both inherited streams, so end of file arrives with the exit and the
    // bytes, the status and the streams come back whole.
    let dir = private_dir("released");
    let owned = OwnedDescendants {
        pid_file: dir.join("pids"),
    };
    let script = helper_script(
        &dir,
        "parent",
        &format!(
            "printf 'out\\r\\n'\nprintf 'err' >&2\nsleep 30 >/dev/null 2>&1 &\necho $! >> '{}'\nexit 3\n",
            owned.pid_file.display()
        ),
    );
    let docker = ProcessDocker {
        timeout: Duration::from_secs(20),
    };
    let (outcome, elapsed) = run_helper(&docker, &script);
    let finished = outcome.expect("the result of the exited child");
    assert_eq!(finished.status, 3);
    assert_eq!(finished.stdout, b"out\r\n");
    assert_eq!(finished.stderr, b"err");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

#[test]
fn output_beyond_the_pipe_buffer_and_non_utf8_bytes_come_back_whole_without_a_reader_thread() {
    // 300000 bytes on each stream (several pipe buffers) are drained while the child is still alive; the bytes are raw (decoding stays the caller's)
    let dir = private_dir("large");
    let script = helper_script(
        &dir,
        "parent",
        "head -c 300000 /dev/zero | tr '\\0' 'a'\nhead -c 200000 /dev/zero | tr '\\0' 'b' >&2\nprintf '\\377\\376'\nexit 0\n",
    );
    let docker = ProcessDocker {
        timeout: Duration::from_secs(20),
    };
    let (outcome, _) = run_helper(&docker, &script);
    let finished = outcome.expect("the child's result");
    assert_eq!(finished.status, 0);
    let mut stdout = vec![b'a'; 300_000];
    stdout.extend_from_slice(&[0xff, 0xfe]);
    assert_eq!(finished.stdout, stdout);
    assert_eq!(finished.stderr, vec![b'b'; 200_000]);
}

#[test]
fn a_child_that_closed_its_pipes_but_keeps_running_is_killed_at_the_same_deadline() {
    // end of file on both pipes is not the end of the child: the wait for the direct child shares the deadline (it is the child itself, by
    // `exec`, that is killed; nothing else is signalled)
    let dir = private_dir("closed-alive");
    let pid_file = dir.join("pid");
    let script = helper_script(
        &dir,
        "parent",
        &format!(
            "echo $$ > '{}'\nexec >&- 2>&-\nexec sleep 30\n",
            pid_file.display()
        ),
    );
    let timeout = Duration::from_millis(400);
    let docker = ProcessDocker { timeout };
    let (outcome, elapsed) = run_helper(&docker, &script);
    assert!(outcome.is_none());
    assert!(
        elapsed >= timeout - Duration::from_millis(50) && elapsed < Duration::from_secs(10),
        "{elapsed:?}"
    );
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        OwnedDescendants::gone(pid),
        "the direct child was killed and reaped"
    );
}
