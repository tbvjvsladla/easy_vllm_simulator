//! The boundary corrections of the second round, observed against the original's saved results and against the language's own
//! primitives:
//!
//! * `api_cases` / `cli_cases`: the 14 function calls and 6 command lines the original was run on once each for the R24 rounds
//!   (`data/model_extra/`: HTTPS trust and timeouts, dtype keys that are large floats or NaN, `--` and a missing option value,
//!   a private temporary directory that already exists, a ratio that rounds into the subnormal range, a non-UTF-8 option value,
//!   a header length beyond an index). The captured paths and origins are tokens (`@REFERENCE@`, `@EXTRA@`, `@RATIO@`,
//!   `@TLS@`, `@PLAIN@`) replaced by this run's scratch directories and loopback servers; that is the only rebasing.
//! * `tls_variants`: which hashed-directory layouts the reference interpreter trusts (its answers, taken once from Python's own
//!   `ssl`/`urllib` against a local server, are the expected values), against a loopback TLS server with certificates made for each
//!   run by the OpenSSL command line.
//! * `subject_hash`: the canonical-name hash against `openssl x509 -subject_hash` for many synthetic names (string types, case,
//!   spaces, multi-valued RDNs, order).

use super::support::{items, member, text_of, Fixture, Pki, Scratch};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use recipe_runtime::tls_trust;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn extra(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("model_extra")
        .join(relative)
}

fn replace_all(text: &str, replacements: &[(&str, String)]) -> String {
    let mut owned = text.to_string();
    for (from, to) in replacements {
        owned = owned.replace(from, to);
    }
    owned
}

