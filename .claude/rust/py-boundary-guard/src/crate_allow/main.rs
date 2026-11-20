//! crate_allow — the crates.io dependency gate of the harness (plan 26111805 D2·D9, cell c20).
//!
//! Every external package pinned by a tracked `Cargo.lock` must be
//!   1. from the crates.io index (no git or other registry source),
//!   2. listed in `crate_allowlist.tsv` with the same version (the human-approved list; approvals are recorded
//!      outside the repository, keyed `crate@version`),
//!   3. under a permitted license: the allowlist's SPDX expression is permitted (`license::PERMITTED_LICENSES`) and
//!      equals the `license` field of the downloaded package manifest in the node's Cargo home,
//!   4. named in the crate notice `notices/NOTICE-crates.md` with the same version and license.
//!
//! An allowlist row that no lock pins yet (a release approved ahead of its first use) passes 3 on the expression
//! alone when its source is not downloaded; once downloaded, its manifest must agree.
//!
//! A source-less (path) package is exempt from 1–4 only when it is a harness package: one of the 16 entries of the
//! closed list `harness::HARNESS_CRATES` whose tracked `Cargo.toml` declares exactly that name and version. Any other
//! is `path-unknown` (a vendored copy, a 17th crate with or without its own lock, a version moved without the list).
//!
//! Only the `crate`, `version` and `license` columns of the allowlist and of the notice are read. The other columns
//! (`transitive`, `scope`, `origin`, `locks`, `use`, `reason`; the notice's `used by`) are values written when the
//! rows were approved (h9) for the human reader; they are not kept up to date and nothing here compares them.
//!
//! Usage:
//!   crate_allow check [--repo DIR] [--allowlist FILE] [--notice FILE] [--cargo-home DIR]
//!
//! Defaults: allowlist `.claude/rust/py-boundary-guard/crate_allowlist.tsv`, notice
//! `.claude/rust/py-boundary-guard/notices/NOTICE-crates.md`, Cargo home `.claude/runtime/rust/cargo` (the
//! repository-local home of `rust_bootstrap.sh`), all under `--repo` (default `.`).
//!
//! Output: one `DEPS-VIOLATION <kind> <crate>@<version> <detail>` line per finding on stdout, then a summary line;
//! on a pass the last line is `DEPS-ALLOW-PASS`.
//! exit: 0 pass · 1 violations · 2 usage or input error (an unreadable or malformed table, lock or harness manifest,
//! a lock table header Cargo does not write, no tracked lock at all, a tracked lock or manifest path or an argument
//! that is not UTF-8: fail closed).

mod harness;
mod license;
mod lockfile;
mod registry;
mod tables;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use registry::DeclaredLicense;
use tables::{AllowedCrate, NoticeEntry};

const DEFAULT_ALLOWLIST: &str = ".claude/rust/py-boundary-guard/crate_allowlist.tsv";
const DEFAULT_NOTICE: &str = ".claude/rust/py-boundary-guard/notices/NOTICE-crates.md";
const DEFAULT_CARGO_HOME: &str = ".claude/runtime/rust/cargo";
const PASS_MARKER: &str = "DEPS-ALLOW-PASS";

fn usage() -> &'static str {
    "Usage: crate_allow check [--repo DIR] [--allowlist FILE] [--notice FILE] [--cargo-home DIR]\n\
     \x20 every external package of every tracked Cargo.lock: crates.io source, listed in --allowlist with the same\n\
     \x20 version, permitted license equal to the downloaded manifest's (in --cargo-home), named in --notice.\n\
     \x20 defaults: .claude/rust/py-boundary-guard/crate_allowlist.tsv, .claude/rust/py-boundary-guard/notices/\n\
     \x20 NOTICE-crates.md, .claude/runtime/rust/cargo (under --repo, default .)"
}

struct Options {
    repo: PathBuf,
    allowlist: Option<PathBuf>,
    notice: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
}

/// One external release and the tracked locks that pin it.
struct PinnedRelease {
    sources: BTreeSet<String>,
    from_crates_io: bool,
    locks: Vec<String>,
}

/// What the tracked locks pin: external releases, and source-less packages with the locks that name them.
struct Pinned {
    external: BTreeMap<(String, String), PinnedRelease>,
    path: BTreeMap<(String, String), Vec<String>>,
}

/// Every tracked path, as git prints it with `-z` (raw bytes).
fn tracked_files(repo: &Path) -> Result<Vec<Vec<u8>>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(<[u8]>::to_vec)
        .collect())
}

/// The tracked paths named `file_name` (at the top or in any directory), sorted. Such a path is printed on finding
/// lines, so one that is not UTF-8 is an input error rather than a lossy name.
fn named(tracked: &[Vec<u8>], file_name: &str) -> Result<Vec<String>, String> {
    let suffix = format!("/{file_name}");
    let mut found = Vec::new();
    for raw in tracked {
        if raw.as_slice() != file_name.as_bytes() && !raw.ends_with(suffix.as_bytes()) {
            continue;
        }
        let path = String::from_utf8(raw.clone()).map_err(|_| {
            format!(
                "a tracked {file_name} path is not UTF-8: {}",
                String::from_utf8_lossy(raw)
            )
        })?;
        found.push(path);
    }
    found.sort();
    Ok(found)
}

