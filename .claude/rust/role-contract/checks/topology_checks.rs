//! Native topology predicate and campaign read side (`role_contract::{topology, campaign}`): the facts of their original
//! sources that are cheap to pin without a saved observation (path normalization, pointer rules, header window and 8192-byte
//! chunking, derive kinds). Saved-observation replays are in the B package check `topology_parity_native`.

use agent_runtime_support::pystr::PyStr;
use role_contract::campaign::{
    active_campaign_id, derive_path, join, normalize, read_declaration, Derived, ModuleSite,
    BOOTSTRAP,
};
use role_contract::topology::read_layer_header;
use std::path::{Path, PathBuf};

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("topology_checks")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn put(root: &Path, rel: &str, bytes: &[u8]) -> Vec<u8> {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(&path, bytes).expect("write");
    path.to_str().expect("UTF-8").as_bytes().to_vec()
}

fn bytes(path: &Path) -> Vec<u8> {
    path.to_str().expect("UTF-8").as_bytes().to_vec()
}

#[test]
fn paths_normalize_like_pathlib() {
    assert_eq!(normalize(b""), b".");
    assert_eq!(normalize(b"a//b/./c/"), b"a/b/c");
    assert_eq!(normalize(b"//a"), b"//a");
    assert_eq!(normalize(b"///a"), b"/a");
    assert_eq!(normalize(b"a/../b"), b"a/../b");
    assert_eq!(join(b"/r", b"campaigns"), b"/r/campaigns");
    assert_eq!(join(b"/r", b"/abs/x"), b"/abs/x");
    assert_eq!(join(b".", b"x"), b"x");
}

#[test]
fn the_active_pointer_names_an_existing_directory_else_bootstrap() {
    let root = scratch("active");
    let module = b"/m/campaign_init.py".as_slice();
    let id = |repo: &Path| {
        active_campaign_id(Some(&bytes(repo)), &bytes(repo), module)
            .expect("active")
            .to_utf8()
            .expect("text")
    };
    assert_eq!(id(&root), BOOTSTRAP);
    put(&root, "campaigns/ACTIVE", b"camp-x\r\n");
    assert_eq!(id(&root), BOOTSTRAP, "named directory is missing");
    std::fs::create_dir_all(root.join("campaigns/camp-x")).expect("dir");
    assert_eq!(
        id(&root),
        "camp-x",
        "CRLF and surrounding whitespace are stripped"
    );
    put(&root, "campaigns/ACTIVE", b"\n");
    assert_eq!(id(&root), BOOTSTRAP, "empty name");
    let other = root.join("elsewhere");
    std::fs::create_dir_all(&other).expect("dir");
    put(
        &root,
        "campaigns/ACTIVE",
        other.to_str().expect("UTF-8").as_bytes(),
    );
    assert_eq!(
        id(&root),
        other.to_str().expect("UTF-8"),
        "an absolute name replaces the base"
    );
    put(&root, "campaigns/ACTIVE", b"camp-x\0y");
    assert_eq!(id(&root), BOOTSTRAP, "an embedded NUL makes is_dir false");
    // an invalid pointer is the exception Python raises, not bootstrap
    put(&root, "campaigns/ACTIVE", b"\xff");
    let crash = active_campaign_id(Some(&bytes(&root)), &bytes(&root), module)
        .expect_err("UnicodeDecodeError");
    assert_eq!(crash.class, "UnicodeDecodeError");
    assert!(crash
        .message
        .to_utf8()
        .expect("text")
        .contains("can't decode byte 0xff in position 0"));
}

#[test]
fn the_declaration_is_json_and_only_an_object_counts() {
    let root = scratch("declaration");
    let base = bytes(&root);
    put(
        &root,
        "campaign.yaml",
        br#"{"control_variables": {"topology": "multi"}, "n": 123456789012345678901234567890}"#,
    );
    let decl = read_declaration(&base).expect("declaration");
    assert!(decl.get("control_variables").is_some());
    for body in [
        &b"[1,2]"[..],
        b"topology: multi\n",
        b"\xff",
        b"\xef\xbb\xbf{\"a\":1}",
    ] {
        put(&root, "campaign.yaml", body);
        assert!(
            read_declaration(&base).expect("declaration").is_empty(),
            "{body:?}"
        );
    }
    std::fs::remove_file(root.join("campaign.yaml")).expect("remove");
    assert!(read_declaration(&base).expect("declaration").is_empty());
}

