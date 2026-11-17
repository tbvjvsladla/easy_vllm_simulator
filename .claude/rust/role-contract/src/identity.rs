//! The node-identity resolver of `node_identity.sh --resolve --repo REPO` (no `--node-id`): who is the node a repository
//! sits on, read from the `output/*/manifest.yaml` files under it.
//!
//! This is a byte-oriented port of the original's resolver chain `ni_resolve_node_id` -> `ni_manifest_self_role` ->
//! `ni_manifest_main_role` -> the fail-loud end, including what the Bash/GNU tools around it did to the bytes: the
//! `grep -E '^self_role:...' | head -1 | sed -E ...` pipeline, the `grep -Ec` count of `- role: main`, the open slug
//! `^[a-z][a-z0-9-]{0,31}$` (NOT the closed role enum), the glob order and `-d`/`-f`/`-r` tests of the file system, and the
//! repository spelling in every message. No external program runs and nothing is read through Python or Bash.
//!
//! Two fixed locale profiles are modeled, the ones the saved observations of the original exist for:
//! * [`Profile::C`]: `LC_ALL=C` - bytes are characters, `[[:space:]]` is the six ASCII blanks, no encoding errors;
//! * [`Profile::CUtf8`]: `LC_ALL=C.UTF-8` - UTF-8 characters, `[[:space:]]` is the `space` class of the glibc 2.39
//!   `i18n_ctype` locale source: a constant compiled into `space_len` (taken from that source file when this was written;
//!   the host's locale files are not read at run time; U+00A0, U+2007 and U+202F are *not* in it), and a selected line
//!   that is not valid UTF-8 is withheld by GNU grep 3.11 with `grep: FILE: binary file matches` on stderr.
//!
//! What grep prints and when it adds that notice (a NUL byte makes the buffers it appears in "binary", decided per read
//! buffer; a selected line with an encoding error is withheld while the scan goes on; `-c` counts NUL as a line end) is
//! modeled by the `identity_scan` module from the grep 3.11 source, including the page-size dependent read buffers and
//! their growth. Where upstream grep's own output depends on the address of its buffer (genuine upstream nondeterminism)
//! the answer is one member of the set GNU permits, from real native allocations (see `identity_scan`); the business
//! status is always 0 (resolved) or 1 (fail-loud), exactly the domain of the original wrapper. The only other status of
//! this library is 2, used when the platform cannot say its page size (grep 3.11 itself aborts then): it is a platform
//! limit, never an answer of the original.
//!
//! Not ported (not part of the call the callers make): the explicit `--node-id` override, the standalone `source`
//! functions and `--self-test`. A falsy repository never gets here: the caller returns before the resolver.

use crate::MAIN_ROLE;
#[path = "identity_scan.rs"]
mod scan;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The fixed locale profile the original ran under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    C,
    CUtf8,
}

impl Profile {
    /// The profile the `LC_ALL` / `LC_CTYPE` / `LANG` precedence of the environment names: `C`, `POSIX` or nothing is
    /// [`Profile::C`]; a name that selects a UTF-8 charset is [`Profile::CUtf8`] (the nearest modeled profile - only the
    /// two fixed ones are verified byte for byte).
    pub fn from_locale_variables(
        lc_all: Option<&[u8]>,
        lc_ctype: Option<&[u8]>,
        lang: Option<&[u8]>,
    ) -> Profile {
        let pick = [lc_all, lc_ctype, lang]
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty())
            .unwrap_or(b"C");
        let lower: Vec<u8> = pick.iter().map(u8::to_ascii_lowercase).collect();
        let utf8 =
            lower.windows(5).any(|w| w == b".utf8") || lower.windows(6).any(|w| w == b".utf-8");
        if utf8 {
            Profile::CUtf8
        } else {
            Profile::C
        }
    }
}

/// The slug scheme of the original (`NI_SLUG_RE`), as shown in the messages.
pub const SLUG_RE: &str = "^[a-z][a-z0-9-]{0,31}$";