fn rebased(value: &PyValue, replacements: &[(&str, String)]) -> PyValue {
    match value {
        PyValue::Str(text) => match text.to_utf8() {
            Some(own) => PyValue::str(&replace_all(&own, replacements)),
            None => value.clone(),
        },
        PyValue::List(entries) => PyValue::List(
            entries
                .iter()
                .map(|entry| rebased(entry, replacements))
                .collect(),
        ),
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, entry) in map.iter() {
                copy.insert(key.clone(), rebased(entry, replacements));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

fn parse_json(text: &str) -> PyValue {
    agent_runtime_support::pyjson::loads_text(&PyStr::new(text)).expect("saved JSON")
}

/// One request through a fresh `recipe_model_api` process with exactly the given environment; the record it answered.
fn ask(request: &PyValue, environment: &[(&str, &Path)]) -> PyValue {
    let line = request.dumps_unicode().expect("a request line");
    let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_model_api"));
    command
        .arg("evaluate-jsonl")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in environment {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("start the service");
    {
        use std::io::Write;
        let mut input = child.stdin.take().expect("stdin");
        input
            .write_all(line.to_utf8().expect("text").as_bytes())
            .and_then(|()| input.write_all(b"\n"))
            .expect("send the request");
    }
    let output = child.wait_with_output().expect("the service ends");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json(
        std::str::from_utf8(&output.stdout)
            .expect("ASCII record")
            .trim_end(),
    )
}

fn private(label: &str) -> PrivateDirectory {
    PrivateDirectory::create(label).expect("a private directory")
}

/// `<hash>.<suffix>`-style layouts of a hashed directory made for the given certificate files.
struct Layout {
    _directory: PrivateDirectory,
    pub path: PathBuf,
}

impl Layout {
    fn new(files: &[(&str, &str)]) -> Layout {
        let directory = private("model-ca-dir-");
        let path = directory.path().to_path_buf();
        for (name, content) in files {
            std::fs::write(path.join(name), content).expect("write a layout file");
        }
        Layout {
            _directory: directory,
            path,
        }
    }
}

// ── the 14 function calls ────────────────────────────────────────────────────────────────────

mod api_cases {
    use super::*;

    #[test]
    fn the_14_function_calls_the_original_was_run_on_for_this_round_are_reproduced_whole() {
        let pki = Pki::generate();
        let hash = pki.subject_hash("ca.pem");
        let (ca, empty_file) = (pki.file("ca.pem"), pki.file("empty.pem"));
        std::fs::write(&empty_file, "").expect("an empty bundle");
        let hashed = Layout::new(&[(&format!("{hash}.0"), &pki.pem("ca.pem"))]);
        let unhashed = Layout::new(&[(&hash, &pki.pem("ca.pem"))]);
        let wrong = Layout::new(&[("00000000.0", &pki.pem("ca.pem"))]);
        let nothing = Layout::new(&[]);
        let scratch = private("model-extra-");
        let input = scratch.path().to_path_buf();
        for (name, file) in [
            ("large-float-keys", "large-float-keys.safetensors"),
            ("nan-identity-keys", "nan-identity-keys.safetensors"),
        ] {
            std::fs::create_dir_all(input.join(name)).expect("a model directory");
            std::fs::copy(
                extra(&format!("input/{file}")),
                input.join(name).join("model.safetensors"),
            )
            .expect("copy");
        }
        std::fs::create_dir_all(input.join("header-size")).expect("a directory");
        std::fs::copy(
            extra("input/oversize.safetensors"),
            input.join("header-size").join("oversize.safetensors"),
        )
        .expect("copy");

        let ids = parse_json(&std::fs::read_to_string(extra("api.json")).expect("the list"));
        let ratio_row = load_original("subnormal-ratio-verdict");
        let total = match member(member(&ratio_row, "value"), "external_total") {
            PyValue::Int(number) => number.clone(),
            other => panic!("{}", other.repr()),
        };
        let mut body = PyDict::new();
        let mut safetensors = PyDict::new();
        safetensors.set("total", PyValue::Int(total));
        body.set("safetensors", PyValue::Dict(safetensors));
        let routes = vec![(
            "/huge".to_string(),
            PyValue::Dict(body)
                .dumps_unicode()
                .expect("a body")
                .to_utf8()
                .expect("text")
                .into_bytes(),
        )];
        let plain = Fixture::start_with(routes);
        let tls = Fixture::start_tls(&pki, Vec::new());
        let replacements = [
            ("@EXTRA@", input.to_string_lossy().into_owned()),
            ("@TLS@", tls.base.clone()),
            ("@PLAIN@", plain.base.clone()),
        ];
        let mut failures: Vec<String> = Vec::new();
        for id in items(&ids) {
            let id = text_of(id);
            let request = rebased(
                &parse_json(
                    &std::fs::read_to_string(extra(&format!("api/{id}/request.json")))
                        .expect("a request"),
                ),
                &replacements,
            );
            let expected = rebased(&load_original(&id), &replacements);
            let environment: Vec<(&str, &Path)> = match id.as_str() {
                "CA-file" => vec![
                    ("SSL_CERT_FILE", ca.as_path()),
                    ("SSL_CERT_DIR", nothing.path.as_path()),
                ],
                "CA-hashed-directory" => {
                    vec![
                        ("SSL_CERT_FILE", empty_file.as_path()),
                        ("SSL_CERT_DIR", hashed.path.as_path()),
                    ]
                }
                "CA-unhashed-directory" => {
                    vec![
                        ("SSL_CERT_FILE", empty_file.as_path()),
                        ("SSL_CERT_DIR", unhashed.path.as_path()),
                    ]
                }
                "CA-untrusted" => {
                    vec![
                        ("SSL_CERT_FILE", empty_file.as_path()),
                        ("SSL_CERT_DIR", nothing.path.as_path()),
                    ]
                }
                "CA-wrong-hash-directory" => {
                    vec![
                        ("SSL_CERT_FILE", empty_file.as_path()),
                        ("SSL_CERT_DIR", wrong.path.as_path()),
                    ]
                }
                _ => Vec::new(),
            };
            let actual = ask(&request, &environment);
            if !same_value(&expected, &actual) {
                failures.push(format!(
                    "{id}\n  want {}\n  got  {}",
                    expected.repr().chars().take(600).collect::<String>(),
                    actual.repr().chars().take(600).collect::<String>()
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of 14 differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
        assert_eq!(ids_len(&ids), 14);
    }

    fn ids_len(ids: &PyValue) -> usize {
        items(ids).len()
    }

    fn load_original(id: &str) -> PyValue {
        let text = std::fs::read_to_string(extra(&format!("api/{id}/original.stdout")))
            .expect("a saved row");
        parse_json(text.trim_end())
    }

    #[test]
    fn the_observer_would_notice_a_trusting_directory_that_the_original_refuses() {
        // Negative control: the unhashed-directory row must differ from a successful lookup, so a client that trusts it fails the above.
        let refused = load_original("CA-unhashed-directory");
        let accepted = load_original("CA-file");
        assert!(!same_value(&refused, &accepted));
        assert_eq!(
            member(member(&refused, "value"), "ok"),
            &PyValue::Bool(false)
        );
    }
}

// ── the 5 command lines ─────────────────────────────────────────────────────────────────────

mod cli_cases {
    use super::*;

    fn replace_bytes(bytes: &[u8], from: &str, to: &[u8]) -> Vec<u8> {
        let from = from.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at..].starts_with(from) {
                out.extend_from_slice(to);
                at += from.len();
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        out
    }

    #[test]
    fn the_6_command_lines_the_original_was_run_on_for_these_rounds_end_as_it_did() {
        let scratch = Scratch::new("extra-cli");
        let reference = scratch.root.as_os_str().as_bytes().to_vec();
        // A directory whose name holds the byte FF (not UTF-8), with the saved model directory inside.
        let ratio = private("model-ratio-");
        let mut name = b"nas-".to_vec();
        name.push(0xFF);
        let nas = ratio.path().join(OsString::from_vec(name));
        std::fs::create_dir_all(&nas).expect("the directory with a non-UTF-8 name");
        super::super::support::copy_tree(
            &scratch.root.join("input").join("Org-Normal"),
            &nas.join("Org-Normal"),
        );
        let ratio_bytes = ratio.path().as_os_str().as_bytes().to_vec();
        let cases = parse_json(&std::fs::read_to_string(extra("cli.json")).expect("the list"));
        assert_eq!(items(&cases).len(), 6);
        let mut failures: Vec<String> = Vec::new();
        for case in items(&cases) {
            let id = text_of(member(case, "id"));
            let owner = text_of(member(case, "owner"));
            let program = if owner == "parse_model_config" {
                env!("CARGO_BIN_EXE_parse_model_config")
            } else {
                env!("CARGO_BIN_EXE_crosscheck_model_card")
            };
            let arguments: Vec<OsString> = items(member(case, "args"))
                .iter()
                .map(|argument| {
                    let text = text_of(argument);
                    let mut bytes = replace_bytes(text.as_bytes(), "@REFERENCE@", &reference);
                    bytes = replace_bytes(&bytes, "@RATIO@", &ratio_bytes);
                    bytes = replace_bytes(&bytes, "<FF>", &[0xFF]);
                    OsString::from_vec(bytes)
                })
                .collect();
            let mut command = Command::new(program);
            command
                .args(&arguments)
                .current_dir(scratch.root.join("input"))
                .env_clear()
                .stdin(Stdio::null());
            let mut sentinel: Option<PathBuf> = None;
            let output = if id == "selftest-existing-private-directory" {
                // The original ran this with a directory of the name its own process would pick already in place; here the
                // temporary directory is one of ours and holds directories named the way earlier versions named theirs.
                let tmp = private("model-tmp-");
                let child = tmp.path().join("holder");
                std::fs::create_dir_all(&child).expect("holder");
                for suffix in 0..4 {
                    let dir = tmp
                        .path()
                        .join(format!("mxfp4-headers-{}-{suffix}", std::process::id()));
                    std::fs::create_dir_all(&dir).expect("a directory with an old-style name");
                    std::fs::write(dir.join("sentinel"), "keep").expect("a sentinel");
                }
                sentinel = Some(tmp.path().to_path_buf());
                let out = command
                    .env("TMPDIR", tmp.path())
                    .output()
                    .expect("run the program");
                let kept: Vec<_> = (0..4)
                    .filter(|suffix| {
                        tmp.path()
                            .join(format!("mxfp4-headers-{}-{suffix}", std::process::id()))
                            .join("sentinel")
                            .is_file()
                    })
                    .collect();
                assert_eq!(
                    kept.len(),
                    4,
                    "a directory that was already there was removed"
                );
                let leftover: Vec<String> = std::fs::read_dir(tmp.path())
                    .expect("list")
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| {
                        name.starts_with("mxfp4-headers-")
                            && !name.contains(&format!("-{}-", std::process::id()))
                    })
                    .collect();
                assert!(
                    leftover.is_empty(),
                    "the self-test left its own directory behind: {leftover:?}"
                );
                out
            } else {
                command.output().expect("run the program")
            };
            drop(sentinel);
            let saved = |name: &str| {
                std::fs::read(extra(&format!("cli/{id}/{name}"))).expect("saved bytes")
            };
            let want = |name: &str| {
                let mut bytes = replace_bytes(&saved(name), "@REFERENCE@", &reference);
                bytes = replace_bytes(&bytes, "@RATIO@", &ratio_bytes);
                bytes
            };
            let wanted_status = match member(case, "exit_code") {
                PyValue::Int(number) => number.to_string(),
                other => panic!("{}", other.repr()),
            };
            let mut problems: Vec<String> = Vec::new();
            if output.status.code().map(|code| code.to_string()) != Some(wanted_status.clone()) {
                problems.push(format!(
                    "status {:?} want {wanted_status}",
                    output.status.code()
                ));
            }
            if output.stdout != want("original.stdout") {
                problems.push(format!(
                    "stdout differs:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want("original.stdout")),
                    String::from_utf8_lossy(&output.stdout)
                ));
            }
            if output.stderr != want("original.stderr") {
                problems.push(format!(
                    "stderr differs:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want("original.stderr")),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            if !problems.is_empty() {
                failures.push(format!("{id} {arguments:?}: {}", problems.join("; ")));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of 6 differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ── which hashed-directory layouts are trusted ──────────────────────────────────────────────────

mod tls_variants {
    use super::*;

    /// The answer of the reference interpreter (Python's `ssl` and `urllib` against a local TLS server, taken once) for layouts
    /// whose outcome does not depend on the order the file system lists a directory.
    const DIRECTORY_VARIANTS: &[(&str, bool)] = &[
        ("<H>.0", true),
        ("<H>.r0", true),
        ("<H>.00", true),
        ("<H>.1 alone", true),
        ("<H>.12345", true),
        ("<H> upper .0", true),
        ("<H> mixed .7", true),
        ("<H>.x", false),
        ("<H>.0x", false),
        ("<H>.", false),
        ("<H>.0.pem", false),
        ("<H>.pem", false),
        ("<H>_0", false),
        ("<H> short .0", false),
        ("<H>0.0", false),
        ("0<H>.0", false),
        (" <H>.0", false),
        ("<H>.0 ", false),
        ("<H>.arabic", false),
        ("<H>.1.0", false),
        ("<H>.r", false),
        ("<H>.R0", false),
        ("<H>", false),
        ("00000000.0", false),
        ("ffffffff.0", false),
        ("ca.pem", false),
        ("other then ca", true),
        ("ca and other", true),
        ("gap then 2", true),
        ("symlink", true),
        ("two certs in one file", true),
        ("server as anchor", false),
        ("empty", false),
    ];

    fn directory_for(variant: &str, pki: &Pki, hash: &str) -> Layout {
        let (ca, other, server) = (
            pki.pem("ca.pem"),
            pki.pem("other.pem"),
            pki.pem("server.pem"),
        );
        let h = hash.to_string();
        let upper = hash.to_uppercase();
        let mixed = format!("{}{}", hash[..4].to_uppercase(), &hash[4..]);
        match variant {
            "<H>.0" => Layout::new(&[(&format!("{h}.0"), &ca)]),
            "<H>.r0" => Layout::new(&[(&format!("{h}.r0"), &ca)]),
            "<H>.00" => Layout::new(&[(&format!("{h}.00"), &ca)]),
            "<H>.1 alone" => Layout::new(&[(&format!("{h}.1"), &ca)]),
            "<H>.12345" => Layout::new(&[(&format!("{h}.12345"), &ca)]),
            "<H> upper .0" => Layout::new(&[(&format!("{upper}.0"), &ca)]),
            "<H> mixed .7" => Layout::new(&[(&format!("{mixed}.7"), &ca)]),
            "<H>.x" => Layout::new(&[(&format!("{h}.x"), &ca)]),
            "<H>.0x" => Layout::new(&[(&format!("{h}.0x"), &ca)]),
            "<H>." => Layout::new(&[(&format!("{h}."), &ca)]),
            "<H>.0.pem" => Layout::new(&[(&format!("{h}.0.pem"), &ca)]),
            "<H>.pem" => Layout::new(&[(&format!("{h}.pem"), &ca)]),
            "<H>_0" => Layout::new(&[(&format!("{h}_0"), &ca)]),
            "<H> short .0" => Layout::new(&[(&format!("{}.0", &h[..7]), &ca)]),
            "<H>0.0" => Layout::new(&[(&format!("{h}0.0"), &ca)]),
            "0<H>.0" => Layout::new(&[(&format!("0{h}.0"), &ca)]),
            " <H>.0" => Layout::new(&[(&format!(" {h}.0"), &ca)]),
            "<H>.0 " => Layout::new(&[(&format!("{h}.0 "), &ca)]),
            "<H>.arabic" => Layout::new(&[(&format!("{h}.\u{663}"), &ca)]),
            "<H>.1.0" => Layout::new(&[(&format!("{h}.1.0"), &ca)]),
            "<H>.r" => Layout::new(&[(&format!("{h}.r"), &ca)]),
            "<H>.R0" => Layout::new(&[(&format!("{h}.R0"), &ca)]),
            "<H>" => Layout::new(&[(&h, &ca)]),
            "00000000.0" => Layout::new(&[("00000000.0", &ca)]),
            "ffffffff.0" => Layout::new(&[("ffffffff.0", &ca)]),
            "ca.pem" => Layout::new(&[("ca.pem", &ca)]),
            "other then ca" => {
                Layout::new(&[(&format!("{h}.0"), &other), (&format!("{h}.1"), &ca)])
            }
            "ca and other" => Layout::new(&[(&format!("{h}.0"), &ca), (&format!("{h}.2"), &other)]),
            "gap then 2" => Layout::new(&[(&format!("{h}.0"), &other), (&format!("{h}.2"), &ca)]),
            "symlink" => {
                let layout = Layout::new(&[]);
                std::os::unix::fs::symlink(pki.file("ca.pem"), layout.path.join(format!("{h}.0")))
                    .expect("a link");
                layout
            }
            "two certs in one file" => Layout::new(&[(&format!("{h}.0"), &format!("{other}{ca}"))]),
            "server as anchor" => Layout::new(&[(&format!("{h}.0"), &server)]),
            "empty" => Layout::new(&[]),
            other => panic!("{other}"),
        }
    }

    pub(super) fn reason_of(row: &PyValue) -> Option<String> {
        let value = member(row, "value");
        match member(value, "ok") {
            PyValue::Bool(true) => None,
            PyValue::Bool(false) => Some(text_of(member(value, "reason"))),
            other => panic!("{}", other.repr()),
        }
    }

    #[test]
    fn hashed_directory_layouts_are_trusted_as_the_reference_interpreter_trusts_them() {
        let pki = Pki::generate();
        let hash = pki.subject_hash("ca.pem");
        let empty_file = pki.file("empty.pem");
        std::fs::write(&empty_file, "").expect("an empty bundle");
        let tls = Fixture::start_tls(&pki, Vec::new());
        let request = |label: &str| {
            let mut dict = PyDict::new();
            dict.set("id", PyValue::str(label));
            dict.set(
                "operation",
                PyValue::str("crosscheck_model_card.fetch_hf_safetensors_total"),
            );
            dict.set(
                "args",
                PyValue::List(vec![PyValue::str("ok"), PyValue::Float(5.0)]),
            );
            dict.set("kwargs", PyValue::Dict(PyDict::new()));
            let mut context = PyDict::new();
            context.set("hf_api_base", PyValue::str(&tls.base));
            dict.set("context", PyValue::Dict(context));
            PyValue::Dict(dict)
        };
        let refused = format!(
            "네트워크 오류: <urlopen error [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: unable to get local issuer certificate (_ssl.c:1000)>({}/ok)",
            tls.base
        );
        let mut wrong: Vec<String> = Vec::new();
        for (variant, trusted) in DIRECTORY_VARIANTS {
            let layout = directory_for(variant, &pki, &hash);
            let row = ask(
                &request(variant),
                &[
                    ("SSL_CERT_FILE", empty_file.as_path()),
                    ("SSL_CERT_DIR", layout.path.as_path()),
                ],
            );
            let got = reason_of(&row);
            let ok = match (&got, trusted) {
                (None, true) => true,
                (Some(reason), false) => *reason == refused,
                _ => false,
            };
            if !ok {
                wrong.push(format!("{variant}: trusted={trusted} got {got:?}"));
            }
        }
        // The two sources are separate: a file with an empty directory, a directory list with empty entries, an emptied variable.
        let hashed = directory_for("<H>.0", &pki, &hash);
        let empty_dir = directory_for("empty", &pki, &hash);
        let listed = OsString::from(format!(
            "{}:{}",
            empty_dir.path.display(),
            hashed.path.display()
        ));
        let colons = OsString::from(format!(":{}::", hashed.path.display()));
        let ca = pki.file("ca.pem");
        let other = pki.file("other.pem");
        let nothing = Path::new("");
        for (label, environment, trusted) in [
            (
                "list: empty then hashed",
                vec![
                    ("SSL_CERT_FILE", empty_file.as_path()),
                    ("SSL_CERT_DIR", Path::new(&listed)),
                ],
                true,
            ),
            (
                "list: with empty entries",
                vec![
                    ("SSL_CERT_FILE", empty_file.as_path()),
                    ("SSL_CERT_DIR", Path::new(&colons)),
                ],
                true,
            ),
            (
                "file: bundle, empty directory",
                vec![
                    ("SSL_CERT_FILE", ca.as_path()),
                    ("SSL_CERT_DIR", empty_dir.path.as_path()),
                ],
                true,
            ),
            (
                "file: other bundle, hashed directory",
                vec![
                    ("SSL_CERT_FILE", other.as_path()),
                    ("SSL_CERT_DIR", hashed.path.as_path()),
                ],
                true,
            ),
            (
                "file: emptied variable, empty directory",
                vec![
                    ("SSL_CERT_FILE", nothing),
                    ("SSL_CERT_DIR", empty_dir.path.as_path()),
                ],
                false,
            ),
            (
                "dir: emptied variable, bundle",
                vec![("SSL_CERT_FILE", ca.as_path()), ("SSL_CERT_DIR", nothing)],
                true,
            ),
            (
                "nothing",
                vec![
                    ("SSL_CERT_FILE", empty_file.as_path()),
                    ("SSL_CERT_DIR", empty_dir.path.as_path()),
                ],
                false,
            ),
        ] {
            let got = reason_of(&ask(&request(label), &environment));
            let ok = match (&got, trusted) {
                (None, true) => true,
                (Some(reason), false) => *reason == refused,
                _ => false,
            };
            if !ok {
                wrong.push(format!("{label}: trusted={trusted} got {got:?}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} differ:\n{}",
            wrong.len(),
            DIRECTORY_VARIANTS.len() + 7,
            wrong.join("\n")
        );
    }

    pub(super) fn fetch_request(base: &str) -> PyValue {
        let mut dict = PyDict::new();
        dict.set("id", PyValue::str("fault"));
        dict.set(
            "operation",
            PyValue::str("crosscheck_model_card.fetch_hf_safetensors_total"),
        );
        dict.set(
            "args",
            PyValue::List(vec![PyValue::str("ok"), PyValue::Float(5.0)]),
        );
        dict.set("kwargs", PyValue::Dict(PyDict::new()));
        let mut context = PyDict::new();
        context.set("hf_api_base", PyValue::str(base));
        dict.set("context", PyValue::Dict(context));
        PyValue::Dict(dict)
    }

    /// The failure texts of the reference interpreter for a server certificate that is expired, not yet valid or names another
    /// host (taken once from Python's `urllib`/`ssl` against a local server: `oracle/tls_oracle4.py`), for each address form.
    #[test]
    fn a_certificate_fault_is_worded_as_the_reference_interpreter_words_it() {
        let pki = Pki::generate();
        pki.issue(
            "expired",
            "20200101000000Z",
            "20200102000000Z",
            "DNS:localhost,IP:127.0.0.1",
        );
        pki.issue(
            "future",
            "20400101000000Z",
            "20400102000000Z",
            "DNS:localhost,IP:127.0.0.1",
        );
        pki.issue(
            "badname",
            "20200101000000Z",
            "20400102000000Z",
            "DNS:other.example,IP:10.1.2.3",
        );
        pki.issue(
            "dnsonly",
            "20200101000000Z",
            "20400102000000Z",
            "DNS:localhost",
        );
        let nothing = Layout::new(&[]);
        let ca = pki.file("ca.pem");
        let environment = [
            ("SSL_CERT_FILE", ca.as_path()),
            ("SSL_CERT_DIR", nothing.path.as_path()),
        ];
        let words = |detail: &str, base: &str| {
            format!("네트워크 오류: <urlopen error [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {detail} (_ssl.c:1000)>({base}/ok)")
        };
        let mut wrong: Vec<String> = Vec::new();
        for (name, host, expected) in [
            ("expired", "127.0.0.1", Some("certificate has expired")),
            ("expired", "localhost", Some("certificate has expired")),
            ("future", "127.0.0.1", Some("certificate is not yet valid")),
            ("future", "localhost", Some("certificate is not yet valid")),
            (
                "badname",
                "127.0.0.1",
                Some("IP address mismatch, certificate is not valid for '127.0.0.1'."),
            ),
            (
                "badname",
                "localhost",
                Some("Hostname mismatch, certificate is not valid for 'localhost'."),
            ),
            (
                "dnsonly",
                "127.0.0.1",
                Some("IP address mismatch, certificate is not valid for '127.0.0.1'."),
            ),
            ("dnsonly", "localhost", None),
        ] {
            let server = Fixture::start_tls_as(&pki, name, Vec::new());
            let base = server.base.replace("127.0.0.1", host);
            let row = ask(&fetch_request(&base), &environment);
            let got = reason_of(&row);
            let want = expected.map(|detail| words(detail, &base));
            if got != want {
                wrong.push(format!("{name} {host}: want {want:?} got {got:?}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of 8 differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    #[test]
    fn a_chain_is_trusted_when_the_bundle_file_names_the_authority_and_the_directory_does_not() {
        let pki = Pki::generate();
        let nothing = Layout::new(&[]);
        let tls = Fixture::start_tls(&pki, Vec::new());
        let ca = pki.file("ca.pem");
        let row = ask(
            &fetch_request(&tls.base),
            &[
                ("SSL_CERT_FILE", ca.as_path()),
                ("SSL_CERT_DIR", nothing.path.as_path()),
            ],
        );
        assert_eq!(member(member(&row, "value"), "ok"), &PyValue::Bool(true));
    }
}

// ── chains: which certificates may end a chain, which may only help build it ───────────────────────

mod tls_chains {
    use super::super::support::rebased;
    use super::tls_variants::{fetch_request, reason_of};
    use super::*;

    fn saved_negatives() -> PyValue {
        parse_json(
            &std::fs::read_to_string(extra("tls_intermediate.json")).expect("the saved answers"),
        )
    }

    /// The whole record the service answers for a lookup of `check` with timeout 3.0.
    fn record(value: &str) -> PyValue {
        parse_json(&format!(
            "{{\"id\": \"fault\", \"kind\": \"value\", \"value\": {value}, \"stdout\": \"\", \"stderr\": \"\", \"args_after\": [\"check\", 3.0], \"kwargs_after\": {{}}}}"
        ))
    }

    fn lookup(base: &str, environment: &[(&str, &Path)]) -> PyValue {
        let mut request = fetch_request(base);
        if let PyValue::Dict(dict) = &mut request {
            dict.set(
                "args",
                PyValue::List(vec![PyValue::str("check"), PyValue::Float(3.0)]),
            );
        }
        ask(&request, environment)
    }

    fn answers(total: i64) -> Vec<super::super::support::Route> {
        vec![(
            "/check".to_string(),
            format!("{{\"safetensors\": {{\"total\": {total}}}}}").into_bytes(),
        )]
    }

    /// The two saved answers of the reference interpreter (Python's `ssl`/`urllib`, default context: verify flags 32768, no partial
    /// chain) for a root -> intermediate -> leaf chain whose server sends leaf + intermediate, when the only certificate configured is
    /// the intermediate (as the CA file, or in a hashed directory): the connection is refused with OpenSSL error 2. The held first
    /// candidate trusted the intermediate as an anchor and answered `ok`.
    #[test]
    fn an_intermediate_alone_cannot_end_a_chain_as_the_reference_interpreter_refuses() {
        let pki = Pki::generate();
        pki.chain();
        let saved = saved_negatives();
        let nothing = Layout::new(&[]);
        let hash = pki.subject_hash("chain-intermediate.pem");
        let hashed = Layout::new(&[(&format!("{hash}.0"), &pki.pem("chain-intermediate.pem"))]);
        let server = Fixture::start_tls_chain(
            &pki,
            &["chain-leaf", "chain-intermediate"],
            "chain-leaf",
            answers(7),
        );
        let intermediate = pki.file("chain-intermediate.pem");
        let blank = Path::new("");
        let mut wrong: Vec<String> = Vec::new();
        for row in items(member(&saved, "rows")) {
            let id = text_of(member(row, "id"));
            let primitive = member(row, "primitive");
            assert_eq!(member(primitive, "ok"), &PyValue::Bool(false));
            let message = text_of(member(primitive, "message"));
            let environment: Vec<(&str, &Path)> = match id.as_str() {
                "intermediate-file" => vec![
                    ("SSL_CERT_FILE", intermediate.as_path()),
                    ("SSL_CERT_DIR", nothing.path.as_path()),
                ],
                "intermediate-hashed-dir" => {
                    vec![
                        ("SSL_CERT_FILE", blank),
                        ("SSL_CERT_DIR", hashed.path.as_path()),
                    ]
                }
                other => panic!("{other}"),
            };
            let got = lookup(&server.base, &environment);
            let want_reason = format!("네트워크 오류: {message}({}/check)", server.base);
            let want = record(&format!(
                "{{\"ok\": false, \"reason\": {}}}",
                PyValue::str(&want_reason)
                    .dumps_unicode()
                    .expect("text")
                    .to_utf8()
                    .expect("text")
            ));
            if !same_value(&want, &got) {
                wrong.push(format!(
                    "{id}\n  want {}\n  got  {}",
                    want.repr(),
                    got.repr()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of 2 differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// Controls so that the refusal above is not a blanket refusal of intermediates: the verdicts of Python's `ssl`/`urllib` for
    /// four new inputs (`oracle/tls_chain_oracle.py`, one call each, all accepted): a server that sends the whole chain with the
    /// root as the only store certificate, a server that sends the leaf alone while the store holds the root and the
    /// intermediate (file or hashed directory), and a server that sends leaf + intermediate with the root in the store.
    #[test]
    fn chains_are_built_from_the_server_and_from_the_store_as_the_reference_interpreter_builds_them(
    ) {
        let pki = Pki::generate();
        pki.chain();
        let nothing = Layout::new(&[]);
        let blank_file = pki.file("blank.pem");
        std::fs::write(&blank_file, "").expect("an empty bundle");
        let both = pki.file("chain-both.pem");
        std::fs::write(
            &both,
            format!(
                "{}{}",
                pki.pem("chain-root.pem"),
                pki.pem("chain-intermediate.pem")
            ),
        )
        .expect("a bundle");
        let root = pki.file("chain-root.pem");
        let hashed = Layout::new(&[
            (
                &format!("{}.0", pki.subject_hash("chain-root.pem")),
                &pki.pem("chain-root.pem"),
            ),
            (
                &format!("{}.0", pki.subject_hash("chain-intermediate.pem")),
                &pki.pem("chain-intermediate.pem"),
            ),
        ]);
        let whole = record("{\"ok\": true, \"total\": 7, \"parameters\": {}}");
        let mut wrong: Vec<String> = Vec::new();
        for (label, sent, environment) in [
            (
                "P1 server leaf+intermediate+root; store file root",
                vec!["chain-leaf", "chain-intermediate", "chain-root"],
                vec![
                    ("SSL_CERT_FILE", root.as_path()),
                    ("SSL_CERT_DIR", nothing.path.as_path()),
                ],
            ),
            (
                "P2 server leaf; store file root+intermediate",
                vec!["chain-leaf"],
                vec![
                    ("SSL_CERT_FILE", both.as_path()),
                    ("SSL_CERT_DIR", nothing.path.as_path()),
                ],
            ),
            (
                "P3 server leaf+intermediate; store file root",
                vec!["chain-leaf", "chain-intermediate"],
                vec![
                    ("SSL_CERT_FILE", root.as_path()),
                    ("SSL_CERT_DIR", nothing.path.as_path()),
                ],
            ),
            (
                "P4 server leaf; store hashed directory root+intermediate",
                vec!["chain-leaf"],
                vec![
                    ("SSL_CERT_FILE", blank_file.as_path()),
                    ("SSL_CERT_DIR", hashed.path.as_path()),
                ],
            ),
        ] {
            let server = Fixture::start_tls_chain(&pki, &sent, "chain-leaf", answers(7));
            let got = lookup(&server.base, &environment);
            if !same_value(&whole, &got) {
                wrong.push(format!(
                    "{label}\n  want {}\n  got  {}",
                    whole.repr(),
                    got.repr()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of 4 differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// The saved whole answer of the original (one new call, `r24-redirect-reference`: `http` -> `302` -> `https` on loopback, valid
    /// root -> intermediate -> leaf chain, CA file = the root, the server sends leaf + intermediate; the held v3 refused it because the
    /// trust store was chosen from the first request's scheme): the same record, from an actual `https` GET after the redirect, and the
    /// requests the two servers saw are the saved ones. Only the origin is rebased.
    #[test]
    fn a_redirect_from_http_to_https_is_verified_against_the_process_selected_store() {
        let pki = Pki::generate();
        pki.chain();
        let nothing = Layout::new(&[]);
        let root = pki.file("chain-root.pem");
        let tls = Fixture::start_tls_chain(
            &pki,
            &["chain-leaf", "chain-intermediate"],
            "chain-leaf",
            answers(7),
        );
        let plain = Fixture::start_redirecting(&tls.base);
        let saved_request = rebased(
            &parse_json(
                &std::fs::read_to_string(extra("redirect/request.json"))
                    .expect("the saved request"),
            ),
            &[("@PLAIN@", plain.base.clone())],
        );
        let saved = parse_json(
            std::fs::read_to_string(extra("redirect/original.stdout"))
                .expect("the saved answer")
                .trim_end(),
        );
        let got = ask(
            &saved_request,
            &[
                ("SSL_CERT_FILE", root.as_path()),
                ("SSL_CERT_DIR", nothing.path.as_path()),
            ],
        );
        assert!(
            same_value(&saved, &got),
            "want {}\n got  {}",
            saved.repr(),
            got.repr()
        );
        // The requests: the saved original's (an `http` GET of `/check`, then an `https` GET of `/check`, both asking for JSON).
        let expected = parse_json(
            &std::fs::read_to_string(extra("redirect/http-requests.json")).expect("saved requests"),
        );
        let originals: Vec<(String, String, String)> = items(&expected)
            .iter()
            .filter(|row| text_of(member(row, "actor")) == "original")
            .map(|row| {
                (
                    text_of(member(row, "scheme")),
                    text_of(member(row, "path")),
                    text_of(member(row, "accept")),
                )
            })
            .collect();
        let json = "application/json".to_string();
        assert_eq!(
            originals,
            vec![
                ("http".to_string(), "/check".to_string(), json.clone()),
                ("https".to_string(), "/check".to_string(), json.clone())
            ]
        );
        assert_eq!(
            plain.received(),
            vec![("/check".to_string(), Some(json.clone()))]
        );
        let secure: Vec<_> = tls
            .received()
            .into_iter()
            .filter(|(path, _)| path == "/check")
            .collect();
        assert_eq!(secure, vec![("/check".to_string(), Some(json))]);
    }

    /// The control: the same redirect with a store that holds no authority is refused with the known refusal of an unknown issuer,
    /// and no request reaches the TLS server (nothing is accepted without verification, whatever the first request's scheme).
    #[test]
    fn a_redirect_to_https_without_a_trusted_authority_is_refused() {
        let pki = Pki::generate();
        pki.chain();
        let nothing = Layout::new(&[]);
        let blank = pki.file("blank.pem");
        std::fs::write(&blank, "").expect("an empty bundle");
        let tls = Fixture::start_tls_chain(
            &pki,
            &["chain-leaf", "chain-intermediate"],
            "chain-leaf",
            answers(7),
        );
        let plain = Fixture::start_redirecting(&tls.base);
        let got = lookup(
            &plain.base,
            &[
                ("SSL_CERT_FILE", blank.as_path()),
                ("SSL_CERT_DIR", nothing.path.as_path()),
            ],
        );
        let want = format!(
            "네트워크 오류: <urlopen error [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: unable to get local issuer certificate (_ssl.c:1000)>({}/check)",
            plain.base
        );
        assert_eq!(reason_of(&got), Some(want));
        assert!(
            tls.received()
                .iter()
                .all(|(path, accept)| path != "/check" && accept.is_none()),
            "no request may reach the TLS server over a refused connection"
        );
    }

    /// Derived from OpenSSL's source (`internal_verify` checks the dates of the trust anchor too; WebPKI does not), not observed: a chain
    /// that is sound except that its anchor's own validity ended is refused with the date's words, and no request reaches the server.
    #[test]
    fn an_anchor_whose_own_dates_are_not_valid_refuses_the_connection() {
        let pki = Pki::generate();
        pki.expired_anchor();
        let nothing = Layout::new(&[]);
        let old_root = pki.file("old-root.pem");
        let server = Fixture::start_tls_chain(&pki, &["old-leaf"], "old-leaf", answers(7));
        let got = lookup(
            &server.base,
            &[
                ("SSL_CERT_FILE", old_root.as_path()),
                ("SSL_CERT_DIR", nothing.path.as_path()),
            ],
        );
        let want = format!(
            "네트워크 오류: <urlopen error [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: certificate has expired (_ssl.c:1000)>({}/check)",
            server.base
        );
        assert_eq!(reason_of(&got), Some(want));
        // (An aborted connection is logged by the fixture as a request for `/` with no `Accept`; none for the lookup's path.)
        assert!(
            server
                .received()
                .iter()
                .all(|(path, accept)| path != "/check" && accept.is_none()),
            "no request may reach the server over a refused connection"
        );
    }
}

// ── the canonical name hash ─────────────────────────────────────────────────────────────────────

mod subject_hash {
    use super::*;

    fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        if content.len() < 0x80 {
            out.push(content.len() as u8);
        } else {
            let bytes = content.len().to_be_bytes();
            let skip = bytes.iter().take_while(|&&byte| byte == 0).count();
            out.push(0x80 | (bytes.len() - skip) as u8);
            out.extend_from_slice(&bytes[skip..]);
        }
        out.extend_from_slice(content);
        out
    }

    const COMMON_NAME: [u8; 5] = [0x06, 0x03, 0x55, 0x04, 0x03];
    const COUNTRY: [u8; 5] = [0x06, 0x03, 0x55, 0x04, 0x06];
    const ORGANIZATION: [u8; 5] = [0x06, 0x03, 0x55, 0x04, 0x0A];
    const UNIT: [u8; 5] = [0x06, 0x03, 0x55, 0x04, 0x0B];
    /// An attribute type the OpenSSL string table does not know, so that it accepts any string type for it.
    const UNKNOWN: [u8; 5] = [0x06, 0x03, 0x2A, 0x03, 0x04];
    const EMAIL: [u8; 11] = [
        0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x01,
    ];

    /// One attribute: `(oid, tag, value bytes)`.
    type Attribute<'a> = (&'a [u8], u8, Vec<u8>);

    /// A `Name` from RDNs (each a list of attributes, written in the order given, which for a multi-valued RDN is not the DER order).
    fn name(rdns: &[Vec<Attribute<'_>>]) -> Vec<u8> {
        let mut body = Vec::new();
        for rdn in rdns {
            let mut set = Vec::new();
            for (oid, tag, value) in rdn {
                let mut attribute = oid.to_vec();
                attribute.extend(tlv(*tag, value));
                set.extend(tlv(0x30, &attribute));
            }
            body.extend(tlv(0x31, &set));
        }
        tlv(0x30, &body)
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    fn utf32(text: &str) -> Vec<u8> {
        text.chars()
            .flat_map(|c| u32::from(c).to_be_bytes())
            .collect()
    }

    /// A DER certificate with the given subject (the signature is not a real one: only the name is read).
    fn certificate(subject: &[u8], key_info: &[u8]) -> Vec<u8> {
        let algorithm = tlv(
            0x30,
            &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02],
        );
        let validity = tlv(
            0x30,
            &[tlv(0x17, b"240101000000Z"), tlv(0x17, b"340101000000Z")].concat(),
        );
        let mut tbs = tlv(0xA0, &tlv(0x02, &[2]));
        tbs.extend(tlv(0x02, &[1]));
        tbs.extend(&algorithm);
        tbs.extend(subject);
        tbs.extend(validity);
        tbs.extend(subject);
        tbs.extend(key_info);
        let mut whole = tlv(0x30, &tbs);
        whole.extend(&algorithm);
        whole.extend(tlv(0x03, &[vec![0u8], vec![7u8; 8]].concat()));
        tlv(0x30, &whole)
    }

    /// A DER certificate with its own issuer, subject, signature algorithm (the whole `AlgorithmIdentifier` element), serial and
    /// extensions (the `Extension` elements; none when empty). The signature is not a real one.
    fn certificate_with(
        issuer: &[u8],
        subject: &[u8],
        algorithm: &[u8],
        serial: u8,
        extensions: &[Vec<u8>],
        key_info: &[u8],
    ) -> Vec<u8> {
        let validity = tlv(
            0x30,
            &[tlv(0x17, b"240101000000Z"), tlv(0x17, b"340101000000Z")].concat(),
        );
        let mut tbs = tlv(0xA0, &tlv(0x02, &[2]));
        tbs.extend(tlv(0x02, &[serial]));
        tbs.extend(algorithm);
        tbs.extend(issuer);
        tbs.extend(validity);
        tbs.extend(subject);
        tbs.extend(key_info);
        if !extensions.is_empty() {
            tbs.extend(tlv(0xA3, &tlv(0x30, &extensions.concat())));
        }
        let mut whole = tlv(0x30, &tbs);
        whole.extend(algorithm);
        whole.extend(tlv(0x03, &[vec![0u8], vec![7u8; 8]].concat()));
        tlv(0x30, &whole)
    }

    fn extension(object: &[u8], value: &[u8]) -> Vec<u8> {
        tlv(0x30, &[object.to_vec(), tlv(0x04, value)].concat())
    }

    /// OpenSSL's own answer to "is this certificate a self-signed trust anchor": `openssl verify` accepts a certificate that is its
    /// own only trusted certificate exactly when it is self-signed in OpenSSL's sense (the signature is not checked by default).
    fn openssl_accepts_as_own_anchor(directory: &Path, der: &[u8]) -> bool {
        let path = directory.join("anchor.pem");
        std::fs::write(&path, pem(der)).expect("write a certificate");
        Command::new("openssl")
            .args(["verify", "-CAfile"])
            .arg(&path)
            .arg(&path)
            .stdin(Stdio::null())
            .output()
            .expect("NOT_RUN: the openssl command line is the oracle of this check")
            .status
            .success()
    }

    /// Which certificates may end a chain: the rule written from OpenSSL 3.0.13 (`EXFLAG_SS`: same names, an authority key identifier
    /// that agrees with the certificate's own, a signature algorithm that suits its key) against what `openssl verify` does with the
    /// same certificate as its own only trust anchor.
    #[test]
    fn the_self_signed_rule_equals_what_openssl_verify_does_with_a_certificate_that_is_its_own_anchor(
    ) {
        let pki = Pki::generate();
        let key_info = find_key_info(&decode(
            &std::fs::read_to_string(pki.file("ca.pem"))
                .expect("the CA")
                .lines()
                .filter(|line| !line.starts_with("-----"))
                .collect::<String>(),
        ));
        let ecdsa = tlv(
            0x30,
            &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02],
        );
        let rsa = tlv(
            0x30,
            &[
                vec![
                    0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x0B,
                ],
                vec![0x05, 0x00],
            ]
            .concat(),
        );
        let skid_oid = [0x06, 0x03, 0x55, 0x1D, 0x0E];
        let akid_oid = [0x06, 0x03, 0x55, 0x1D, 0x23];
        let skid = |id: &[u8]| extension(&skid_oid, &tlv(0x04, id));
        let akid_key = |id: &[u8]| extension(&akid_oid, &tlv(0x30, &tlv(0x80, id)));
        let akid_serial = |serial: u8| extension(&akid_oid, &tlv(0x30, &tlv(0x82, &[serial])));
        let a = name(&[vec![printable("Anchor A")]]);
        let b = name(&[vec![printable("Anchor B")]]);
        let cases: Vec<(&str, Vec<u8>, bool)> = vec![
            (
                "self-issued, no extensions",
                certificate_with(&a, &a, &ecdsa, 1, &[], &key_info),
                true,
            ),
            (
                "self-issued, identifiers agree",
                certificate_with(
                    &a,
                    &a,
                    &ecdsa,
                    1,
                    &[skid(b"k1"), akid_key(b"k1")],
                    &key_info,
                ),
                true,
            ),
            (
                "self-issued, key identifiers differ",
                certificate_with(
                    &a,
                    &a,
                    &ecdsa,
                    1,
                    &[skid(b"k1"), akid_key(b"k2")],
                    &key_info,
                ),
                false,
            ),
            (
                "self-issued, authority serial differs",
                certificate_with(&a, &a, &ecdsa, 1, &[akid_serial(9)], &key_info),
                false,
            ),
            (
                "self-issued, authority serial agrees",
                certificate_with(&a, &a, &ecdsa, 1, &[akid_serial(1)], &key_info),
                true,
            ),
            (
                "self-issued, RSA signature algorithm on an EC key",
                certificate_with(&a, &a, &rsa, 1, &[], &key_info),
                false,
            ),
            (
                "issued by another name",
                certificate_with(&b, &a, &ecdsa, 1, &[], &key_info),
                false,
            ),
        ];
        let directory = private("model-anchors-");
        let mut wrong: Vec<String> = Vec::new();
        for (label, der, expected) in &cases {
            let oracle = openssl_accepts_as_own_anchor(directory.path(), der);
            let ours = tls_trust::facts(der).map(|facts| tls_trust::is_self_signed(&facts));
            if oracle != *expected || ours != Some(*expected) {
                wrong.push(format!(
                    "{label}: openssl {oracle}, ours {ours:?}, expected {expected}"
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} differ:\n{}",
            wrong.len(),
            cases.len(),
            wrong.join("\n")
        );
    }

    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let value = chunk.iter().enumerate().fold(0u32, |acc, (at, &byte)| {
                acc | (u32::from(byte) << (16 - 8 * at))
            });
            for at in 0..4 {
                if at <= chunk.len() {
                    out.push(char::from(
                        ALPHABET[((value >> (18 - 6 * at)) & 63) as usize],
                    ));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn pem(der: &[u8]) -> String {
        let encoded = base64(der);
        let lines: Vec<&str> = encoded
            .as_bytes()
            .chunks(64)
            .map(|line| std::str::from_utf8(line).expect("ASCII"))
            .collect();
        format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            lines.join("\n")
        )
    }

    fn printable(text: &str) -> Attribute<'static> {
        (&COMMON_NAME, 0x13, text.as_bytes().to_vec())
    }

    #[test]
    fn the_canonical_name_hash_equals_openssls_for_many_names() {
        let pki = Pki::generate();
        // A valid SubjectPublicKeyInfo to put in the synthetic certificates (the CA's own).
        let key_info = {
            let der = {
                let pem = super::super::support::openssl(&[
                    "x509",
                    "-in",
                    &pki.file("ca.pem").to_string_lossy(),
                    "-pubkey",
                    "-noout",
                ]);
                let body: String = pem
                    .lines()
                    .filter(|line| !line.starts_with("-----"))
                    .collect();
                body
            };
            // decode with the system tool-free path: the CA certificate's own key info element
            let _ = der;
            let certificate = std::fs::read_to_string(pki.file("ca.pem")).expect("the CA");
            let body: String = certificate
                .lines()
                .filter(|line| !line.starts_with("-----"))
                .collect();
            let bytes = decode(&body);
            find_key_info(&bytes)
        };
        let long_value = "x".repeat(200);
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("printable", name(&[vec![printable("Local Root")]])),
            (
                "spaces and case",
                name(&[vec![(&COMMON_NAME, 0x0C, b"  LOCAL   root  ".to_vec())]]),
            ),
            (
                "several rdns, three encodings",
                name(&[
                    vec![(&COUNTRY, 0x13, b"US".to_vec())],
                    vec![(&ORGANIZATION, 0x0C, b"My  Org".to_vec())],
                    vec![(&COMMON_NAME, 0x14, b"caf\xe9".to_vec())],
                ]),
            ),
            (
                "multi-valued rdn written out of order",
                name(&[vec![
                    (&COMMON_NAME, 0x13, b"A".to_vec()),
                    (&ORGANIZATION, 0x0C, b"B".to_vec()),
                ]]),
            ),
            (
                "multi-valued rdn written in order",
                name(&[vec![
                    (&ORGANIZATION, 0x0C, b"B".to_vec()),
                    (&COMMON_NAME, 0x13, b"A".to_vec()),
                ]]),
            ),
            (
                "bmp",
                name(&[vec![(&COMMON_NAME, 0x1E, utf16("Ünï  Çode"))]]),
            ),
            (
                "universal beyond the BMP",
                name(&[vec![(&COMMON_NAME, 0x1C, utf32("Ab 😀 C"))]]),
            ),
            (
                "universal",
                name(&[vec![(&COMMON_NAME, 0x1C, utf32("Ab  C"))]]),
            ),
            (
                "ia5 email",
                name(&[vec![(&EMAIL, 0x16, b"Test@Example.COM".to_vec())]]),
            ),
            // VisibleString is canonicalised by OpenSSL too, but the command line of this build refuses to load a name holding one
            // ("unsupported"), so there is no oracle for it here: that string type is covered by the unit test of the canonical strings.
            (
                "numeric is copied",
                name(&[vec![(&UNKNOWN, 0x12, b"12 34".to_vec())]]),
            ),
            ("empty subject", name(&[])),
            (
                "non-ASCII UTF-8",
                name(&[vec![(
                    &COMMON_NAME,
                    0x0C,
                    "Éclair  ÜBER".as_bytes().to_vec(),
                )]]),
            ),
            (
                "inner white space",
                name(&[vec![(&COMMON_NAME, 0x0C, b"a\t\tb\nc\x0bd".to_vec())]]),
            ),
            (
                "long value",
                name(&[vec![(&COMMON_NAME, 0x0C, long_value.clone().into_bytes())]]),
            ),
            (
                "two units in two rdns",
                name(&[
                    vec![(&UNIT, 0x13, b"b".to_vec())],
                    vec![(&UNIT, 0x13, b"a".to_vec())],
                ]),
            ),
            (
                "only spaces",
                name(&[vec![(&COMMON_NAME, 0x13, b"   ".to_vec())]]),
            ),
        ];
        let directory = private("model-names-");
        let mut wrong: Vec<String> = Vec::new();
        for (label, subject) in &cases {
            let der = certificate(subject, &key_info);
            let path = directory.path().join("name.pem");
            std::fs::write(&path, pem(&der)).expect("write a certificate");
            let output = Command::new("openssl")
                .args([
                    "x509",
                    "-in",
                    &path.to_string_lossy(),
                    "-noout",
                    "-subject_hash",
                ])
                .stdin(Stdio::null())
                .output()
                .expect("NOT_RUN: the openssl command line is the oracle of this check");
            if !output.status.success() {
                wrong.push(format!(
                    "{label}: the oracle cannot read the certificate: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
                continue;
            }
            let oracle = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let ours = tls_trust::subject_hash(&der).map(|hash| format!("{hash:08x}"));
            if ours.as_deref() != Some(oracle.as_str()) {
                wrong.push(format!("{label}: openssl {oracle}, ours {ours:?}"));
            }
        }
        // The same attributes in the two orders hash alike, and the certificates this module's fixtures use agree too.
        let a = tls_trust::subject_hash(&certificate(&cases[3].1, &key_info));
        let b = tls_trust::subject_hash(&certificate(&cases[4].1, &key_info));
        assert_eq!(a, b);
        for file in ["ca.pem", "other.pem", "server.pem"] {
            let text = std::fs::read_to_string(pki.file(file)).expect("a certificate");
            let der = decode(
                &text
                    .lines()
                    .filter(|line| !line.starts_with("-----"))
                    .collect::<String>(),
            );
            assert_eq!(
                tls_trust::subject_hash(&der).map(|hash| format!("{hash:08x}")),
                Some(pki.subject_hash(file)),
                "{file}"
            );
        }
        assert!(
            wrong.is_empty(),
            "{} of {} differ:\n{}",
            wrong.len(),
            cases.len(),
            wrong.join("\n")
        );
    }

    fn decode(body: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut accumulator, mut bits) = (0u32, 0u32);
        for byte in body.bytes().filter(|byte| *byte != b'=') {
            let value = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => continue,
            };
            accumulator = (accumulator << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((accumulator >> bits) as u8);
                accumulator &= (1 << bits) - 1;
            }
        }
        out
    }

    /// The SubjectPublicKeyInfo element of a DER certificate (the one after the subject in the TBS).
    fn find_key_info(der: &[u8]) -> Vec<u8> {
        fn read(data: &[u8]) -> (u8, &[u8], &[u8], &[u8]) {
            let first = data[1];
            let (length, header) = if first < 0x80 {
                (usize::from(first), 2)
            } else {
                let count = usize::from(first & 0x7F);
                (
                    data[2..2 + count]
                        .iter()
                        .fold(0usize, |acc, &byte| (acc << 8) | usize::from(byte)),
                    2 + count,
                )
            };
            (
                data[0],
                &data[header..header + length],
                &data[header + length..],
                &data[..header + length],
            )
        }
        let (_, certificate, _, _) = read(der);
        let (_, tbs, _, _) = read(certificate);
        let mut rest = tbs;
        let (tag, _, after, _) = read(rest);
        if tag == 0xA0 {
            rest = after;
        }
        // serial, signature, issuer, validity, subject, then the key info
        for _ in 0..5 {
            let (_, _, after, _) = read(rest);
            rest = after;
        }
        let (_, _, _, whole) = read(rest);
        whole.to_vec()
    }
}
