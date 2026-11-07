//! Pattern matching and temporary directories of the model scripts' file handling, against the language's own standard library
//! (`fnmatch.fnmatchcase`, `glob.glob`; the interpreter is used only as the oracle of those primitives, never of the original
//! scripts) and against directories that already exist.

use super::support::Scratch;
use agent_runtime_support::pystr::PyStr;
use recipe_runtime::model_files::{fnmatch, glob, PrivateDirectory};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Runs a short script of the reference interpreter (isolated mode) with the given text on standard input.
fn python(script: &str, input: &str) -> String {
    let mut child = Command::new("python3")
        .args(["-I", "-W", "ignore", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!("NOT_RUN: the interpreter is the oracle of this check: {error}")
        });
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("send the input");
    let output = child.wait_with_output().expect("the interpreter ends");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("text")
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
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

#[test]
fn fnmatch_agrees_with_pythons_for_sets_ranges_escapes_and_stars() {
    let patterns = [
        "",
        "*",
        "?",
        "**",
        "a*",
        "*a",
        "a*b",
        "a*b*c",
        "?*",
        "*?",
        "a?c",
        "abc",
        "A",
        "[a-c]",
        "[c-a]",
        "[a-]",
        "[-a]",
        "[]-a]",
        "[]]",
        "[]",
        "[!]",
        "[!]]",
        "[!]a]",
        "[",
        "[a",
        "[!",
        "[!a",
        "a[",
        "[a]",
        "[!a]",
        "[^a]",
        "[a^]",
        "[a-c-e]",
        "[a-cx-z]",
        "[a--]",
        "[--a]",
        "[!--]",
        "[a-b-c]",
        "[\\]",
        "[\\a]",
        "[a\\-z]",
        "[a\\]b]",
        "[[]",
        "[[a]",
        "[]a-]",
        "[a-a]",
        "[z-a]",
        "[a-c][x-z]",
        "x[a-c]*",
        "*[!a-c]",
        "[&&]",
        "[~~]",
        "[||]",
        "[a&&b]",
        "[a||b]",
        "[a~~b]",
        "[\\w]",
        "\\*",
        "\\?",
        "\\[",
        "a\\",
        "é*",
        "[é-ü]",
        "[!é]",
        "?é",
        "🙂*",
        "[🙂]",
        "[a-🙂]",
        "*.safetensors",
        "model-*-of-*.safetensors",
        "model-0000?-of-0000?.safetensors",
        "*.[ch]",
        "*.{a,b}",
        "a.b",
        ".*",
        "[.]*",
        "(a)",
        "a+b",
        "a|b",
        "^a$",
        "a{2}",
        "*[*]*",
        "[*]",
        "[?]",
        "[a-z0-9_]*",
        "[!a-z]*",
        "[A-Za-z]*-[0-9]*",
    ];
    let names = [
        "",
        "a",
        "b",
        "c",
        "x",
        "A",
        "ab",
        "abc",
        "abcabc",
        "aXbYc",
        "a.b",
        "a-b",
        "-",
        "--",
        "]",
        "[",
        "[]",
        "^",
        "&",
        "&&",
        "~",
        "|",
        "||",
        "\\",
        "a\\",
        "*",
        "?",
        "é",
        "ü",
        "ñ",
        "🙂",
        "🙂a",
        "z",
        "m",
        "-a",
        "a-",
        ".",
        ".a",
        "x.safetensors",
        "model-00001-of-00002.safetensors",
        "model-1-of-2.safetensors",
        "foo.c",
        "foo.h",
        "foo.{a,b}",
        "(a)",
        "a+b",
        "a|b",
        "^a$",
        "aa",
        "a{2}",
        "a*b",
        "[a-z]",
        "ab[",
        "a_1-9",
    ];
    // The pair table and the oracle's answers, one line of `0`/`1` per pattern.
    let mut request = String::from("{\"patterns\":[");
    request.push_str(
        &patterns
            .iter()
            .map(|pattern| json_string(pattern))
            .collect::<Vec<_>>()
            .join(","),
    );
    request.push_str("],\"names\":[");
    request.push_str(
        &names
            .iter()
            .map(|name| json_string(name))
            .collect::<Vec<_>>()
            .join(","),
    );
    request.push_str("]}");
    let answers = python(
        "import fnmatch,json,sys\nd=json.load(sys.stdin)\nfor p in d['patterns']:\n print(''.join('1' if fnmatch.fnmatchcase(n,p) else '0' for n in d['names']))\n",
        &request,
    );
    let lines: Vec<&str> = answers.lines().collect();
    assert_eq!(lines.len(), patterns.len());
    let mut wrong: Vec<String> = Vec::new();
    for (pattern, line) in patterns.iter().zip(lines) {
        let ours: String = names
            .iter()
            .map(|name| {
                if fnmatch(
                    PyStr::new(name).code_points(),
                    PyStr::new(pattern).code_points(),
                ) {
                    '1'
                } else {
                    '0'
                }
            })
            .collect();
        if ours != line {
            let differing: Vec<&str> = names
                .iter()
                .zip(ours.chars().zip(line.chars()))
                .filter(|(_, (a, b))| a != b)
                .map(|(name, _)| *name)
                .collect();
            wrong.push(format!("pattern {pattern:?}: differs on {differing:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} patterns differ ({} names each):\n{}",
        wrong.len(),
        patterns.len(),
        names.len(),
        wrong.join("\n")
    );
}

#[test]
fn glob_agrees_with_pythons_on_hidden_names_magic_directories_and_sets() {
    let scratch = Scratch::new("glob");
    let root = scratch.root.join("tree");
    let files = [
        "a.safetensors",
        "b.safetensors",
        ".hidden.safetensors",
        "model-00001-of-00002.safetensors",
        "model-00002-of-00002.safetensors",
        "notes.txt",
        "é.safetensors",
        "[x].txt",
        "sub1/c.safetensors",
        "sub1/.d.safetensors",
        "sub2/e.txt",
        "sub2/deep/f.safetensors",
        ".dot/g.safetensors",
        "sub[1]/h.txt",
    ];
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directories");
        std::fs::write(&path, "x").expect("a file");
    }
    std::os::unix::fs::symlink("nowhere", root.join("broken.link")).expect("a dangling link");
    let base = root.to_string_lossy().into_owned();
    let patterns = [
        "*.safetensors",
        ".*.safetensors",
        "*",
        ".*",
        "model-*-of-*.safetensors",
        "sub?/*",
        "sub*/*.safetensors",
        "*/*.safetensors",
        ".*/*",
        "sub[12]/*",
        "sub[!1]/*",
        "[ab].safetensors",
        "[!a].safetensors",
        "*/deep/*",
        "sub2/*/*.safetensors",
        "sub1/c.safetensors",
        "nothing*",
        "*.link",
        "broken.link",
        "[[]x[]].txt",
        "sub[[]1[]]/*",
        "é*",
        "?.safetensors",
        "**",
        "**/*.safetensors",
        "sub*",
    ];
    let mut request = String::from("{\"base\":");
    request.push_str(&json_string(&base));
    request.push_str(",\"patterns\":[");
    request.push_str(
        &patterns
            .iter()
            .map(|pattern| json_string(pattern))
            .collect::<Vec<_>>()
            .join(","),
    );
    request.push_str("]}");
    // Compared as sorted lists: the order is the directory's and is not part of the claim here.
    let answers = python(
        "import glob,json,sys\nd=json.load(sys.stdin)\nfor p in d['patterns']:\n print(json.dumps(sorted(glob.glob(d['base']+'/'+p))))\n",
        &request,
    );
    let lines: Vec<&str> = answers.lines().collect();
    assert_eq!(lines.len(), patterns.len());
    let mut wrong: Vec<String> = Vec::new();
    for (pattern, line) in patterns.iter().zip(lines) {
        let mut ours: Vec<String> = glob(&PyStr::new(&format!("{base}/{pattern}")))
            .iter()
            .map(|path| path.to_utf8().expect("text"))
            .collect();
        ours.sort();
        let rendered = format!(
            "[{}]",
            ours.iter()
                .map(|path| json_string(path))
                .collect::<Vec<_>>()
                .join(", ")
        );
        if rendered != line {
            wrong.push(format!("{pattern:?}:\n  want {line}\n  got  {rendered}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} patterns differ:\n{}",
        wrong.len(),
        patterns.len(),
        wrong.join("\n")
    );
}

#[test]
fn a_private_directory_never_reuses_or_removes_one_that_already_exists() {
    let outer = PrivateDirectory::create("model-collision-").expect("a private directory");
    let parent = outer.path().to_path_buf();
    // A directory, a file and a link already hold the first three names; each keeps its content.
    let taken_directory = parent.join("p-one");
    std::fs::create_dir(&taken_directory).expect("a directory");
    std::fs::write(taken_directory.join("sentinel"), "keep").expect("a sentinel");
    let taken_file = parent.join("p-two");
    std::fs::write(&taken_file, "keep").expect("a file");
    let taken_link = parent.join("p-three");
    std::os::unix::fs::symlink(&taken_directory, &taken_link).expect("a link");
    let own: PathBuf = {
        let directory = PrivateDirectory::create_with(
            &parent,
            "p-",
            ["one", "two", "three", "four"].map(String::from),
        )
        .expect("the first free name is used");
        let path = directory.path().to_path_buf();
        assert_eq!(path, parent.join("p-four"));
        assert_eq!(
            std::fs::metadata(&path)
                .expect("exists")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        std::fs::write(path.join("inner"), "x").expect("write inside");
        path
    };
    // Dropping removed the one it made, with its content, and only that.
    assert!(!own.exists());
    assert_eq!(
        std::fs::read_to_string(taken_directory.join("sentinel")).expect("kept"),
        "keep"
    );
    assert_eq!(std::fs::read_to_string(&taken_file).expect("kept"), "keep");
    assert!(std::fs::symlink_metadata(&taken_link)
        .expect("kept")
        .file_type()
        .is_symlink());
    // Every name taken: an error, nothing touched.
    let error =
        PrivateDirectory::create_with(&parent, "p-", ["one", "two", "three"].map(String::from))
            .err()
            .expect("no free name");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read_to_string(taken_directory.join("sentinel")).expect("kept"),
        "keep"
    );
    assert!(taken_file.exists());
}

#[test]
fn the_argument_parser_agrees_with_pythons_argparse_on_markers_values_and_abbreviations() {
    use recipe_runtime::cliparse::{parse, Kind, Opt, Outcome, Positional, Spec, Value};
    let opt = |flags: &'static [&'static str], dest: &'static str, kind: Kind| Opt {
        flags,
        dest,
        kind,
        help: Some("x"),
        required: false,
        group: None,
    };
    let spec = Spec {
        prog: "parse_model_config.py",
        description: "d",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help),
            opt(&["--self-test"], "self_test", Kind::Flag),
            opt(&["--nas-root"], "nas_root", Kind::Text),
            opt(&["--json"], "json", Kind::Flag),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "path",
            help: Some("p"),
        }),
    };
    let lines: Vec<Vec<&str>> = vec![
        vec![],
        vec!["x"],
        vec!["--"],
        vec!["--", "x"],
        vec!["x", "--"],
        vec!["--", "--"],
        vec!["--", "--", "x"],
        vec!["--json", "--", "x"],
        vec!["--", "--json"],
        vec!["--", "-h"],
        vec!["--", "--help"],
        vec!["x", "--", "y"],
        vec!["x", "y"],
        vec!["--", "x", "y"],
        vec!["--nas-root", "--"],
        vec!["--nas-root", "--", "x"],
        vec!["--nas-root", "a", "--"],
        vec!["--nas-root", "a", "--", "x"],
        vec!["--nas-root=--"],
        vec!["--nas-root", "--json"],
        vec!["--nas-root="],
        vec!["--nas-root=", "x"],
        vec!["--nas-root=a=b"],
        vec!["--nas-root"],
        vec!["--nas-root", "-"],
        vec!["--nas-root", "-x"],
        vec!["--nas-root", "é"],
        vec!["--nas-root", "a b"],
        vec!["--json", "--json"],
        vec!["--js", "x"],
        vec!["--self", "x"],
        vec!["--s", "x"],
        vec!["--n", "a"],
        vec!["--nas", "a", "x"],
        vec!["-x"],
        vec!["--unknown", "a"],
        vec!["-", "x"],
        vec!["-"],
        vec!["---"],
        vec!["---", "x"],
        vec!["--=x"],
        vec!["--json=1"],
        vec!["a", "--nas-root", "b", "--json"],
        vec!["--json", "a", "--nas-root", "b"],
        vec!["--nas-root", "b", "a", "c"],
        vec!["--self-test", "--", "p"],
        vec!["", "x"],
        vec![""],
        vec!["--nas-root", ""],
        vec!["--", ""],
    ];
    let mut request = String::from("[");
    request.push_str(
        &lines
            .iter()
            .map(|line| {
                format!(
                    "[{}]",
                    line.iter()
                        .map(|text| json_string(text))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(","),
    );
    request.push(']');
    let answers = python(
        r#"
import argparse, contextlib, io, json, os, sys
os.environ['COLUMNS'] = '80'
def build():
    p = argparse.ArgumentParser(prog='parse_model_config.py', description='d')
    p.add_argument('--self-test', action='store_true', help='x')
    p.add_argument('--nas-root', help='x')
    p.add_argument('--json', action='store_true', help='x')
    p.add_argument('path', nargs='?', help='p')
    return p
for argv in json.load(sys.stdin):
    err = io.StringIO()
    try:
        with contextlib.redirect_stderr(err):
            a = build().parse_args(argv)
        print('ok ' + json.dumps([a.path, a.self_test, a.nas_root, a.json]))
    except SystemExit as e:
        print('exit %s %s' % (e.code, json.dumps(err.getvalue())))
"#,
        &request,
    );
    let rows: Vec<&str> = answers.lines().collect();
    assert_eq!(rows.len(), lines.len());
    let mut wrong: Vec<String> = Vec::new();
    for (line, want) in lines.iter().zip(rows) {
        let arguments: Vec<PyStr> = line.iter().map(|text| PyStr::new(text)).collect();
        let got = match parse(&spec, &arguments, Some("80")) {
            Outcome::Values(values) => {
                let text = |key: &str| match values.get(key) {
                    Some(Value::Text(own)) => json_string(&own.to_utf8().expect("text")),
                    _ => "null".to_string(),
                };
                let flag = |key: &str| {
                    if matches!(values.get(key), Some(Value::Flag)) {
                        "true"
                    } else {
                        "false"
                    }
                };
                format!(
                    "ok [{}, {}, {}, {}]",
                    text("path"),
                    flag("self_test"),
                    text("nas_root"),
                    flag("json")
                )
            }
            Outcome::Exit { status, stderr, .. } => {
                format!(
                    "exit {status} {}",
                    json_string(&stderr.to_utf8().expect("text"))
                )
            }
        };
        if got != want {
            wrong.push(format!("{line:?}:\n  want {want}\n  got  {got}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} argument lists differ:\n{}",
        wrong.len(),
        lines.len(),
        wrong.join("\n")
    );
}
