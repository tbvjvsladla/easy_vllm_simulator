//! Which certificate authorities the release lookup trusts, decided like OpenSSL does: a CA *file* (`SSL_CERT_FILE`, else
//! the platform default file) and CA *directories* (`SSL_CERT_DIR`, else the platform default directories) are chosen
//! independently. A variable overrides only its own side; set-but-empty switches its side off; both set means only
//! those; neither means the platform's. Python's `ssl.get_default_verify_paths` behaves this way (reviewed on the
//! original: file-only and directory-only overrides each leave the other side at its default).
//!
//! A directory is OpenSSL's hash index, not a bundle: only `<subject hash>.<n>` entries whose certificate hashes to the name
//! are trust anchors (arbitrary names and wrong hashes are not), while a CA file is a PEM bundle of any name. The
//! directories of these checks therefore hold genuinely hashed entries, made with `openssl x509 -hash` at set-up, and
//! `directory_scope_follows_openssl_hashing` is the actual-TLS control for unhashed, wrongly hashed and matching entries.
//!
//! Two kinds of assertion, through the production selection and loading code (`select_trust_sources`,
//! `load_trust_roots`, the transport's `HttpEnvironment`):
//!
//! * the **selection** of the sources for every case (default, file only, directory only, both, empty), and
//! * an **actual TLS handshake** per case against a loopback server whose certificate is signed by a temporary CA. The
//!   environment and the "platform default" locations are injected as values (private temporary files and folders), so no
//!   real global store and no personal certificate is read, and nothing global changes: the CA that signed the server is
//!   reachable only from the side a case leaves at its default, or from the override that names it.
//!
//! The `openssl` command makes the temporary CAs (`NOT_RUN` if absent); the server is rustls.

#[path = "guidellm_release_tls_common.rs"]
mod common;

use common::*;
use std::ffi::OsString;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use sweep_stop::guidellm_release::{fetch_latest, ReleaseRequest, ReleaseTransport};
use sweep_stop::guidellm_release_http::{
    load_trust_roots, select_trust_sources, HttpEnvironment, HttpsTransport, PlatformTrust,
    TrustSources,
};
use sweep_stop::openssl_ca_directory::subject_name_hash;
use sweep_stop::pyvalue::PyValue;

fn environment(
    file: Option<&str>,
    directory: Option<&str>,
    platform: PlatformTrust,
) -> HttpEnvironment {
    HttpEnvironment {
        ssl_cert_file: file.map(OsString::from),
        ssl_cert_dir: directory.map(OsString::from),
        platform,
        ..HttpEnvironment::default()
    }
}

fn path(value: &Path) -> &str {
    value.to_str().expect("UTF-8 scratch path")
}

/// A CA directory in the form OpenSSL reads: a copy of `ca_pem` named `<subject hash>.0`, the hash made by `openssl x509
/// -hash` at set-up (a *file* of any name would not be a trust anchor in a directory; see
/// `directory_scope_follows_openssl_hashing`).
fn directory_with(folder: &Path, name: &str, ca_pem: &Path) -> PathBuf {
    let directory = folder.join(name);
    std::fs::create_dir_all(&directory).expect("directory");
    let entry = format!("{}.0", openssl_subject_hash(ca_pem));
    std::fs::copy(ca_pem, directory.join(entry)).expect("copy the CA under its hash");
    directory
}

#[test]
fn the_two_sources_are_selected_independently() {
    let platform = PlatformTrust {
        file: Some(PathBuf::from("/platform/file.pem")),
        directories: vec![
            PathBuf::from("/platform/one"),
            PathBuf::from("/platform/two"),
        ],
    };
    let sources = |file: Option<&str>, directory: Option<&str>| {
        select_trust_sources(&environment(file, directory, platform.clone()))
    };
    let default = sources(None, None);
    assert_eq!(default.file, platform.file);
    assert_eq!(default.directories, platform.directories);
    // a file override keeps the platform directories
    let file_only = sources(Some("/override/file.pem"), None);
    assert_eq!(file_only.file, Some(PathBuf::from("/override/file.pem")));
    assert_eq!(file_only.directories, platform.directories);
    // a directory override (a `:` list) keeps the platform file
    let directory_only = sources(None, Some("/override/a::/override/b"));
    assert_eq!(directory_only.file, platform.file);
    assert_eq!(
        directory_only.directories,
        vec![PathBuf::from("/override/a"), PathBuf::from("/override/b")]
    );
    // both: only the overrides
    let both = sources(Some("/override/file.pem"), Some("/override/a"));
    assert_eq!(both.file, Some(PathBuf::from("/override/file.pem")));
    assert_eq!(both.directories, vec![PathBuf::from("/override/a")]);
    // empty means "none on that side", not "the default"
    let empty_file = sources(Some(""), None);
    assert_eq!(empty_file.file, None);
    assert_eq!(empty_file.directories, platform.directories);
    let empty_directory = sources(None, Some(""));
    assert_eq!(empty_directory.file, platform.file);
    assert!(empty_directory.directories.is_empty());
    let both_empty = sources(Some(""), Some(""));
    assert_eq!(both_empty, TrustSources::default());
}