#[test]
fn derive_path_follows_the_original_kinds_and_refusals() {
    let root = scratch("derive");
    let repo = bytes(&root);
    let camp = PyStr::new("zz");
    let path = |kind: &str, cell: Option<&str>, sweep: Option<&str>, context: Option<&str>| {
        derive_path(
            kind,
            cell,
            sweep,
            context,
            Some(&camp),
            Some(&repo),
            ModuleSite {
                root: &repo,
                file: b"/m",
            },
        )
        .expect("derive")
    };
    let base = format!("{}/campaigns/zz", root.display());
    assert_eq!(
        path("root", None, None, None),
        Derived::Path(base.clone().into_bytes())
    );
    assert_eq!(
        path("config", Some("c1"), None, None),
        Derived::Path(format!("{base}/cells/c1/config.yaml").into_bytes())
    );
    assert_eq!(
        path("cell-status", Some("c1"), None, None),
        Derived::Path(format!("{base}/cells/c1/cell.status.json").into_bytes())
    );
    assert_eq!(
        path("sweep", None, Some("s1"), None),
        Derived::Path(format!("{base}/sweeps/s1.json").into_bytes())
    );
    assert_eq!(
        path("relay", None, None, Some("ctx")),
        Derived::Path(format!("{base}/relay/ctx.json").into_bytes())
    );
    assert_eq!(
        path("relay", None, None, None),
        Derived::Path(format!("{base}/relay").into_bytes())
    );
    assert_eq!(
        path("pending-hitl", None, None, None),
        Derived::Path(format!("{base}/relay/pending_hitl.json").into_bytes())
    );
    assert_eq!(
        path("evidence", None, None, None),
        Derived::Path(format!("{base}/evidence_pointers.json").into_bytes())
    );
    assert_eq!(
        path("phases", None, None, None),
        Derived::Path(format!("{base}/phases").into_bytes())
    );
    let Derived::Refusal(refusal) = path("config", None, None, None) else {
        panic!("refusal")
    };
    assert_eq!(
        refusal.0.to_utf8().expect("text"),
        "--derive config 는 --cell 이 필요하다(셀 없는 셀 입력은 없다)"
    );
    let Derived::Refusal(refusal) = path("bogus", None, None, None) else {
        panic!("refusal")
    };
    assert_eq!(
        refusal.0.to_utf8().expect("text"),
        "알 수 없는 파생 종류: 'bogus'"
    );
}

#[test]
fn the_header_window_the_closed_form_and_the_decoding_chunk() {
    let root = scratch("header");
    let read = |body: &[u8]| {
        let path = put(&root, "x.topology.md", body);
        let (value, why) = read_layer_header(&path);
        (value, why.to_utf8().expect("text"))
    };
    assert_eq!(
        read(b"# t\n\n**topology: multi** \xc2\xb7 layer\n").0,
        Some("multi")
    );
    assert_eq!(read("**topology:\tsingle**\n".as_bytes()).0, Some("single"));
    assert_eq!(
        read(b"**topology: multi**\r\n").0,
        Some("multi"),
        "universal newlines"
    );
    for body in [
        &b"topology: multi\n"[..],
        b"**Topology: multi**\n",
        b"**topology:multix**\n",
        b"\xef\xbb\xbf**topology: multi**\n",
        b"",
    ] {
        assert_eq!(read(body).0, None, "{body:?}");
    }
    let late = format!("{}**topology: single**\n", "\n".repeat(10));
    assert_eq!(read(late.as_bytes()).0, None, "outside the 10-line window");
    let (value, why) = read(b"**topology: multi**\n**topology: single**\n");
    assert_eq!(value, None);
    assert!(
        why.contains("2개") && why.contains("['multi', 'single']"),
        "{why}"
    );
    // invalid UTF-8 inside the first 8192-byte chunk is a read failure even beyond line 10; beyond the chunk it is never seen
    let mut inside = b"**topology: multi**\n".to_vec();
    inside.extend(std::iter::repeat_n(b'\n', 20));
    inside.push(0xff);
    let (value, why) = read(&inside);
    assert_eq!(value, None);
    assert!(
        why.starts_with(
            "읽기 실패: 'utf-8' codec can't decode byte 0xff in position 40: invalid start byte"
        ),
        "{why}"
    );
    let mut beyond = b"**topology: multi**\n".to_vec();
    beyond.extend(std::iter::repeat_n(b'\n', 9000));
    beyond.push(0xff);
    assert_eq!(read(&beyond).0, Some("multi"));
}