/// What `bash node_identity.sh --resolve --repo REPO` leaves: the exit status and the raw byte streams.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// `[[:space:]]` of the profile: the byte length of the blank character at `at`, if there is one.
fn space_len(profile: Profile, bytes: &[u8], at: usize) -> Option<usize> {
    let first = *bytes.get(at)?;
    match first {
        b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r' => Some(1),
        0x00..=0x7F => None,
        _ if profile == Profile::C => None,
        _ => {
            let width = match first {
                0xC2..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF4 => 4,
                _ => return None,
            };
            let end = at + width;
            let text = std::str::from_utf8(bytes.get(at..end)?).ok()?;
            let point = text.chars().next()? as u32;
            let blank = matches!(point, 0x1680 | 0x2000..=0x2006 | 0x2008..=0x200A | 0x2028 | 0x2029 | 0x205F | 0x3000);
            blank.then_some(width)
        }
    }
}

/// The end of the run of blanks starting at `at`.
fn skip_spaces(profile: Profile, bytes: &[u8], mut at: usize) -> usize {
    while let Some(width) = space_len(profile, bytes, at) {
        at += width;
    }
    at
}

/// The lines of a file as grep sees them: split at `\n` (and at NUL when the file is binary); a final unterminated line
/// counts, an empty file has none.
fn grep_lines(bytes: &[u8], binary: bool) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' || (binary && *byte == 0) {
            lines.push(&bytes[start..index]);
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

fn push_line(out: &mut Vec<u8>, parts: &[&[u8]]) {
    for part in parts {
        out.extend_from_slice(part);
    }
    out.push(b'\n');
}

/// The path text of a manifest, `"$repo"/output/NAME/manifest.yaml`, with the repository spelled as given.
fn manifest_text(repo: &[u8], name: &[u8]) -> Vec<u8> {
    let mut path = repo.to_vec();
    path.extend_from_slice(b"/output/");
    path.extend_from_slice(name);
    path.extend_from_slice(b"/manifest.yaml");
    path
}

fn os_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// `for mf in "$repo"/output/*/manifest.yaml; do [ -f "$mf" ] || continue`: the regular files (symlinks followed) among
/// the matches of the glob, in byte order, dot-names skipped. A pattern that matches nothing stays literal in Bash and
/// fails `-f`, so "no match" is simply an empty list.
fn manifests(repo: &[u8]) -> Vec<Vec<u8>> {
    let output = {
        let mut path = repo.to_vec();
        path.extend_from_slice(b"/output");
        os_path(&path)
    };
    let Ok(entries) = fs::read_dir(&output) else {
        return Vec::new();
    };
    let mut names: Vec<Vec<u8>> = entries
        .flatten()
        .map(|entry| entry.file_name().as_bytes().to_vec())
        .filter(|name| !name.starts_with(b"."))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| manifest_text(repo, &name))
        .filter(|path| {
            fs::metadata(os_path(path))
                .map(|meta| meta.is_file())
                .unwrap_or(false)
        })
        .collect()
}

/// `[ -r "$mf" ]` and the read: the bytes, or `None` when the file cannot be opened for reading.
fn read_manifest(path: &[u8]) -> Option<Result<Vec<u8>, std::io::Error>> {
    let mut file = fs::File::open(Path::new(std::ffi::OsStr::from_bytes(path))).ok()?;
    let mut bytes = Vec::new();
    Some(file.read_to_end(&mut bytes).map(|_| bytes))
}

fn unreadable(stderr: &mut Vec<u8>, mf: &[u8]) {
    push_line(
        stderr,
        &[
            "[node-identity] FAIL: ".as_bytes(),
            mf,
            " 를 읽을 수 없다(권한/소유권) — 정체성 판정을 추측으로 잇지 않는다.".as_bytes(),
        ],
    );
}