#[test]
fn only_readable_certificate_locations_contribute_roots() {
    let folder = Scratch::new("roots");
    let first = authority(&folder.0.join("ca"), "roots");
    let second = authority(&folder.0.join("second"), "second");
    let directory = directory_with(&folder.0, "dir", &second.ca_pem);
    let count = |sources: TrustSources| load_trust_roots(&sources).len();
    let nothing = TrustSources::default();
    assert_eq!(count(nothing), 0);
    assert_eq!(
        count(TrustSources {
            file: Some(first.ca_pem.clone()),
            directories: vec![]
        }),
        1
    );
    assert_eq!(
        count(TrustSources {
            file: None,
            directories: vec![directory.clone()]
        }),
        1
    );
    // file and directory together (two different CAs); a missing file or directory adds nothing and breaks nothing
    assert_eq!(
        count(TrustSources {
            file: Some(first.ca_pem.clone()),
            directories: vec![directory, folder.0.join("absent")],
        }),
        2
    );
    assert_eq!(
        count(TrustSources {
            file: Some(folder.0.join("absent.pem")),
            directories: vec![]
        }),
        0
    );
}

/// One lookup through the production transport against a server signed by `server`'s CA, with the environment given as
/// a value. `true` when the release was fetched, `false` when the lookup failed as `URLError`.
fn lookup(server: &Authority, mut environment: HttpEnvironment) -> bool {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    let reply = Reply::Complete {
        status_line: "200 OK",
        body: b"{\"tag_name\":\"v0.9.1\"}".to_vec(),
    };
    let handle = serve_once(listener, server_config(server), reply);
    environment.https_proxy = Some(format!("http://127.0.0.1:{port}"));
    let transport = HttpsTransport::with_environment(environment);
    let outcome = fetch_latest(&PyValue::str("vllm-project/guidellm"), &transport);
    let served = handle.join().expect("server thread");
    match outcome {
        Ok(_) => {
            assert!(served.is_ok(), "a fetched release was served: {served:?}");
            true
        }
        Err(failure) => {
            assert_eq!(failure.class, "URLError", "{failure:?}");
            assert!(
                served.is_err(),
                "an untrusted peer must never be sent the request"
            );
            false
        }
    }
}

