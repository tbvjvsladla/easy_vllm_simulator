//! `regen_build_patches_src` — the four subcommands that replace the inline Python of `sync_to_sub.sh`
//! (`script_modules_active`, `script_module_files`, `snapshot_script_modules`, `snapshot_post_build_patches`).
//!
//! `modules-kind` / `modules-files` read one `PROVENANCE.json`. `snapshot-modules` / `snapshot-post` copy untracked build inputs of the canonical
//! tree into the transactional snapshot. Every directory is opened without following a symlink (`O_NOFOLLOW | O_DIRECTORY`, one component at a
//! time, each relative to the descriptor of its parent through `/proc/self/fd`), a payload file is opened `O_NOFOLLOW | O_NONBLOCK` and must be a
//! regular file with one link, and a copy that changed while it was read is refused. Messages and exit codes follow the heredoc programs: a
//! `[sync] FAIL(...)` line on stderr and exit 1, `[sync] info:` lines on stdout.

use crate::regen_patches_src::{as_dict, as_text, dict_get, script_module_inventory, verify_script_modules, Fail, Opts, Provenance};
use agent_runtime_support::pyjson::{decode_file_text, loads_text};
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// A `SystemExit(<text>)` of the original heredoc: the text goes to stderr verbatim, exit 1. `Done` is `SystemExit(0)`.
enum Stop {
    Done,
    Exit(String),
}

type Step<T> = Result<T, Stop>;

fn exit<T>(msg: impl Into<String>) -> Step<T> {
    Err(Stop::Exit(msg.into()))
}

fn io_exit<T>(what: &str, err: std::io::Error) -> Step<T> {
    exit(format!("{what}: {err}"))
}

fn load_json_file(path: &str) -> Step<PyValue> {
    let bytes = std::fs::read(path).map_err(|e| Stop::Exit(format!("{path}: {e}")))?;
    parse_json(&bytes, path)
}

fn parse_json(bytes: &[u8], what: &str) -> Step<PyValue> {
    let text = decode_file_text(bytes).map_err(|e| Stop::Exit(format!("{what}: {e:?}")))?;
    loads_text(&text).map_err(|e| Stop::Exit(format!("{what}: {e}")))
}

fn payload_kind(value: &PyValue) -> Option<PyValue> {
    as_dict(value).and_then(|m| dict_get(m, "source_payload")).and_then(as_dict).and_then(|p| dict_get(p, "kind").cloned())
}

fn kind_is(value: &PyValue, want: &str) -> bool {
    payload_kind(value).as_ref().and_then(as_text).as_deref() == Some(want)
}

fn modules_kind(o: &Opts) -> Step<()> {
    let value = load_json_file(o.manifest.as_deref().unwrap_or_default())?;
    if kind_is(&value, "script-modules") {
        Err(Stop::Done)
    } else {
        Err(Stop::Exit(String::new()))
    }
}

fn modules_files(o: &Opts) -> Step<()> {
    let value = load_json_file(o.manifest.as_deref().unwrap_or_default())?;
    let names: BTreeSet<String> = as_dict(&value)
        .and_then(|m| dict_get(m, "source_payload"))
        .and_then(as_dict)
        .and_then(|p| dict_get(p, "modules_sha256"))
        .and_then(as_dict)
        .map(|d| d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect())
        .ok_or_else(|| Stop::Exit("modules_sha256 missing".to_string()))?;
    let mut out = String::new();
    for name in names {
        out.push_str(&name);
        out.push('\n');
    }
    out.push_str("PROVENANCE.json\n");
    print!("{out}");
    let _ = std::io::stdout().flush();
    Ok(())
}

/// `os.path.abspath`: absolute and lexically normalised (`..` and `.` collapsed, no symlink resolution).
fn abspath(path: &str) -> PathBuf {
    let base = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(path)
    };
    let mut out = PathBuf::from("/");
    for comp in base.components() {
        match comp {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            _ => {}
        }
    }
    out
}

fn fd_path(dir: &File, name: &str) -> String {
    format!("/proc/self/fd/{}/{}", dir.as_raw_fd(), name)
}

fn open_dir_at(parent: &File, name: &str) -> std::io::Result<File> {
    std::fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW).open(fd_path(parent, name))
}

fn open_root() -> std::io::Result<File> {
    std::fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY).open("/")
}

/// Directory descriptor of `path` with no symlink ancestor (`open_module_root`).
fn open_dir_chain(path: &Path) -> std::io::Result<File> {
    let mut dir = open_root()?;
    for comp in path.components() {
        if let Component::Normal(part) = comp {
            dir = open_dir_at(&dir, &part.to_string_lossy())?;
        }
    }
    Ok(dir)
}

fn open_file_at(dir: &File, name: &str) -> std::io::Result<File> {
    std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(fd_path(dir, name))
}

fn lstat_at(dir: &File, name: &str) -> std::io::Result<std::fs::Metadata> {
    std::fs::symlink_metadata(fd_path(dir, name))
}