/// `ni_valid_slug` on a single-line value.
pub fn valid_slug(value: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value[0].is_ascii_lowercase()
        && value[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// `sed -E 's/^self_role:[[:space:]]*//; s/[[:space:]]*#.*$//; s/^["'\'']//; s/["'\'']$//'` on a line already known to start
/// with `self_role:`.
fn self_role_value(profile: Profile, line: &[u8]) -> Vec<u8> {
    let mut at = b"self_role:".len();
    at = skip_spaces(profile, line, at);
    let rest = &line[at..];
    // `[[:space:]]*#.*$`: the leftmost `#` and the blanks right before it go with everything after.
    let mut cut = rest.len();
    if let Some(hash) = rest.iter().position(|b| *b == b'#') {
        let mut start = hash;
        // the blank run directly before the `#` (the leftmost start from which only blanks lead to it)
        let mut index = 0;
        let mut run_start = None;
        while index < hash {
            match space_len(profile, rest, index) {
                Some(width) => {
                    run_start.get_or_insert(index);
                    index += width;
                }
                None => {
                    run_start = None;
                    // step over the whole character so a multi-byte one is not cut in the middle
                    index += utf8_step(profile, rest, index);
                }
            }
        }
        if let Some(run) = run_start {
            if index == hash {
                start = run;
            }
        }
        cut = start;
    }
    let mut value = rest[..cut].to_vec();
    if matches!(value.first(), Some(b'"' | b'\'')) {
        value.remove(0);
    }
    if matches!(value.last(), Some(b'"' | b'\'')) {
        value.pop();
    }
    value
}

fn utf8_step(profile: Profile, bytes: &[u8], at: usize) -> usize {
    if profile == Profile::C {
        return 1;
    }
    let width = match bytes[at] {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 1,
    };
    match bytes.get(at..at + width).map(std::str::from_utf8) {
        Some(Ok(_)) => width,
        _ => 1,
    }
}

/// `grep -E '^self_role:...' MF | head -1 | sed ...`: the value of the first `self_role:` line grep prints (empty when
/// there is none), and what grep said on stderr. Which lines grep prints is `identity_scan`'s answer.
fn grep_self_role(
    profile: Profile,
    page_size: usize,
    mf: &[u8],
    bytes: &[u8],
    stderr: &mut Vec<u8>,
) -> Vec<u8> {
    let outcome = scan::grep_self_role(bytes, profile, page_size);
    if outcome.notice {
        push_line(stderr, &[b"grep: ", mf, b": binary file matches"]);
    }
    match outcome.printed.first() {
        Some((start, end)) => self_role_value(profile, &bytes[*start..*end]),
        None => Vec::new(),
    }
}

/// The outputs of `grep -E '^self_role:...'` GNU 3.11 permits for these bytes (checks: source-derived range assertions).
pub use scan::{permitted_outcomes, Outcome as GrepOutcome};

/// The one output this process's native buffers select from that set (the resolver's own choice).
pub fn native_grep_outcome(bytes: &[u8], profile: Profile, page_size: usize) -> GrepOutcome {
    scan::grep_self_role(bytes, profile, page_size)
}

/// The page size GNU grep 3.11 read (`sysconf (_SC_PAGESIZE)`), if the host answers.
pub fn host_page_size() -> Option<usize> {
    extern "C" {
        fn sysconf(name: i32) -> i64;
    }
    /// glibc Linux `_SC_PAGESIZE`.
    const SC_PAGESIZE: i32 = 30;
    // SAFETY: `sysconf` takes an integer and returns an integer; it reads none of our memory and has no preconditions
    // (libc is already linked by std on the Linux GNU platform this crate targets).
    let size = unsafe { sysconf(SC_PAGESIZE) };
    usize::try_from(size).ok().filter(|size| *size > 0)
}

/// `^[[:space:]]*-[[:space:]]*role:[[:space:]]*main([[:space:]]|#|$)` on one line.
fn is_main_role_line(profile: Profile, line: &[u8]) -> bool {
    let mut at = skip_spaces(profile, line, 0);
    if line.get(at) != Some(&b'-') {
        return false;
    }
    at = skip_spaces(profile, line, at + 1);
    let key = b"role:";
    if !line[at..].starts_with(key) {
        return false;
    }
    at = skip_spaces(profile, line, at + key.len());
    let main = MAIN_ROLE.as_bytes();
    if !line[at..].starts_with(main) {
        return false;
    }
    at += main.len();
    at == line.len() || line[at] == b'#' || space_len(profile, line, at).is_some()
}

/// `grep -Ec` of the main-role pattern.
fn count_main_roles(profile: Profile, bytes: &[u8]) -> usize {
    let binary = bytes.contains(&0);
    grep_lines(bytes, binary)
        .into_iter()
        .filter(|line| is_main_role_line(profile, line))
        .count()
}

fn scheme_violation(stderr: &mut Vec<u8>, what: &str, value: &[u8]) {
    push_line(
        stderr,
        &[
            "[node-identity] FAIL: manifest ".as_bytes(),
            what.as_bytes(),
            "='".as_bytes(),
            value,
            "' 가 스킴 위반이다(".as_bytes(),
            SLUG_RE.as_bytes(),
            ").".as_bytes(),
        ],
    );
}

enum Step {
    /// `return 0` with a value.
    Found(Vec<u8>),
    /// `return 1`: absent, the normal fall-through.
    Absent,
    /// `return 2`: a contract violation or an unreadable manifest, already written to stderr.
    Violation,
}

/// `ni_manifest_self_role`.
fn manifest_self_role(
    profile: Profile,
    page_size: usize,
    repo: &[u8],
    stderr: &mut Vec<u8>,
) -> Step {
    let mut found: Vec<u8> = Vec::new();
    let mut count = 0usize;
    for mf in manifests(repo) {
        let bytes = match read_manifest(&mf) {
            None => {
                unreadable(stderr, &mf);
                return Step::Violation;
            }
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => {
                push_line(
                    stderr,
                    &[b"grep: ", &mf, b": ", error.to_string().as_bytes()],
                );
                Vec::new()
            }
        };
        let value = grep_self_role(profile, page_size, &mf, &bytes, stderr);
        if value.is_empty() {
            continue;
        }
        if !found.is_empty() && found != value {
            push_line(
                stderr,
                &[
                    "[node-identity] FAIL: manifest 간 self_role 불일치('".as_bytes(),
                    &found,
                    "' vs '".as_bytes(),
                    &value,
                    "') — terraforming_node 로 재발급하라.".as_bytes(),
                ],
            );
            return Step::Violation;
        }
        found = value;
        count += 1;
    }
    if count > 0 {
        Step::Found(found)
    } else {
        Step::Absent
    }
}

/// `ni_manifest_main_role`.
fn manifest_main_role(profile: Profile, repo: &[u8], stderr: &mut Vec<u8>) -> Step {
    let mut count = 0usize;
    for mf in manifests(repo) {
        let bytes = match read_manifest(&mf) {
            None => {
                unreadable(stderr, &mf);
                return Step::Violation;
            }
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => {
                push_line(
                    stderr,
                    &[b"grep: ", &mf, b": ", error.to_string().as_bytes()],
                );
                Vec::new()
            }
        };
        let hits = count_main_roles(profile, &bytes);
        if hits == 0 {
            continue;
        }
        if hits != 1 {
            push_line(
                stderr,
                &[
                    "[node-identity] FAIL: ".as_bytes(),
                    &mf,
                    " 에 role: ".as_bytes(),
                    MAIN_ROLE.as_bytes(),
                    " 항목이 ".as_bytes(),
                    hits.to_string().as_bytes(),
                    "개다(계약은 정확히 1개).".as_bytes(),
                ],
            );
            push_line(
                stderr,
                &[
                    "[node-identity]       scan_node.sh manifest 게이트를 다시 통과시켜라."
                        .as_bytes(),
                ],
            );
            return Step::Violation;
        }
        count += 1;
    }
    if count > 0 {
        Step::Found(MAIN_ROLE.as_bytes().to_vec())
    } else {
        Step::Absent
    }
}

/// `bash node_identity.sh --resolve --repo REPO` for a non-empty `repo` (the spelling the caller gave, as bytes).
pub fn resolve(repo: &[u8], profile: Profile) -> Resolution {
    match host_page_size() {
        Some(page_size) => resolve_with_page_size(repo, profile, page_size),
        // grep 3.11 aborts when `sysconf (_SC_PAGESIZE)` is unusable; there is no business answer to invent. Status 2 is a
        // platform limit outside the original wrapper's 0/1 domain (the private helper's own error domain).
        None => Resolution {
            status: 2,
            stdout: Vec::new(),
            stderr: b"node_identity_resolve: platform: sysconf(_SC_PAGESIZE) is unavailable; GNU grep 3.11 (which the original ran) aborts there\n".to_vec(),
        },
    }
}

/// [`resolve`] with the page size grep's read buffers are sized from given (checks use it to hold the geometry fixed).
pub fn resolve_with_page_size(repo: &[u8], profile: Profile, page_size: usize) -> Resolution {
    let mut stderr = Vec::new();
    let status = |status: i32, stdout: Vec<u8>, stderr: Vec<u8>| Resolution {
        status,
        stdout,
        stderr,
    };
    if repo.is_empty()
        || !fs::metadata(os_path(repo))
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
    {
        push_line(
            &mut stderr,
            &["[node-identity] FAIL: repo 루트를 못 찾았다(node_id 해소 불가).".as_bytes()],
        );
        return status(1, Vec::new(), stderr);
    }
    let emit = |value: Vec<u8>, stderr: Vec<u8>| {
        let mut out = value;
        out.push(b'\n');
        Resolution {
            status: 0,
            stdout: out,
            stderr,
        }
    };

    match manifest_self_role(profile, page_size, repo, &mut stderr) {
        Step::Violation => return status(1, Vec::new(), stderr),
        Step::Found(value) if !value.is_empty() => {
            if valid_slug(&value) {
                return emit(value, stderr);
            }
            scheme_violation(&mut stderr, "self_role", &value);
            return status(1, Vec::new(), stderr);
        }
        _ => {}
    }
    match manifest_main_role(profile, repo, &mut stderr) {
        Step::Violation => return status(1, Vec::new(), stderr),
        Step::Found(value) if !value.is_empty() => {
            if valid_slug(&value) {
                return emit(value, stderr);
            }
            scheme_violation(&mut stderr, "role", &value);
            return status(1, Vec::new(), stderr);
        }
        _ => {}
    }
    push_line(
        &mut stderr,
        &[
            "[node-identity] FAIL: node_id 를 해소하지 못했다 — hostname 으로 대체하지 않는다."
                .as_bytes(),
        ],
    );
    push_line(
        &mut stderr,
        &[
            "[node-identity]   찾은 곳: (a) ".as_bytes(),
            repo,
            "/output/*/manifest.yaml 의 self_role: (서브 · terraforming 발급)  (b) 동 파일의 유일한 role: ".as_bytes(),
            MAIN_ROLE.as_bytes(),
            " (메인)".as_bytes(),
        ],
    );
    push_line(
        &mut stderr,
        &["[node-identity]   해소: 서브면 메인이 scan_node.sh --emit-sub-manifest → render_sub_env.sh 로 서브 manifest 재배달 · 메인이면 manifest 를".as_bytes()],
    );
    push_line(
        &mut stderr,
        &["[node-identity]         terraforming_node 로 채워라. 급하면 --node-id=<slug> 로 명시 주입.".as_bytes()],
    );
    status(1, Vec::new(), stderr)
}