fn read(path: &Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {what} {}: {e}", path.display()))
}

fn pinned_releases(repo: &Path, locks: &[String]) -> Result<Pinned, String> {
    let mut pinned = Pinned {
        external: BTreeMap::new(),
        path: BTreeMap::new(),
    };
    for lock in locks {
        let packages = lockfile::parse(&read(&repo.join(lock), "lock")?)
            .map_err(|e| format!("{lock}: {e}"))?;
        for pkg in packages {
            if !pkg.is_external() {
                let named_by = pinned.path.entry((pkg.name, pkg.version)).or_default();
                if !named_by.contains(lock) {
                    named_by.push(lock.clone());
                }
                continue;
            }
            let from_crates_io = pkg.is_from_crates_io();
            let entry = pinned
                .external
                .entry((pkg.name, pkg.version))
                .or_insert_with(|| PinnedRelease {
                    sources: BTreeSet::new(),
                    from_crates_io: true,
                    locks: Vec::new(),
                });
            entry.from_crates_io &= from_crates_io;
            entry.sources.extend(pkg.source);
            if !entry.locks.contains(lock) {
                entry.locks.push(lock.clone());
            }
        }
    }
    Ok(pinned)
}

fn where_pinned(locks: &[String]) -> String {
    match locks {
        [only] => format!("pinned by {only}"),
        [first, rest @ ..] => format!("pinned by {first} and {} more lock(s)", rest.len()),
        [] => "not pinned".to_string(),
    }
}

/// Findings for the allowlist rows themselves: a malformed or non-permitted expression, and disagreement with a
/// downloaded manifest. `declared` caches the manifest lookups for the pinned-release pass.
fn check_rows(
    rows: &[AllowedCrate],
    cargo_home: &Path,
    declared: &mut BTreeMap<(String, String), DeclaredLicense>,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for row in rows {
        let id = format!("{}@{}", row.name, row.version);
        match license::is_permitted(&row.license) {
            Ok(true) => {}
            Ok(false) => violations.push(format!(
                "DEPS-VIOLATION license-denied {id} allowlist line {}: '{}' needs a license outside the permitted set",
                row.line, row.license
            )),
            Err(e) => violations.push(format!(
                "DEPS-VIOLATION license-invalid {id} allowlist line {}: '{}': {e}",
                row.line, row.license
            )),
        }
        let found = registry::declared_license(cargo_home, &row.name, &row.version)?;
        match &found {
            DeclaredLicense::Expression(expr) if *expr != row.license => violations.push(format!(
                "DEPS-VIOLATION license-mismatch {id} allowlist line {} says '{}', the downloaded manifest says '{expr}'",
                row.line, row.license
            )),
            DeclaredLicense::NotDeclared => violations.push(format!(
                "DEPS-VIOLATION license-undeclared {id} the downloaded manifest has no license field"
            )),
            _ => {}
        }
        declared.insert((row.name.clone(), row.version.clone()), found);
    }
    Ok(())
}

fn check_pinned(
    pinned: &BTreeMap<(String, String), PinnedRelease>,
    rows: &[AllowedCrate],
    notice: &[NoticeEntry],
    cargo_home: &Path,
    declared: &BTreeMap<(String, String), DeclaredLicense>,
    violations: &mut Vec<String>,
) {
    for ((name, version), release) in pinned {
        let id = format!("{name}@{version}");
        let at = where_pinned(&release.locks);
        if !release.from_crates_io {
            let sources: Vec<&str> = release.sources.iter().map(String::as_str).collect();
            violations.push(format!(
                "DEPS-VIOLATION source {id} {at}: not the crates.io index ({})",
                sources.join(", ")
            ));
        }
        let Some(row) = rows
            .iter()
            .find(|r| r.name == *name && r.version == *version)
        else {
            let listed: Vec<&str> = rows
                .iter()
                .filter(|r| r.name == *name)
                .map(|r| r.version.as_str())
                .collect();
            let has = if listed.is_empty() {
                "no row".to_string()
            } else {
                format!("only {}", listed.join(", "))
            };
            violations.push(format!(
                "DEPS-VIOLATION unlisted {id} {at}; the allowlist has {has} for {name}"
            ));
            continue;
        };
        // Every allowlist row was looked up by `check_rows`; a release without a lookup is treated as not downloaded.
        if matches!(
            declared.get(&(name.clone(), version.clone())),
            None | Some(DeclaredLicense::SourceMissing)
        ) {
            violations.push(format!(
                "DEPS-VIOLATION no-source {id} {at}: the license cannot be checked, {name}-{version} is not unpacked \
                 under {}/registry/src (build the crate offline once, or fetch it)",
                cargo_home.display()
            ));
        }
        match notice
            .iter()
            .find(|n| n.name == *name && n.version == *version)
        {
            None => violations.push(format!(
                "DEPS-VIOLATION no-notice {id} {at}: no row in the crate notice"
            )),
            Some(n) if n.license != row.license => violations.push(format!(
                "DEPS-VIOLATION notice-license {id} the notice says '{}', the allowlist '{}'",
                n.license, row.license
            )),
            Some(_) => {}
        }
    }
}