#[test]
fn an_actual_handshake_tells_every_source_case_apart() {
    let folder = Scratch::new("handshake");
    let real = authority(&folder.0.join("real"), "real");
    let other = authority(&folder.0.join("other"), "other");
    let real_dir = directory_with(&folder.0, "real-dir", &real.ca_pem);
    let other_dir = directory_with(&folder.0, "other-dir", &other.ca_pem);
    let defaults_real = || PlatformTrust {
        file: Some(real.ca_pem.clone()),
        directories: vec![real_dir.clone()],
    };
    let file_default_real = || PlatformTrust {
        file: Some(real.ca_pem.clone()),
        directories: vec![],
    };
    let dir_default_real = || PlatformTrust {
        file: None,
        directories: vec![real_dir.clone()],
    };
    let nothing = PlatformTrust::default;

    // default: the host store alone decides
    assert!(
        lookup(&real, environment(None, None, file_default_real())),
        "default file"
    );
    assert!(
        lookup(&real, environment(None, None, dir_default_real())),
        "default directory"
    );
    assert!(
        !lookup(&real, environment(None, None, nothing())),
        "empty host store trusts nothing"
    );
    // file-only override: it replaces the file side and the default directory stays trusted
    assert!(
        lookup(
            &real,
            environment(Some(path(&other.ca_pem)), None, dir_default_real())
        ),
        "file override must keep the default directory"
    );
    assert!(
        !lookup(
            &real,
            environment(Some(path(&other.ca_pem)), None, file_default_real())
        ),
        "file override replaces the default file"
    );
    // directory-only override: it replaces the directory side and the default file stays trusted
    assert!(
        lookup(
            &real,
            environment(None, Some(path(&other_dir)), file_default_real())
        ),
        "directory override must keep the default file"
    );
    assert!(
        !lookup(
            &real,
            environment(None, Some(path(&other_dir)), dir_default_real())
        ),
        "directory override replaces the default directory"
    );
    // both overrides: only they are trusted, whatever the host store holds
    assert!(
        !lookup(
            &real,
            environment(
                Some(path(&other.ca_pem)),
                Some(path(&other_dir)),
                defaults_real()
            )
        ),
        "both overrides name another CA: the host store must not be added"
    );
    assert!(lookup(
        &real,
        environment(Some(path(&real.ca_pem)), Some(path(&other_dir)), nothing())
    ));
    assert!(lookup(
        &real,
        environment(Some(path(&other.ca_pem)), Some(path(&real_dir)), nothing())
    ));
    // set but empty: that side is off and its default does not come back
    assert!(
        !lookup(&real, environment(Some(""), None, file_default_real())),
        "an empty file override must not re-enable the default file"
    );
    assert!(
        !lookup(&real, environment(None, Some(""), dir_default_real())),
        "an empty directory override must not re-enable the default directories"
    );
    assert!(
        lookup(&real, environment(Some(""), None, dir_default_real())),
        "an empty file override leaves the directory side at its default"
    );
    assert!(
        lookup(&real, environment(None, Some(""), file_default_real())),
        "an empty directory override leaves the file side at its default"
    );
    assert!(
        !lookup(&real, environment(Some(""), Some(""), defaults_real())),
        "both empty: nothing is trusted"
    );
}

#[test]
fn the_transport_is_usable_as_a_release_transport_object() {
    // compile-time shape check: the production transport is a `ReleaseTransport` built from an injected environment
    fn takes(_: &dyn ReleaseTransport, _: &ReleaseRequest) {}
    let transport = HttpsTransport::with_environment(HttpEnvironment::default());
    let request =
        sweep_stop::guidellm_release::latest_request(&PyValue::str("a/b")).expect("request");
    takes(&transport, &request);
}

fn der_of(pem: &Path) -> Vec<u8> {
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    CertificateDer::from_pem_file(pem)
        .expect("certificate")
        .as_ref()
        .to_vec()
}