#[test]
fn a_missing_file_and_a_directory_are_read_failures_with_the_oserror_text() {
    let root = scratch("header_os");
    let missing = bytes(&root.join("nope.topology.md"));
    let (value, why) = read_layer_header(&missing);
    assert_eq!(value, None);
    assert_eq!(
        why.to_utf8().expect("text"),
        format!(
            "읽기 실패: [Errno 2] No such file or directory: '{}'",
            root.join("nope.topology.md").display()
        )
    );
    std::fs::create_dir_all(root.join("dir.topology.md")).expect("dir");
    let (_, why) = read_layer_header(&bytes(&root.join("dir.topology.md")));
    assert!(why
        .to_utf8()
        .expect("text")
        .contains("[Errno 21] Is a directory"));
}

// ---- text profile, git decode, realpath (H2 repair) ---------------------------------------------------------------

use role_contract::pyprofile::{
    decode_preferred, encode_stream, with_profile, Encoding, Errors, TextProfile,
};
use role_contract::topology::{evaluate, layer_files, resolve_checked};

/// One observed interpreter row: the environment and what the interpreter reported (stdout encoding, errors, UTF-8 mode, preferred).
type ProfileRow<'a> = (&'a [(&'a str, &'a str)], Encoding, Errors, bool, Encoding);

fn profile_of(vars: &[(&str, &str)]) -> Result<TextProfile, String> {
    let owned: Vec<(String, String)> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    TextProfile::from_environment(&|name| {
        owned
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_bytes().to_vec())
    })
}

#[test]
fn the_six_observed_interpreter_profiles_are_modelled_exactly() {
    // (environment, stdout encoding, stdout errors, utf8_mode, preferred, filesystem) as the interpreter reported (PIDs 1425187..1425192)
    let utf8 = Encoding::Utf8;
    let rows: [ProfileRow; 6] = [
        (&[], utf8, Errors::SurrogateEscape, true, utf8),
        (
            &[("LC_ALL", "C"), ("LANG", "C")],
            utf8,
            Errors::SurrogateEscape,
            true,
            utf8,
        ),
        (
            &[
                ("LC_ALL", "C"),
                ("LANG", "C"),
                ("PYTHONUTF8", "0"),
                ("PYTHONCOERCECLOCALE", "0"),
            ],
            Encoding::Ascii,
            Errors::SurrogateEscape,
            false,
            Encoding::Ascii,
        ),
        (
            &[("LC_ALL", "C.UTF-8"), ("LANG", "C.UTF-8")],
            utf8,
            Errors::SurrogateEscape,
            false,
            utf8,
        ),
        (
            &[("LC_ALL", "C"), ("LANG", "C"), ("PYTHONUTF8", "1")],
            utf8,
            Errors::SurrogateEscape,
            true,
            utf8,
        ),
        (
            &[
                ("LC_ALL", "C.UTF-8"),
                ("LANG", "C.UTF-8"),
                ("PYTHONIOENCODING", "ascii:strict"),
            ],
            Encoding::Ascii,
            Errors::Strict,
            false,
            utf8,
        ),
    ];
    for (vars, stdout, errors, mode, preferred) in rows {
        let profile = profile_of(vars).expect("modelled profile");
        assert_eq!(
            (
                profile.stdout,
                profile.stdout_errors,
                profile.utf8_mode,
                profile.preferred,
                profile.filesystem
            ),
            (stdout, errors, mode, preferred, preferred),
            "{vars:?}"
        );
    }
}