fn tracked_files(canonical: &str, relative: &str) -> Step<Vec<Vec<u8>>> {
    let out = Command::new("git").args(["-C", canonical, "ls-files", "-z", "--", relative]).output().map_err(|e| Stop::Exit(format!("git: {e}")))?;
    if !out.status.success() {
        return exit(format!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout.split(|b| *b == 0).map(|s| s.to_vec()).collect())
}

fn any_tracked_beyond_gitkeep(tracked: &[Vec<u8>], relative: &str) -> bool {
    let keep = format!("{relative}/.gitkeep").into_bytes();
    tracked.iter().any(|n| !n.is_empty() && *n != keep)
}

fn same_snapshot(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    (a.size(), a.mtime(), a.mtime_nsec(), a.ctime(), a.ctime_nsec()) == (b.size(), b.mtime(), b.mtime_nsec(), b.ctime(), b.ctime_nsec())
}

fn copy_new(source: &mut File, dest: &Path) -> Step<()> {
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(|e| Stop::Exit(format!("{}: {e}", dest.display())))?;
    std::io::copy(source, &mut out).map_err(|e| Stop::Exit(format!("{}: {e}", dest.display())))?;
    Ok(())
}

fn chmod(dest: &Path, mode: u32) -> Step<()> {
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(mode & 0o777)).map_err(|e| Stop::Exit(format!("{}: {e}", dest.display())))
}

fn fail_text(Fail(msg): Fail) -> Stop {
    Stop::Exit(format!("{} FAIL: {msg}", crate::regen_patches_src::TAG))
}

fn verify_modules_dir(path: &Path, value: &PyValue, quiet: bool) -> Step<()> {
    // Verified through the descriptor of the symlink-free walk, so the checked directory is the one that was opened.
    let dir = open_dir_chain(path).map_err(|e| Stop::Exit(format!("{}: {e}", path.display())))?;
    let prov = Provenance { value: value.clone(), script_modules: true };
    let root = format!("/proc/self/fd/{}", dir.as_raw_fd());
    match verify_script_modules(&root, &prov, quiet) {
        Ok(0) => Ok(()),
        Ok(_) => exit(String::new()),
        Err(f) => Err(fail_text(f)),
    }
}

fn snapshot_modules(o: &Opts) -> Step<()> {
    let (canonical, snapshot, topology) = (o.canonical.as_deref().unwrap_or(""), o.snapshot.as_deref().unwrap_or(""), o.topology.as_deref().unwrap_or(""));
    if topology != "single" && topology != "multi" {
        return exit("[sync] FAIL(PRE modules): invalid topology");
    }
    let relative = format!("output/{topology}/build_patches_src");
    let root = abspath(&format!("{canonical}/{relative}"));
    let dir = match open_dir_chain(&root) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Stop::Done),
        Err(e) => return io_exit(&root.display().to_string(), e),
    };
    let mut prov_file = match open_file_at(&dir, "PROVENANCE.json") {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Stop::Done),
        Err(e) => return io_exit("PROVENANCE.json", e),
    };
    let meta = prov_file.metadata().map_err(|e| Stop::Exit(e.to_string()))?;
    if !meta.file_type().is_file() || meta.nlink() != 1 {
        return exit("[sync] FAIL(PRE modules): nonregular provenance");
    }
    let mut bytes = Vec::new();
    prov_file.read_to_end(&mut bytes).map_err(|e| Stop::Exit(e.to_string()))?;
    let value = parse_json(&bytes, "PROVENANCE.json")?;
    let Some(man) = as_dict(&value) else { return exit("PROVENANCE.json must be a JSON object") };
    match payload_kind(&value) {
        None | Some(PyValue::None) => {
            if !man.contains_key("files") {
                return exit("[sync] FAIL(PRE modules): authored modules require an explicit script-modules discriminator");
            }
            return Err(Stop::Done);
        }
        Some(ref k) if as_text(k).as_deref() == Some("files-map") => {
            if !man.contains_key("files") {
                return exit("[sync] FAIL(PRE modules): authored modules require an explicit script-modules discriminator");
            }
            return Err(Stop::Done);
        }
        Some(ref k) if as_text(k).as_deref() == Some("script-modules") => {}
        Some(_) => return exit("[sync] FAIL(PRE modules): unknown source payload kind"),
    }
    if !man.contains_key("source") || man.contains_key("files") {
        return exit("[sync] FAIL(PRE modules): ambiguous/invalid provenance schema");
    }
    verify_modules_dir(&root, &value, false)?;
    if any_tracked_beyond_gitkeep(&tracked_files(canonical, &relative)?, &relative) {
        return exit("[sync] FAIL(PRE modules): authored modules must remain untracked");
    }
    let dest = Path::new(snapshot).join(&relative);
    std::fs::create_dir_all(&dest).map_err(|e| Stop::Exit(format!("{}: {e}", dest.display())))?;
    let mut names: BTreeSet<String> = script_module_inventory(man).map_err(fail_text)?.into_iter().map(|(n, _)| n).collect();
    names.insert("PROVENANCE.json".to_string());
    for name in &names {
        let mut input = open_file_at(&dir, name).map_err(|e| Stop::Exit(format!("{name}: {e}")))?;
        let st = input.metadata().map_err(|e| Stop::Exit(e.to_string()))?;
        if !st.file_type().is_file() || st.nlink() != 1 {
            return exit("[sync] FAIL(PRE modules): input changed type");
        }
        copy_new(&mut input, &dest.join(name))?;
        chmod(&dest.join(name), st.mode())?;
    }
    verify_modules_dir(&dest, &value, false)?;
    println!("[sync] script-modules snapshot: {relative}; no files/ derivation or bundle");
    Ok(())
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
}