/// Source-less packages that are not harness packages (see the module notes).
fn check_path_packages(
    path: &BTreeMap<(String, String), Vec<String>>,
    harness: &BTreeSet<(String, String)>,
    violations: &mut Vec<String>,
) {
    for ((name, version), locks) in path {
        if !harness.contains(&(name.clone(), version.clone())) {
            violations.push(format!(
                "DEPS-VIOLATION path-unknown {name}@{version} {}: a package without source that is not one of the {} \
                 harness crates (harness.rs HARNESS_CRATES, as their tracked Cargo.toml declare them)",
                where_pinned(locks),
                harness::HARNESS_CRATES.len()
            ));
        }
    }
}

fn run(opts: &Options) -> Result<bool, String> {
    let repo = &opts.repo;
    let allowlist_path = opts
        .allowlist
        .clone()
        .unwrap_or_else(|| repo.join(DEFAULT_ALLOWLIST));
    let notice_path = opts
        .notice
        .clone()
        .unwrap_or_else(|| repo.join(DEFAULT_NOTICE));
    let cargo_home = opts
        .cargo_home
        .clone()
        .unwrap_or_else(|| repo.join(DEFAULT_CARGO_HOME));
    let rows = tables::parse_allowlist(&read(&allowlist_path, "allowlist")?)
        .map_err(|e| format!("{}: {e}", allowlist_path.display()))?;
    let notice = tables::parse_notice(&read(&notice_path, "notice")?)
        .map_err(|e| format!("{}: {e}", notice_path.display()))?;
    let tracked = tracked_files(repo)?;
    let locks = named(&tracked, "Cargo.lock")?;
    if locks.is_empty() {
        return Err(format!(
            "no tracked Cargo.lock under {} (nothing to check is not a pass)",
            repo.display()
        ));
    }
    let manifests = named(&tracked, "Cargo.toml")?;
    let harness = harness::packages(repo, &manifests)?;
    let pinned = pinned_releases(repo, &locks)?;

    let mut violations = Vec::new();
    let mut declared = BTreeMap::new();
    check_rows(&rows, &cargo_home, &mut declared, &mut violations)?;
    check_path_packages(&pinned.path, &harness, &mut violations);
    check_pinned(
        &pinned.external,
        &rows,
        &notice,
        &cargo_home,
        &declared,
        &mut violations,
    );

    let unpinned_rows = rows
        .iter()
        .filter(|r| {
            !pinned
                .external
                .contains_key(&(r.name.clone(), r.version.clone()))
        })
        .count();
    for v in &violations {
        println!("{v}");
    }
    println!(
        "[crate_allow] {} tracked Cargo.lock · {} external releases pinned · {} path packages, {} harness manifests · \
         allowlist {} rows ({unpinned_rows} not pinned by any lock) · notice {} rows · permitted licenses: {}",
        locks.len(),
        pinned.external.len(),
        pinned.path.len(),
        harness.len(),
        rows.len(),
        notice.len(),
        license::PERMITTED_LICENSES.join(", ")
    );
    if violations.is_empty() {
        println!("{PASS_MARKER}");
        return Ok(true);
    }
    eprintln!("[crate_allow] FAIL {} violation(s)", violations.len());
    Ok(false)
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        repo: PathBuf::from("."),
        allowlist: None,
        notice: None,
        cargo_home: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = |what: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs {what}"))
        };
        match flag.as_str() {
            "--repo" => opts.repo = PathBuf::from(value("DIR")?),
            "--allowlist" => opts.allowlist = Some(PathBuf::from(value("FILE")?)),
            "--notice" => opts.notice = Some(PathBuf::from(value("FILE")?)),
            "--cargo-home" => opts.cargo_home = Some(PathBuf::from(value("DIR")?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(opts)
}

fn main() -> ExitCode {
    // Arguments come in as OS strings: a path that is not UTF-8 would be printed lossily on L1 finding lines, so it is
    // a usage error (exit 2) instead of a panic or a U+FFFD path.
    let mut args: Vec<String> = Vec::new();
    for raw in std::env::args_os().skip(1) {
        match raw.into_string() {
            Ok(arg) => args.push(arg),
            Err(raw) => {
                eprintln!(
                    "[crate_allow] an argument is not UTF-8: {}\n{}",
                    raw.to_string_lossy(),
                    usage()
                );
                return ExitCode::from(2);
            }
        }
    }
    match args.first().map(String::as_str) {
        Some("check") => {}
        Some("-h" | "--help") => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        other => {
            let what = other.map_or("missing command".to_string(), |o| {
                format!("unknown command: {o}")
            });
            eprintln!("[crate_allow] {what}\n{}", usage());
            return ExitCode::from(2);
        }
    }
    let opts = match parse_options(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[crate_allow] {e}\n{}", usage());
            return ExitCode::from(2);
        }
    };
    match run(&opts) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("[crate_allow] ERROR {e}");
            ExitCode::from(2)
        }
    }
}