#[test]
fn other_combinations_follow_the_startup_rules_and_unsupported_ones_are_named() {
    // LANG=C alone (no LC_ALL) with UTF-8 mode off is coerced to C.UTF-8 unless PYTHONCOERCECLOCALE=0
    let coerced = profile_of(&[("LANG", "C"), ("PYTHONUTF8", "0")]).expect("coerced");
    assert_eq!(
        (coerced.stdout, coerced.filesystem),
        (Encoding::Utf8, Encoding::Utf8)
    );
    let strict = profile_of(&[
        ("LANG", "C"),
        ("PYTHONUTF8", "0"),
        ("PYTHONCOERCECLOCALE", "0"),
    ])
    .expect("strict");
    assert_eq!(
        (strict.stdout, strict.filesystem, strict.preferred),
        (Encoding::Ascii, Encoding::Ascii, Encoding::Ascii)
    );
    // another UTF-8 locale: strict errors, UTF-8 everywhere
    let other = profile_of(&[("LC_ALL", "en_US.UTF-8")]).expect("utf-8 locale");
    assert_eq!(
        (other.stdout, other.stdout_errors),
        (Encoding::Utf8, Errors::Strict)
    );
    // concrete unsupported conditions
    assert!(profile_of(&[("LC_ALL", "de_DE.ISO-8859-1")])
        .expect_err("charset")
        .contains("neither UTF-8 nor the C locale"));
    assert!(profile_of(&[("PYTHONIOENCODING", "latin-1")])
        .expect_err("io encoding")
        .contains("PYTHONIOENCODING encoding"));
    assert!(profile_of(&[("PYTHONIOENCODING", "utf-8:ignore")])
        .expect_err("handler")
        .contains("error handler"));
}

#[test]
fn stdout_encoding_follows_the_profile_and_stderr_never_fails() {
    let text = agent_runtime_support::pystr::PyStr::new("é를");
    assert_eq!(
        encode_stream(&text, Encoding::Utf8, Errors::Strict).expect("utf-8"),
        "é를".as_bytes()
    );
    let message = encode_stream(&text, Encoding::Ascii, Errors::Strict)
        .expect_err("ascii strict")
        .message;
    assert_eq!(
        message,
        "'ascii' codec can't encode characters in position 0-1: ordinal not in range(128)"
    );
    assert_eq!(
        encode_stream(&text, Encoding::Ascii, Errors::BackslashReplace).expect("stderr"),
        b"\\xe9\\ub97c"
    );
}

#[test]
fn git_output_is_decoded_with_the_preferred_encoding_of_the_profile() {
    let root = scratch("git_profile");
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("git")
            .status;
        assert!(status.success());
    };
    run(&["init", "-q"]);
    put(&root, "é.topology.md", b"**topology: single**\n");
    run(&["add", "-A"]);
    let repo = bytes(&root);
    let ascii = profile_of(&[
        ("LC_ALL", "C"),
        ("PYTHONUTF8", "0"),
        ("PYTHONCOERCECLOCALE", "0"),
    ])
    .expect("strict C");
    let crash = with_profile(&ascii, || layer_files(&repo))
        .expect_err("a non-ASCII tracked name under the ascii profile");
    assert_eq!(crash.class, "UnicodeDecodeError");
    assert!(
        crash
            .message
            .to_utf8()
            .expect("text")
            .starts_with("'ascii' codec can't decode byte 0xc3 in position"),
        "{}",
        crash.message.to_utf8().unwrap_or_default()
    );
    let utf8 = profile_of(&[("LC_ALL", "C.UTF-8")]).expect("C.UTF-8");
    let files = with_profile(&utf8, || layer_files(&repo)).expect("utf-8 decodes it");
    assert_eq!(files.len(), 1);
    assert_eq!(
        with_profile(&ascii, || decode_preferred(b"abc")).expect("ascii text"),
        "abc"
    );
}