fn snapshot_post(o: &Opts) -> Step<()> {
    let (canonical, snapshot, topology) = (o.canonical.as_deref().unwrap_or(""), o.snapshot.as_deref().unwrap_or(""), o.topology.as_deref().unwrap_or(""));
    let kind = o.kind.as_deref().unwrap_or("post");
    let leaf = match kind {
        "post" => "build_patches",
        "arch-dockerfile" => "Dockerfile.source-build-sm12x",
        "deps-dockerfile" => "Dockerfile.source-build-deps2",
        _ => "",
    };
    if (topology != "single" && topology != "multi") || leaf.is_empty() {
        return exit("[sync] FAIL(post slot): invalid topology/input kind");
    }
    let relative = format!("output/{topology}/{leaf}");
    if any_tracked_beyond_gitkeep(&tracked_files(canonical, &relative)?, &relative) {
        return exit("[sync] FAIL(build input): generated payload must remain untracked");
    }
    let scaffold = Path::new(snapshot).join(&relative).join(".gitkeep");
    if kind == "post" {
        if let Ok(st) = std::fs::symlink_metadata(&scaffold) {
            if !st.file_type().is_file() || st.nlink() != 1 {
                return exit("[sync] FAIL(post slot): invalid index-owned .gitkeep scaffold");
            }
        }
    }
    let canonical_root = canonical.trim_end_matches('/');
    let mut dir = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(if canonical_root.is_empty() { "/" } else { canonical_root })
        .map_err(|e| Stop::Exit(format!("{canonical_root}: {e}")))?;
    let parts: &[&str] = if kind == "post" { &["output", topology, "build_patches"] } else { &["output", topology] };
    for part in parts {
        match open_dir_at(&dir, part) {
            Ok(child) => dir = child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("[sync] info: optional post slot absent: {relative}");
                return Err(Stop::Done);
            }
            Err(e) => return io_exit(part, e),
        }
    }
    let names: Vec<String> = if kind == "post" {
        let listing = std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd())).map_err(|e| Stop::Exit(e.to_string()))?;
        let mut names = Vec::new();
        for entry in listing {
            let name = entry.map_err(|e| Stop::Exit(e.to_string()))?.file_name();
            let name = name.to_string_lossy().into_owned();
            if name != ".gitkeep" {
                names.push(name);
            }
        }
        names.sort();
        names
    } else {
        match lstat_at(&dir, leaf) {
            Ok(_) => vec![leaf.to_string()],
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Stop::Done),
            Err(e) => return io_exit(leaf, e),
        }
    };
    for name in &names {
        let st = lstat_at(&dir, name).map_err(|e| Stop::Exit(format!("{name}: {e}")))?;
        if !st.file_type().is_file() || st.nlink() != 1 {
            return exit(format!("[sync] FAIL(post slot): nonregular/symlink/hardlink payload: {relative}/{name}"));
        }
        if !safe_name(name) {
            return exit(format!("[sync] FAIL(post slot): unsafe filename: '{name}'"));
        }
    }
    if !names.is_empty() {
        let dest = if kind == "post" { Path::new(snapshot).join(&relative) } else { Path::new(snapshot).join(format!("output/{topology}")) };
        std::fs::create_dir_all(&dest).map_err(|e| Stop::Exit(format!("{}: {e}", dest.display())))?;
        for name in &names {
            let mut source = open_file_at(&dir, name).map_err(|e| Stop::Exit(format!("{name}: {e}")))?;
            let before = source.metadata().map_err(|e| Stop::Exit(e.to_string()))?;
            if !before.file_type().is_file() || before.nlink() != 1 {
                return exit("[sync] FAIL(post slot): payload changed type while copying");
            }
            copy_new(&mut source, &dest.join(name))?;
            let after = source.metadata().map_err(|e| Stop::Exit(e.to_string()))?;
            if !same_snapshot(&before, &after) {
                return exit("[sync] FAIL(post slot): payload changed while copying");
            }
            chmod(&dest.join(name), before.mode())?;
        }
    }
    println!("[sync] info: untracked {kind} build-input snapshot: {relative} ({} regular files)", names.len());
    Ok(())
}

pub(crate) fn run(cmd: &str, o: &Opts) -> Result<i32, Fail> {
    let result = match cmd {
        "modules-kind" => modules_kind(o),
        "modules-files" => modules_files(o),
        "snapshot-modules" => snapshot_modules(o),
        _ => snapshot_post(o),
    };
    let _ = std::io::stdout().flush();
    match result {
        Ok(()) | Err(Stop::Done) => Ok(0),
        Err(Stop::Exit(msg)) => {
            if !msg.is_empty() {
                eprintln!("{msg}");
            }
            Ok(1)
        }
    }
}