/// The production subject hash agrees with `openssl x509 -hash` over many shapes of subject name: plain, mixed case, outer
/// and repeated inner spaces, several RDNs and string types (printable and UTF-8), non-ASCII text, a repeated attribute, a
/// multi-valued RDN, a long value and a domain-component chain.
#[test]
fn the_subject_hash_is_openssl_compatible_over_many_name_shapes() {
    let folder = Scratch::new("hashes");
    let shapes: &[(&str, &str)] = &[
        ("plain", "/CN=Plain"),
        ("mixed", "/CN=Mixed Case CA"),
        ("spaced", "/CN=  Spaced    Out  "),
        (
            "rdns",
            "/C=US/ST=New York/L=Albany/O=Example Corp/OU=Unit/CN=Multi RDN",
        ),
        ("korean", "/C=KR/O=한글 기관/CN=한글 CA"),
        ("accent", "/O=Société Générale/CN=ÉCOLE Root"),
        ("repeat", "/CN=a/CN=b"),
        ("multivalue", "/CN=x+OU=y/O=z"),
        (
            "long",
            "/CN=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        ("dc", "/DC=com/DC=example/CN=dc root"),
        ("tab", "/CN=tab\tseparated"),
    ];
    for (name, subject) in shapes {
        let pem = self_signed(&folder.0, name, subject);
        let expected = openssl_subject_hash(&pem);
        let ours = subject_name_hash(&der_of(&pem)).map(|hash| format!("{hash:08x}"));
        assert_eq!(
            ours.as_deref(),
            Some(expected.as_str()),
            "{name}: {subject}"
        );
    }
}

/// The directory rule, through the same production loading and the same HTTPS boundary as the other cases. The file
/// is named by `SSL_CERT_FILE=""` (explicitly empty) so nothing else can trust the server's CA.
#[test]
fn directory_scope_follows_openssl_hashing() {
    let folder = Scratch::new("directory-scope");
    let real = authority(&folder.0.join("real"), "real");
    let hash = openssl_subject_hash(&real.ca_pem);
    assert_ne!(
        hash, "00000000",
        "the wrong-hash control needs a different hash"
    );
    let in_directory = |name: &str, entries: &[&str]| -> PathBuf {
        let directory = folder.0.join(name);
        std::fs::create_dir_all(&directory).expect("directory");
        for entry in entries {
            std::fs::copy(&real.ca_pem, directory.join(entry)).expect("copy");
        }
        directory
    };
    let verdict = |directory: &Path| {
        lookup(
            &real,
            environment(Some(""), Some(path(directory)), PlatformTrust::default()),
        )
    };
    // negative controls: the genuine CA PEM under an arbitrary name, under a valid but wrong hash, with an upper-case
    // hash (OpenSSL looks for lower case), with a bad index, and as a plain `.pem` next to nothing
    assert!(
        !verdict(&in_directory("unhashed", &["arbitrary-name.pem"])),
        "arbitrary name"
    );
    assert!(
        !verdict(&in_directory("wrong-hash", &["00000000.0"])),
        "valid pattern, wrong hash"
    );
    assert!(
        !verdict(&in_directory(
            "upper",
            &[&format!("{}.0", hash.to_uppercase())]
        )),
        "upper-case hash"
    );
    assert!(
        !verdict(&in_directory("crl-name", &[&format!("{hash}.r0")])),
        "CRL entry name"
    );
    assert!(!verdict(&in_directory("no-index", &[&hash])), "no index");
    // positive controls: the matching hash (any index), and the symbolic link `openssl rehash` makes
    assert!(
        verdict(&in_directory("matching", &[&format!("{hash}.0")])),
        "matching hash"
    );
    assert!(
        verdict(&in_directory("matching-second", &[&format!("{hash}.7")])),
        "matching hash, index 7"
    );
    assert!(
        verdict(&in_directory(
            "mixed",
            &["arbitrary-name.pem", "00000000.0", &format!("{hash}.0")]
        )),
        "one right entry among wrong ones"
    );
    let linked = folder.0.join("linked");
    std::fs::create_dir_all(&linked).expect("directory");
    std::os::unix::fs::symlink(&real.ca_pem, linked.join(format!("{hash}.0"))).expect("symlink");
    assert!(verdict(&linked), "rehash-style symbolic link");
    // the CA *file* is a different thing: a bundle whose ordinary name is valid, with the directory side empty
    let arbitrary_file = in_directory("file-holder", &["any-name.pem"]).join("any-name.pem");
    assert!(
        lookup(
            &real,
            environment(
                Some(path(&arbitrary_file)),
                Some(""),
                PlatformTrust::default()
            )
        ),
        "a CA file of any name is a bundle"
    );
    // ...and the same arbitrary-named file is not trusted when it is only listed as a directory entry
    assert!(!verdict(arbitrary_file.parent().expect("parent")));
}

/// Optional replay of three externally recorded certificate-directory controls (an unhashed entry, a wrongly hashed entry
/// and a matching entry, recorded by the original through OpenSSL): `GUIDELLM_CA_CONTROL_UNHASHED`, `_WRONG` and `_MATCHING`
/// name three directories. Ignored by default because the paths are the caller's; run with `--ignored` and the three
/// variables. It checks the production loader and hash on those exact files (the TLS verdict needs the recorded server key,
/// which is not part of the controls).
#[test]
#[ignore = "needs GUIDELLM_CA_CONTROL_UNHASHED, GUIDELLM_CA_CONTROL_WRONG and GUIDELLM_CA_CONTROL_MATCHING"]
fn externally_recorded_directory_controls_are_judged_like_the_original() {
    let directory = |name: &str| -> PathBuf {
        PathBuf::from(
            std::env::var_os(name).unwrap_or_else(|| panic!("NOT_RUN: {name} is not set")),
        )
    };
    let roots = |name: &str| {
        load_trust_roots(&TrustSources {
            file: None,
            directories: vec![directory(name)],
        })
        .len()
    };
    assert_eq!(
        roots("GUIDELLM_CA_CONTROL_UNHASHED"),
        0,
        "an arbitrary name is no trust anchor"
    );
    assert_eq!(
        roots("GUIDELLM_CA_CONTROL_WRONG"),
        0,
        "a wrong hash is no trust anchor"
    );
    assert_eq!(
        roots("GUIDELLM_CA_CONTROL_MATCHING"),
        1,
        "the matching hash is a trust anchor"
    );
}