#[test]
fn a_symlink_loop_is_the_runtime_error_of_pathlib_resolve_and_chains_are_followed() {
    let root = scratch("realpath");
    std::fs::create_dir_all(root.join("real/sub")).expect("dirs");
    std::os::unix::fs::symlink("real", root.join("link")).expect("link");
    std::os::unix::fs::symlink("link/sub", root.join("chain")).expect("chain");
    let resolved = resolve_checked(&bytes(&root.join("chain/../x"))).expect("resolves");
    assert_eq!(
        resolved,
        bytes(&root.join("real/x")),
        "the chain is followed and `..` applies to the resolved directory"
    );
    std::os::unix::fs::symlink("loop2", root.join("loop1")).expect("loop");
    std::os::unix::fs::symlink("loop1", root.join("loop2")).expect("loop");
    let crash = resolve_checked(&bytes(&root.join("loop1"))).expect_err("loop");
    assert_eq!(crash.class, "RuntimeError");
    assert!(crash
        .message
        .to_utf8()
        .expect("text")
        .starts_with("Symlink loop from '"));
    // evaluate refuses nothing about a missing tail: a nonexistent repo resolves lexically
    assert!(evaluate(&bytes(&root.join("absent")), None).is_ok());
}

// ---- H3: typed details, route Unmet, invalid PYTHONUTF8, abspath -------------------------------------------------

use role_contract::campaign::derive_path as derive;
use role_contract::source_identity::{route, Route};
use role_contract::topology::{abspath, Detail};

#[test]
fn an_invalid_pythonutf8_is_a_concrete_startup_profile_failure_not_zero() {
    let error = profile_of(&[("PYTHONUTF8", "2")]).expect_err("invalid value");
    assert!(
        error.contains("PYTHONUTF8") && error.contains("invalid"),
        "{error}"
    );
    // an empty value is ignored like an unset one (UTF-8 mode follows the C locale)
    assert!(
        profile_of(&[("PYTHONUTF8", "")])
            .expect("empty is ignored")
            .utf8_mode
    );
}

#[test]
fn a_named_owner_that_does_not_exist_is_unmet_while_a_missing_campaign_module_stays_the_absent_leg()
{
    let root = scratch("route");
    assert!(matches!(
        route(Some(&bytes(&root.join("no_such_owner.py"))), None),
        Route::Unmet { .. }
    ));
    assert_eq!(
        route(None, Some(&bytes(&root))),
        Route::NativeCurrentSource,
        "no campaign module: the original's absent leg"
    );
    let owner = put(&root, "owner.py", b"print('a different owner')\n");
    assert!(matches!(
        route(Some(&owner), None),
        Route::LegacySourceVariant { .. }
    ));
}

#[test]
fn encode_and_os_crashes_carry_their_typed_constructor_details() {
    let root = scratch("details");
    let repo = bytes(&root);
    let ascii = profile_of(&[
        ("LC_ALL", "C"),
        ("PYTHONUTF8", "0"),
        ("PYTHONCOERCECLOCALE", "0"),
    ])
    .expect("strict C");
    let camp = agent_runtime_support::pystr::PyStr::new("é");
    let crash = with_profile(&ascii, || {
        derive(
            "root",
            None,
            None,
            None,
            Some(&camp),
            Some(&repo),
            ModuleSite {
                root: &repo,
                file: b"/m",
            },
        )
    })
    .expect_err("unencodable id");
    assert_eq!(crash.class, "UnicodeEncodeError");
    match &*crash.detail {
        Detail::Encode {
            encoding,
            object,
            start,
            end,
            reason,
        } => {
            assert_eq!(
                (*encoding, *start, *end, reason.as_str()),
                ("ascii", 0, 1, "ordinal not in range(128)")
            );
            assert_eq!(object.to_utf8().expect("text"), "é");
        }
        other => panic!("{other:?}"),
    }
    // a symlink loop: RuntimeError with the loop's typed detail (implicit context ELOOP)
    std::os::unix::fs::symlink("loop2", root.join("loop1")).expect("loop");
    std::os::unix::fs::symlink("loop1", root.join("loop2")).expect("loop");
    let crash = resolve_checked(&bytes(&root.join("loop1"))).expect_err("loop");
    assert!(matches!(&*crash.detail, Detail::SymlinkLoop { .. }));
    // abspath: relative to the current directory, `..` collapsed lexically
    let cwd = std::env::current_dir().expect("cwd");
    assert_eq!(abspath(b"a/../b"), bytes(&cwd.join("b")));
    assert_eq!(abspath(b"/x//y/./z/.."), b"/x/y");
}

