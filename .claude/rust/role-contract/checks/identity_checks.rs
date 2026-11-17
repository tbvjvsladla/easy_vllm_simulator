//! The node-identity resolver (`role_contract::identity`) against fixtures built from the saved observations of
//! `node_identity.sh --resolve --repo REPO` (expected bytes are those the original left for the same inputs).

use role_contract::identity::{
    host_page_size, native_grep_outcome, permitted_outcomes, resolve, resolve_with_page_size,
    valid_slug, Profile, Resolution,
};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("identity_checks")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
    std::fs::write(path, bytes).expect("write");
}

fn run(root: &Path, profile: Profile) -> Resolution {
    resolve(root.as_os_str().as_bytes(), profile)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn ok(label: &str, files: &[(&str, &[u8])], profile: Profile, expected: &str) {
    let root = scratch(label);
    for (path, bytes) in files {
        put(&root, path, bytes);
    }
    let got = run(&root, profile);
    assert_eq!(
        (got.status, text(&got.stdout)),
        (0, format!("{expected}\n")),
        "{label}: stderr {}",
        text(&got.stderr)
    );
}

const ROSTER: &[u8] = b"nodes:\n  - role: main\n  - role: sub\n";

#[test]
fn self_role_is_the_first_top_level_line_with_comment_then_quotes_stripped() {
    for profile in [Profile::C, Profile::CUtf8] {
        ok(
            "comment",
            &[(
                "output/multi/manifest.yaml",
                b"self_role: sub   # issued\nnodes:\n  - role: main\n",
            )],
            profile,
            "sub",
        );
        ok(
            "dq",
            &[("output/multi/manifest.yaml", b"self_role: \"sub\"\n")],
            profile,
            "sub",
        );
        ok(
            "sq",
            &[("output/multi/manifest.yaml", b"self_role: 'sub'\n")],
            profile,
            "sub",
        );
        ok(
            "two",
            &[(
                "output/multi/manifest.yaml",
                b"self_role: sub\nself_role: main\n",
            )],
            profile,
            "sub",
        );
        ok(
            "near",
            &[(
                "output/multi/manifest.yaml",
                b" self_role: x\nxself_role: y\nself_role_x: z\n- role: main\n",
            )],
            profile,
            "main",
        );
        ok(
            "no_newline",
            &[("output/multi/manifest.yaml", b"self_role: sub")],
            profile,
            "sub",
        );
        ok(
            "empty_value_falls_to_main",
            &[(
                "output/multi/manifest.yaml",
                b"self_role:\nnodes:\n  - role: main\n",
            )],
            profile,
            "main",
        );
        ok(
            "len32",
            &[(
                "output/multi/manifest.yaml",
                b"self_role: abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n",
            )],
            profile,
            "abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
    }
}

#[test]
fn a_comment_inside_quotes_truncates_and_leaves_one_quote_unstripped() {
    // `s/[[:space:]]*#.*$//` runs before the quote strips: `"a#b"` -> `"a` -> `a`.
    ok(
        "hash_in_quote",
        &[("output/multi/manifest.yaml", b"self_role: \"a#b\"\n")],
        Profile::C,
        "a",
    );
    // each end is stripped on its own
    ok(
        "one_sided",
        &[("output/multi/manifest.yaml", b"self_role: sub'\n")],
        Profile::C,
        "sub",
    );
}

#[test]
fn a_value_outside_the_open_slug_scheme_fails_loud_with_the_raw_value() {
    let cases: [(&[u8], &str); 4] = [
        (b"self_role: SUB!\n", "SUB!"),
        (b"self_role: sub x\n", "sub x"),
        (
            b"self_role: abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n",
            "abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
        (b"self_role: sub\r\nnodes:\n", "sub\r"),
    ];
    for (index, (bytes, value)) in cases.iter().enumerate() {
        for profile in [Profile::C, Profile::CUtf8] {
            let root = scratch(&format!("bad{index}"));
            put(&root, "output/multi/manifest.yaml", bytes);
            let got = run(&root, profile);
            assert_eq!(got.status, 1);
            assert!(got.stdout.is_empty());
            assert_eq!(text(&got.stderr), format!("[node-identity] FAIL: manifest self_role='{value}' 가 스킴 위반이다(^[a-z][a-z0-9-]{{0,31}}$).\n"));
        }
    }
    let root = scratch("nonascii");
    put(
        &root,
        "output/multi/manifest.yaml",
        "self_role: É\n".as_bytes(),
    );
    for profile in [Profile::C, Profile::CUtf8] {
        let got = run(&root, profile);
        assert_eq!(text(&got.stderr), "[node-identity] FAIL: manifest self_role='É' 가 스킴 위반이다(^[a-z][a-z0-9-]{0,31}$).\n");
    }
    assert!(
        valid_slug(b"a")
            && valid_slug(b"a-9")
            && !valid_slug(b"")
            && !valid_slug(b"9a")
            && !valid_slug(b"-a")
    );
}

#[test]
fn the_roster_main_is_counted_per_line_with_the_originals_boundaries() {
    for profile in [Profile::C, Profile::CUtf8] {
        ok(
            "roster",
            &[("output/multi/manifest.yaml", ROSTER)],
            profile,
            "main",
        );
        ok(
            "inline_comment",
            &[(
                "output/multi/manifest.yaml",
                b"nodes:\n  - role: main # x\n",
            )],
            profile,
            "main",
        );
        ok(
            "crlf",
            &[(
                "output/multi/manifest.yaml",
                b"nodes:\r\n  - role: main\r\n  - role: sub\r\n",
            )],
            profile,
            "main",
        );
        ok(
            "tabs",
            &[("output/multi/manifest.yaml", b"nodes:\n\t-\trole:\tmain\n")],
            profile,
            "main",
        );
        for (label, bytes) in [
            ("no_main", &b"nodes:\n  - role: sub\n"[..]),
            ("commented", b"nodes:\n  # - role: main\n"),
            ("quoted", b"nodes:\n  - role: \"main\"\n"),
            ("extra", b"nodes:\n  - role: mainframe\n"),
            ("no_dash", b"nodes:\n  role: main\n"),
            ("empty", b""),
            ("blank", b"\n\n\n"),
        ] {
            let root = scratch(label);
            put(&root, "output/multi/manifest.yaml", bytes);
            let got = run(&root, profile);
            assert_eq!(got.status, 1, "{label}");
            assert!(got.stdout.is_empty());
            assert!(text(&got.stderr).starts_with("[node-identity] FAIL: node_id 를 해소하지 못했다 — hostname 으로 대체하지 않는다.\n"), "{label}");
            assert_eq!(text(&got.stderr).lines().count(), 4);
        }
    }
    // NBSP is not a blank in either profile (it is excluded from the glibc `space` class).
    for profile in [Profile::C, Profile::CUtf8] {
        let root = scratch("nbsp");
        put(
            &root,
            "output/multi/manifest.yaml",
            "nodes:\n  -\u{a0}role: main\n".as_bytes(),
        );
        assert_eq!(run(&root, profile).status, 1);
    }
    // A multi-byte blank of the glibc `space` class is a blank in C.UTF-8 only.
    let em_space = "nodes:\n  -\u{2003}role: main\n";
    let root = scratch("em_space");
    put(&root, "output/multi/manifest.yaml", em_space.as_bytes());
    assert_eq!(run(&root, Profile::CUtf8).status, 0);
    assert_eq!(run(&root, Profile::C).status, 1);
}

#[test]
fn two_roster_mains_in_one_manifest_name_the_file_and_the_count_with_the_repo_spelling() {
    let root = scratch("two_main");
    put(
        &root,
        "output/multi/manifest.yaml",
        b"nodes:\n  - role: main\n  - role: main\n",
    );
    let got = resolve(format!("{}/", root.display()).as_bytes(), Profile::C);
    assert_eq!(got.status, 1);
    assert_eq!(
        text(&got.stderr),
        format!("[node-identity] FAIL: {}//output/multi/manifest.yaml 에 role: main 항목이 2개다(계약은 정확히 1개).\n[node-identity]       scan_node.sh manifest 게이트를 다시 통과시켜라.\n", root.display())
    );
}

#[test]
fn several_manifests_must_agree_and_the_glob_order_is_byte_order() {
    for profile in [Profile::C, Profile::CUtf8] {
        ok(
            "same",
            &[
                ("output/multi/manifest.yaml", b"self_role: sub\n"),
                ("output/single/manifest.yaml", b"self_role: sub\n"),
            ],
            profile,
            "sub",
        );
        ok(
            "one_only",
            &[
                ("output/multi/manifest.yaml", b"self_role: sub\n"),
                ("output/single/manifest.yaml", ROSTER),
            ],
            profile,
            "sub",
        );
        ok(
            "main_two_topologies",
            &[
                ("output/multi/manifest.yaml", ROSTER),
                ("output/single/manifest.yaml", b"self_role: main\n"),
            ],
            profile,
            "main",
        );
        // `Multi` < `_x` < `multi` in byte order: sub, then main -> mismatch names the first value first.
        let root = scratch("order");
        put(&root, "output/Multi/manifest.yaml", b"self_role: sub\n");
        put(&root, "output/_x/manifest.yaml", b"self_role: main\n");
        put(&root, "output/multi/manifest.yaml", b"self_role: sub\n");
        let got = run(&root, profile);
        assert_eq!(text(&got.stderr), "[node-identity] FAIL: manifest 간 self_role 불일치('sub' vs 'main') — terraforming_node 로 재발급하라.\n");
    }
}

#[test]
fn directories_links_and_non_files_are_skipped_or_followed_as_the_shell_tests_do() {
    // a directory named manifest.yaml fails `-f`
    let root = scratch("dir");
    std::fs::create_dir_all(root.join("output/multi/manifest.yaml")).expect("dir");
    put(&root, "output/single/manifest.yaml", ROSTER);
    assert_eq!(text(&run(&root, Profile::C).stdout), "main\n");
    // symlink followed; dangling and looping links skipped; hard link is a plain file
    let root = scratch("links");
    put(&root, "output/real/manifest.yaml", b"self_role: sub\n");
    std::fs::create_dir_all(root.join("output/multi")).expect("multi");
    symlink(
        "../real/manifest.yaml",
        root.join("output/multi/manifest.yaml"),
    )
    .expect("link");
    assert_eq!(text(&run(&root, Profile::C).stdout), "sub\n");
    let root = scratch("dangling");
    std::fs::create_dir_all(root.join("output/multi")).expect("multi");
    symlink(
        "../absent/manifest.yaml",
        root.join("output/multi/manifest.yaml"),
    )
    .expect("link");
    put(&root, "output/single/manifest.yaml", ROSTER);
    assert_eq!(text(&run(&root, Profile::C).stdout), "main\n");
    let root = scratch("loop");
    std::fs::create_dir_all(root.join("output/multi")).expect("multi");
    symlink("manifest.yaml", root.join("output/multi/manifest.yaml")).expect("loop");
    put(&root, "output/single/manifest.yaml", ROSTER);
    assert_eq!(text(&run(&root, Profile::C).stdout), "main\n");
    let root = scratch("hardlink");
    put(&root, "output/multi/manifest.yaml", b"self_role: sub\n");
    std::fs::create_dir_all(root.join("output/single")).expect("single");
    std::fs::hard_link(
        root.join("output/multi/manifest.yaml"),
        root.join("output/single/manifest.yaml"),
    )
    .expect("hard link");
    assert_eq!(text(&run(&root, Profile::C).stdout), "sub\n");
    // `output` is a file / a dot-name directory is not matched by `*`
    let root = scratch("output_file");
    put(&root, "output", b"");
    assert_eq!(run(&root, Profile::C).status, 1);
    let root = scratch("dot");
    put(&root, "output/.hidden/manifest.yaml", b"self_role: sub\n");
    assert_eq!(run(&root, Profile::C).status, 1);
}

#[test]
fn the_repo_is_a_literal_directory_spelling() {
    let root = scratch("repo");
    put(
        &root,
        "a b[x]*/output/multi/manifest.yaml",
        b"self_role: sub\n",
    );
    let got = resolve(format!("{}/a b[x]*", root.display()).as_bytes(), Profile::C);
    assert_eq!(text(&got.stdout), "sub\n");
    let file = root.join("f");
    put(&root, "f", b"");
    let got = resolve(file.as_os_str().as_bytes(), Profile::C);
    assert_eq!(
        (got.status, text(&got.stderr)),
        (
            1,
            "[node-identity] FAIL: repo 루트를 못 찾았다(node_id 해소 불가).\n".to_string()
        )
    );
    put(
        &root,
        "real/output/multi/manifest.yaml",
        b"self_role: sub\n",
    );
    symlink("real", root.join("link")).expect("repo link");
    assert_eq!(
        text(&resolve(root.join("link").as_os_str().as_bytes(), Profile::C).stdout),
        "sub\n"
    );
    assert_eq!(
        text(
            &resolve(
                format!("{}//", root.join("real").display()).as_bytes(),
                Profile::C
            )
            .stdout
        ),
        "sub\n"
    );
}

#[test]
fn nul_and_invalid_utf8_follow_gnu_grep_not_a_guess() {
    for profile in [Profile::C, Profile::CUtf8] {
        // NUL: the file is binary, the self_role line is withheld with a notice, NUL separates lines for the main count.
        let root = scratch("nul");
        put(
            &root,
            "output/multi/manifest.yaml",
            b"self_role: sub\0x\nnodes:\n  - role: main\n",
        );
        let got = run(&root, profile);
        assert_eq!((got.status, text(&got.stdout)), (0, "main\n".into()));
        assert_eq!(
            text(&got.stderr),
            format!(
                "grep: {}/output/multi/manifest.yaml: binary file matches\n",
                root.display()
            )
        );
        // invalid UTF-8 on a line that is not printed does not matter
        let root = scratch("bad_comment_line");
        put(
            &root,
            "output/multi/manifest.yaml",
            b"self_role: sub\n# \xff\xfe\nnodes:\n  - role: main\n",
        );
        let got = run(&root, profile);
        assert_eq!(
            (got.status, text(&got.stdout), got.stderr.is_empty()),
            (0, "sub\n".into(), true)
        );
    }
    // invalid UTF-8 on the selected line itself: withheld in C.UTF-8 (grep's notice), plain bytes in C
    let root = scratch("bad_self");
    put(
        &root,
        "output/multi/manifest.yaml",
        b"self_role: su\xffb\nnodes:\n  - role: main\n",
    );
    let utf8 = run(&root, Profile::CUtf8);
    assert_eq!((utf8.status, text(&utf8.stdout)), (0, "main\n".into()));
    assert!(text(&utf8.stderr).ends_with("manifest.yaml: binary file matches\n"));
    let plain = run(&root, Profile::C);
    assert_eq!(plain.status, 1);
    assert!(plain
        .stderr
        .starts_with(b"[node-identity] FAIL: manifest self_role='su\xffb'"));
}

#[test]
fn an_unreadable_manifest_is_a_failure_not_a_not_applicable() {
    let root = scratch("mode000");
    put(&root, "output/multi/manifest.yaml", ROSTER);
    std::fs::set_permissions(
        root.join("output/multi/manifest.yaml"),
        std::fs::Permissions::from_mode(0o0),
    )
    .expect("chmod own fixture");
    if std::fs::File::open(root.join("output/multi/manifest.yaml")).is_ok() {
        eprintln!("SKIPPED: the process can read a mode-000 file (privileged user)");
        return;
    }
    let got = run(&root, Profile::C);
    assert_eq!(got.status, 1);
    assert_eq!(text(&got.stderr), format!("[node-identity] FAIL: {}/output/multi/manifest.yaml 를 읽을 수 없다(권한/소유권) — 정체성 판정을 추측으로 잇지 않는다.\n", root.display()));
    // a valid first manifest does not hide an unreadable second one
    let root = scratch("mode000_second");
    put(&root, "output/multi/manifest.yaml", b"self_role: sub\n");
    put(&root, "output/single/manifest.yaml", ROSTER);
    std::fs::set_permissions(
        root.join("output/single/manifest.yaml"),
        std::fs::Permissions::from_mode(0o0),
    )
    .expect("chmod own fixture");
    let got = run(&root, Profile::C);
    assert_eq!(got.status, 1);
    assert!(text(&got.stderr).contains("output/single/manifest.yaml 를 읽을 수 없다"));
}

#[test]
fn the_locale_profile_follows_the_variable_precedence() {
    let p = |a: Option<&[u8]>, c: Option<&[u8]>, l: Option<&[u8]>| {
        Profile::from_locale_variables(a, c, l)
    };
    assert_eq!(p(None, None, None), Profile::C);
    assert_eq!(p(Some(b"C"), None, Some(b"C.UTF-8")), Profile::C);
    assert_eq!(p(Some(b""), None, Some(b"C.UTF-8")), Profile::CUtf8);
    assert_eq!(p(Some(b"C.UTF-8"), None, Some(b"C")), Profile::CUtf8);
    assert_eq!(p(None, Some(b"POSIX"), Some(b"C.utf8")), Profile::C);
}

// ---- GNU grep 3.11 buffer / encoding rules (H2 repair) --------------------------------------------------------------
// Labels: OBSERVED = the byte shape of one of the two new original observations (H01/H02 of the targeted reference);
// SOURCE-DERIVED = an expectation read out of grep-3.11/src/grep.c, NOT a new original observation.

const PAD: &[u8] = b"# padding line\n";
const PAGE: usize = 4096;

fn run_page(root: &Path, profile: Profile, page: usize) -> Resolution {
    resolve_with_page_size(root.as_os_str().as_bytes(), profile, page)
}

fn manifest(label: &str, bytes: &[u8]) -> PathBuf {
    let root = scratch(label);
    put(&root, "output/multi/manifest.yaml", bytes);
    root
}

fn pad_to(bytes: &mut Vec<u8>, length: usize) {
    while bytes.len() + PAD.len() <= length {
        bytes.extend_from_slice(PAD);
    }
}

fn notice(root: &Path) -> String {
    format!(
        "grep: {}/output/multi/manifest.yaml: binary file matches\n",
        root.display()
    )
}

#[test]
fn observed_h01_a_later_selected_line_with_an_encoding_error_adds_the_notice_and_keeps_the_first_value(
) {
    // OBSERVED (H01, C.UTF-8): value `sub`, notice on stderr, the roster is not consulted.
    let root = manifest(
        "h01",
        b"self_role: sub\nself_role: \xff\nnodes:\n  - role: main\n",
    );
    let got = run_page(&root, Profile::CUtf8, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
    assert_eq!(text(&got.stderr), notice(&root));
    // C profile (SOURCE-DERIVED): bytes are characters, no encoding error, no notice; the first line still answers.
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!(
        (got.status, text(&got.stdout), got.stderr.is_empty()),
        (0, "sub\n".to_string(), true)
    );
}

#[test]
fn source_derived_an_invalid_first_selected_line_is_skipped_and_the_scan_goes_on() {
    // print_line_head() withholds the line and sets encoding_error_output; grepbuf() continues, so the next valid
    // selected line is what `head -1` receives.
    let root = manifest("skip_first", b"self_role: su\xffb\nself_role: sub\n");
    let got = run_page(&root, Profile::CUtf8, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
    assert_eq!(text(&got.stderr), notice(&root));
}

#[test]
fn observed_h02_a_late_nul_does_not_erase_the_first_matching_line() {
    // OBSERVED (H02, C.UTF-8): 1_200_039 bytes, value `sub`, stderr EMPTY.
    let mut bytes = b"self_role: sub\nnodes:\n  - role: main\n".to_vec();
    for _ in 0..80_000 {
        bytes.extend_from_slice(PAD);
    }
    bytes.extend_from_slice(b"\0\n");
    assert_eq!(bytes.len(), 1_200_039);
    let root = manifest("h02", &bytes);
    for page in [
        PAGE,
        16384,
        65536,
        host_page_size().expect("host page size"),
    ] {
        for profile in [Profile::C, Profile::CUtf8] {
            let got = run_page(&root, profile, page);
            assert_eq!(
                (got.status, text(&got.stdout), got.stderr.is_empty()),
                (0, "sub\n".to_string(), true),
                "{page:?} {profile:?}"
            );
        }
    }
}

#[test]
fn source_derived_a_selected_line_after_the_nul_buffer_adds_the_notice_but_earlier_lines_stay_printed(
) {
    let mut bytes = b"self_role: sub\n".to_vec();
    pad_to(&mut bytes, 100_000);
    let nul_at = 250_000;
    pad_to(&mut bytes, nul_at);
    bytes.extend_from_slice(b"\0\nself_role: later\n");
    let root = manifest("after_nul", &bytes);
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
    assert_eq!(text(&got.stderr), notice(&root));
}

#[test]
fn source_derived_a_nul_in_the_first_buffer_makes_every_selected_line_quiet() {
    // reads are at least 96 KiB: a NUL inside the first read switches the whole first buffer to quiet; the first value is
    // not printed and the roster answers (the unchanged F01 shape, here with the NUL after the first line)
    let root = manifest(
        "nul_first",
        b"self_role: sub\nx\0y\nnodes:\n  - role: main\n",
    );
    for page in [PAGE, 65536] {
        let got = run_page(&root, Profile::CUtf8, page);
        assert_eq!(
            (got.status, text(&got.stdout)),
            (0, "main\n".to_string()),
            "{page:?}"
        );
        assert_eq!(text(&got.stderr), notice(&root));
    }
}

/// A file whose first NUL sits in read buffer 2 (offset about 2 x 96 KiB) and whose `self_role: x` line ends at
/// `selected_end`; the read-2 start is 2A-P or 2A depending on grep's buffer address.
fn two_reads_file(selected_end: usize) -> Vec<u8> {
    let mut bytes = b"self_role: sub\n".to_vec();
    // a 200-byte comment line straddles the end of the first 96 KiB read, so the saved incomplete line is ~100 bytes and the
    // page-aligned start of read 1 can move by one page with the (unknown) buffer address
    pad_to(&mut bytes, 98_200);
    bytes.push(b'#');
    bytes.extend(std::iter::repeat_n(b'x', 198));
    bytes.push(b'\n');
    pad_to(&mut bytes, selected_end - 13);
    bytes.extend_from_slice(b"self_role: x\n");
    pad_to(&mut bytes, 197_608);
    bytes.extend_from_slice(b"\0\n");
    bytes
}

#[test]
fn source_derived_a_line_ending_inside_the_buffer_start_ambiguity_has_a_set_of_permitted_outputs_and_business_status_0(
) {
    // 2A-P = 192512 <= 196497 < 2A = 196608: GNU permits both "quiet" (read starts at 2A-P) and "printed" (at 2A) for page
    // size 4096; which one a process gets depends on the address of its own buffer (upstream nondeterminism).
    let bytes = two_reads_file(196_497);
    let permitted = permitted_outcomes(&bytes, Profile::C, PAGE);
    assert!(
        permitted.len() >= 2,
        "the input is ambiguous under GNU: {permitted:?}"
    );
    assert!(permitted.iter().any(|o| o.notice) && permitted.iter().any(|o| !o.notice));
    // the native allocation selects a member, and the resolver answers with the original's business domain
    assert!(permitted.contains(&native_grep_outcome(&bytes, Profile::C, PAGE)));
    let root = manifest("ambiguous", &bytes);
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
    assert!(
        got.stderr.is_empty() || text(&got.stderr) == notice(&root),
        "{}",
        text(&got.stderr)
    );
}

#[test]
fn source_derived_lines_far_from_the_ambiguity_are_decided_by_the_source_alone() {
    // ends at ~100 KiB: read 1, before every possible read-2 start -> printed, no notice
    let root = manifest("early_line", &two_reads_file(100_003));
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!(
        (got.status, text(&got.stdout), got.stderr.is_empty()),
        (0, "sub\n".to_string(), true)
    );
    // ends after the NUL (197_608): quiet in every layout -> the first value is printed, the notice is added
    let mut bytes = b"self_role: sub\n".to_vec();
    pad_to(&mut bytes, 197_000);
    bytes.extend_from_slice(b"\0\n");
    pad_to(&mut bytes, 200_000);
    bytes.extend_from_slice(b"self_role: y\n");
    let root = manifest("late_line", &bytes);
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
    assert_eq!(text(&got.stderr), notice(&root));
}

#[test]
fn source_derived_a_line_longer_than_the_initial_buffer_grows_the_buffer_and_stays_a_business_0_or_1(
) {
    // xpalloc growth: the saved line (> A - P bytes) before the first NUL forces a new, larger allocation. No rejection.
    let mut bytes = b"self_role: sub\n".to_vec();
    bytes.push(b'#');
    bytes.extend(std::iter::repeat_n(b'x', 300_000));
    bytes.extend_from_slice(b"\n\0\n");
    let root = manifest("long_line", &bytes);
    for page in [PAGE, 16384, 65536] {
        // the only selected line is completed in read 0, before the NUL buffer, whatever the allocations are
        assert_eq!(
            permitted_outcomes(&bytes, Profile::C, page).len(),
            1,
            "{page}"
        );
        let got = run_page(&root, Profile::C, page);
        assert_eq!(
            (got.status, text(&got.stdout), got.stderr.is_empty()),
            (0, "sub\n".to_string(), true),
            "{page}"
        );
    }
    // a selected line after the long line, ending near the growth-dependent read boundary: a set, a member is chosen
    let mut bytes = b"self_role: sub\n".to_vec();
    bytes.push(b'#');
    bytes.extend(std::iter::repeat_n(b'x', 150_000));
    bytes.extend_from_slice(b"\nself_role: x\n");
    bytes.push(b'#');
    bytes.extend(std::iter::repeat_n(b'y', 200_000));
    bytes.extend_from_slice(b"\n\0\n");
    let permitted = permitted_outcomes(&bytes, Profile::C, PAGE);
    assert!(!permitted.is_empty());
    assert!(permitted.contains(&native_grep_outcome(&bytes, Profile::C, PAGE)));
    for outcome in &permitted {
        // every permitted output keeps the first valid selected line (it ends in read 0)
        assert_eq!(outcome.printed.first(), Some(&(0, 14)));
    }
    let root = manifest("long_line_two", &bytes);
    let got = run_page(&root, Profile::C, PAGE);
    assert_eq!((got.status, text(&got.stdout)), (0, "sub\n".to_string()));
}

#[test]
fn source_derived_every_ordinary_input_stays_in_the_business_domain_for_every_page_size() {
    // property: status is 0 or 1 for all of these under every page size (no 3, no other status)
    for page in [4096usize, 16384, 65536] {
        for bytes in [
            two_reads_file(196_497),
            two_reads_file(100_003),
            b"self_role: sub\n".to_vec(),
            b"nodes:\n".to_vec(),
        ] {
            let root = manifest("domain", &bytes);
            for profile in [Profile::C, Profile::CUtf8] {
                let got = run_page(&root, profile, page);
                assert!(
                    got.status == 0 || got.status == 1,
                    "{page} {profile:?} {}",
                    got.status
                );
            }
        }
    }
}

#[test]
fn the_host_page_size_is_a_power_of_two() {
    let page = host_page_size().expect("sysconf answers on the GNU Linux host");
    assert!(page.is_power_of_two() && page >= 4096, "{page}");
}