// ---- R44: the module's own location and the typed `derive_path("root", camp_id=...)` the repeat callers ask ----------------

mod owner_queries {
    use agent_runtime_support::pystr::PyStr;
    use agent_runtime_support::pyvalue::{PyDict, PyValue};
    use role_contract::campaign::{derive_root_path, module_site, ModuleSite};
    use std::path::PathBuf;

    fn site() -> (Vec<u8>, Vec<u8>) {
        (
            b"/mod/root".to_vec(),
            b"/mod/root/.claude/s/t/campaign_init.py".to_vec(),
        )
    }

    fn derive(camp: &PyValue) -> Result<String, (String, String)> {
        let (root, file) = site();
        derive_root_path(
            camp,
            ModuleSite {
                root: &root,
                file: &file,
            },
        )
        .map(|path| path.to_utf8().expect("text"))
        .map_err(|crash| {
            (
                crash.class.to_string(),
                crash.message.to_utf8().expect("text"),
            )
        })
    }

    fn text(code_points: &[u32]) -> PyValue {
        PyValue::Str(PyStr::from_code_points(code_points.to_vec()))
    }

    fn scratch(label: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("r44_owner")
            .join(format!("{}_{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        root
    }

    #[test]
    fn the_module_root_is_the_resolved_file_fifth_parent() {
        // exactly five components: the root is "/" ; deeper: the prefix
        assert_eq!(
            module_site(b"/a/b/c/d/e").expect("site").root,
            b"/".to_vec()
        );
        assert_eq!(
            module_site(b"/r/a/b/c/d/e").expect("site").root,
            b"/r".to_vec()
        );
        let site = module_site(b"/x/y/.//z/../q/w/e/r").expect("site");
        assert_eq!(
            site.file,
            b"/x/y/.//z/../q/w/e/r".to_vec(),
            "the file is the path as given"
        );
    }

    #[test]
    fn a_shallow_path_is_index_error_4_and_a_symlink_loop_is_the_resolve_runtime_error() {
        for shallow in [&b"/a"[..], b"/a/b/c", b"/a/b/c/d"] {
            let crash = module_site(shallow).expect_err("shallow");
            assert_eq!(
                (crash.class, crash.message.to_utf8().as_deref()),
                ("IndexError", Some("4"))
            );
        }
        let dir = scratch("loop");
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::os::unix::fs::symlink(&b, &a).expect("link");
        std::os::unix::fs::symlink(&a, &b).expect("link");
        let crash =
            module_site(std::os::unix::ffi::OsStrExt::as_bytes(a.as_os_str())).expect_err("loop");
        assert_eq!(crash.class, "RuntimeError");
        assert!(crash
            .message
            .to_utf8()
            .expect("text")
            .starts_with("Symlink loop from "));
    }

    #[test]
    fn a_symlinked_module_belongs_to_the_root_it_resolves_to() {
        let dir = scratch("link");
        let real = dir.join("other/.claude/skills/terraforming_node/scripts");
        std::fs::create_dir_all(&real).expect("dirs");
        std::fs::write(real.join("campaign_init.sh"), b"#").expect("file");
        let caller = dir.join("caller/.claude/skills/terraforming_node/scripts");
        std::fs::create_dir_all(&caller).expect("dirs");
        std::os::unix::fs::symlink(
            real.join("campaign_init.sh"),
            caller.join("campaign_init.sh"),
        )
        .expect("link");
        let via_link = caller.join("campaign_init.sh");
        let site = module_site(std::os::unix::ffi::OsStrExt::as_bytes(via_link.as_os_str()))
            .expect("site");
        let want = std::fs::canonicalize(dir.join("other")).expect("canonical");
        assert_eq!(
            site.root,
            std::os::unix::ffi::OsStrExt::as_bytes(want.as_os_str()).to_vec()
        );
    }

    #[test]
    fn a_truthy_value_that_is_not_text_is_the_type_error_of_the_path_division() {
        let message =
            |kind: &str| format!("unsupported operand type(s) for /: 'PosixPath' and '{kind}'");
        let mut dict = PyDict::new();
        dict.set("k", PyValue::None);
        assert_eq!(
            derive(&PyValue::Int(5.into())),
            Err(("TypeError".into(), message("int")))
        );
        assert_eq!(
            derive(&PyValue::Bool(true)),
            Err(("TypeError".into(), message("bool")))
        );
        assert_eq!(
            derive(&PyValue::Float(1.5)),
            Err(("TypeError".into(), message("float")))
        );
        assert_eq!(
            derive(&PyValue::List(vec![PyValue::None])),
            Err(("TypeError".into(), message("list")))
        );
        assert_eq!(
            derive(&PyValue::Dict(dict)),
            Err(("TypeError".into(), message("dict")))
        );
    }

    #[test]
    fn a_falsy_value_means_the_active_campaign_of_the_module_root() {
        // /mod/root does not exist: no ACTIVE pointer, the reserved bootstrap id
        let want = Ok("/mod/root/campaigns/_bootstrap".to_string());
        for falsy in [
            PyValue::None,
            PyValue::Int(0.into()),
            PyValue::Bool(false),
            PyValue::Float(0.0),
            PyValue::Str(PyStr::new("")),
            PyValue::List(Vec::new()),
            PyValue::Dict(PyDict::new()),
        ] {
            assert_eq!(derive(&falsy), want);
        }
    }

    #[test]
    fn path_text_keeps_unicode_nul_and_surrogates_and_follows_pathlib_joining() {
        let case = |camp: PyValue| derive(&camp);
        assert_eq!(
            case(text(&[0x63, 0x61, 0x6d, 0x70, 0x2d, 0x78])),
            Ok("/mod/root/campaigns/camp-x".into())
        );
        assert_eq!(
            case(text(&[0x2e, 0x2e, 0x2f, 0x78])),
            Ok("/mod/root/campaigns/../x".into()),
            "'..' stays lexical"
        );
        assert_eq!(
            case(text(&[0x2f, 0x61, 0x62, 0x73])),
            Ok("/abs".into()),
            "an absolute id replaces the base"
        );
        assert_eq!(
            case(text(&[0x2f, 0x2f, 0x61])),
            Ok("//a".into()),
            "exactly two leading slashes stay"
        );
        assert_eq!(
            case(text(&[0x61, 0x2f, 0x2f, 0x2e, 0x2f, 0x62])),
            Ok("/mod/root/campaigns/a/b".into())
        );
        assert_eq!(
            case(text(&[0xCA0, 0xD504])),
            Ok("/mod/root/campaigns/\u{ca0}\u{d504}".into())
        );
        assert_eq!(
            case(text(&[0x61, 0, 0x62])),
            Ok("/mod/root/campaigns/a\u{0}b".into()),
            "NUL is path text"
        );
        let lone = |unit: u32| {
            let (root, file) = site();
            derive_root_path(
                &text(&[unit]),
                ModuleSite {
                    root: &root,
                    file: &file,
                },
            )
            .expect("a lone surrogate is path text")
            .code_points()
            .to_vec()
        };
        let tail = |unit: u32| {
            let mut points: Vec<u32> = "/mod/root/campaigns/".chars().map(u32::from).collect();
            points.push(unit);
            points
        };
        assert_eq!(lone(0xDCFF), tail(0xDCFF));
        assert_eq!(
            lone(0xD800),
            tail(0xD800),
            "a high surrogate is not encoded for the file system"
        );
    }
}
