//! `regen_build_patches_src.py` — the `verify` and `bundle` subcommands (the two the local callers run).
//!
//! `verify` checks a `build_patches_src` payload against `PROVENANCE.json` (the files-map kind: per-file sha256, missing / mismatch / extra;
//! the `script-modules` kind: the exact authored inventory). `bundle` writes the deterministic `tar.gz` of a verified payload and prints its
//! sha256. `derive`, `unbundle` and `snapshot-resolutions` are NOT ported here: `unbundle` runs on the sub node (the original is piped to the
//! remote interpreter there), and `derive` / `snapshot-resolutions` clone upstream sources through git; they stay with the original script.
//!
//! The bundle is deterministic by construction (sorted members, mtime 0, uid/gid 0, two normalised modes, gzip mtime 0). It is not byte-identical
//! to the original's (another DEFLATE encoder); the caller only needs the same payload to give the same bytes, and the receiving side unpacks and
//! re-verifies against `PROVENANCE.json`.

use agent_runtime_support::pyjson::{decode_file_text, loads_text};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use ring::digest::{digest, SHA256};
use std::collections::BTreeSet;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

const TAG: &str = "[regen-src]";

struct Fail(String);

fn log(msg: &str) {
    println!("{TAG} {msg}");
}

fn fail<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail(msg.into()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_bytes(data: &[u8]) -> String {
    hex(digest(&SHA256, data).as_ref())
}

struct Opts {
    root: String,
    provenance: Option<String>,
    files_root: Option<String>,
    quiet: bool,
    out: Option<String>,
}

fn usage_error(msg: &str) -> i32 {
    eprintln!("usage: regen_build_patches_src.py [-h] [--repo REPO] {{verify,bundle}} --root ROOT [--provenance P] [--files-root F] ...");
    eprintln!("regen_build_patches_src.py: error: {msg}");
    2
}

fn dict_get<'a>(dict: &'a PyDict, key: &str) -> Option<&'a PyValue> {
    dict.get(key).map(PyValue::peel)
}

fn as_dict(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(d) => Some(d),
        _ => None,
    }
}

fn as_text(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}

struct Provenance {
    value: PyValue,
    script_modules: bool,
}

impl Provenance {
    fn dict(&self) -> &PyDict {
        as_dict(&self.value).expect("checked object")
    }

    fn files(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = dict_get(self.dict(), "files")
            .and_then(as_dict)
            .map(|d| {
                d.iter()
                    .map(|(k, v)| (k.to_utf8().unwrap_or_default(), as_text(v).unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
}

fn load_provenance(root: &str, override_path: &Option<String>) -> Result<(Provenance, String), Fail> {
    let path = override_path.clone().unwrap_or_else(|| format!("{root}/PROVENANCE.json"));
    if !Path::new(&path).is_file() {
        return fail(format!("PROVENANCE.json 부재: {path} — 이식 변종이 아니면 이 도구를 부르지 않는다"));
    }
    let bytes = std::fs::read(&path).map_err(|e| Fail(format!("{path}: {e}")))?;
    let text = decode_file_text(&bytes).map_err(|e| Fail(format!("{path}: {e:?}")))?;
    let value = loads_text(&text).map_err(|e| Fail(format!("{path}: {e}")))?;
    let Some(man) = as_dict(&value) else {
        return fail("PROVENANCE.json must be a JSON object");
    };
    let kind = dict_get(man, "source_payload")
        .and_then(as_dict)
        .and_then(|p| dict_get(p, "kind").cloned());
    match kind {
        Some(ref k) if as_text(k).as_deref() == Some("script-modules") => {
            if !man.contains_key("source") || man.contains_key("files") {
                return fail("script-modules requires source and forbids a files-map (including empty files)");
            }
            script_module_inventory(man)?;
            Ok((Provenance { value: value.clone(), script_modules: true }, path))
        }
        None | Some(PyValue::None) => files_map(value, path),
        Some(ref k) if as_text(k).as_deref() == Some("files-map") => files_map(value, path),
        Some(k) => fail(format!("unknown source_payload.kind: {}", k.repr())),
    }
}

fn files_map(value: PyValue, path: String) -> Result<(Provenance, String), Fail> {
    let man = as_dict(&value).expect("object");
    for key in ["source", "files"] {
        if !man.contains_key(key) {
            return fail(format!("PROVENANCE.json 스키마 위반: '{key}' 부재 ({path})"));
        }
    }
    let nfiles = dict_get(man, "files").and_then(as_dict).map(|d| d.len()).unwrap_or(0);
    let declared = dict_get(man, "counts").and_then(as_dict).and_then(|c| dict_get(c, "files").cloned());
    if let Some(d) = declared {
        if !matches!(d.peel(), PyValue::None) && !d.equals_int(&nfiles.into()) {
            return fail(format!(
                "PROVENANCE.json 자기모순: counts.files={} 인데 files 항목은 {nfiles}개",
                d.display_text().to_utf8().unwrap_or_default()
            ));
        }
    }
    Ok((Provenance { value, script_modules: false }, path))
}

fn is_sh_module(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 6
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'-'
        && name.ends_with(".sh")
        && name[3..name.len() - 3].chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
}

fn is_helper(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".py") else { return false };
    let mut chars = stem.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn script_module_inventory(man: &PyDict) -> Result<Vec<(String, String)>, Fail> {
    let payload = dict_get(man, "source_payload").and_then(as_dict);
    let modules = payload.and_then(|p| dict_get(p, "modules_sha256")).and_then(as_dict);
    let Some(modules) = modules.filter(|m| !m.is_empty()) else {
        return fail("script-modules needs a nonempty modules_sha256 inventory");
    };
    let mut helpers: Vec<String> = Vec::new();
    match payload.and_then(|p| dict_get(p, "helpers")) {
        None => {}
        Some(PyValue::List(items)) => {
            for item in items {
                match as_text(item) {
                    Some(n) if is_helper(&n) => helpers.push(n),
                    _ => return fail("script-modules helpers must be explicit top-level Python basenames"),
                }
            }
        }
        Some(_) => return fail("script-modules helpers must be explicit top-level Python basenames"),
    }
    if helpers.iter().collect::<BTreeSet<_>>().len() != helpers.len() {
        return fail("script-modules helpers must be explicit top-level Python basenames");
    }
    let mut out = Vec::new();
    for (name, dg) in modules.iter() {
        let name = name.to_utf8().unwrap_or_default();
        let dg = as_text(dg);
        let name_ok = is_sh_module(&name) || helpers.contains(&name);
        let dg_ok = dg.as_deref().map_or(false, |d| d.len() == 64 && d.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        if !name_ok || !dg_ok {
            return fail(format!("invalid script module inventory item: '{name}'"));
        }
        out.push((name, dg.unwrap()));
    }
    let names: BTreeSet<&String> = out.iter().map(|(n, _)| n).collect();
    if helpers.iter().any(|h| !names.contains(h)) || !out.iter().any(|(n, _)| n.ends_with(".sh")) {
        return fail("script-modules must contain scripts and every explicit helper");
    }
    out.sort();
    Ok(out)
}

fn verify_script_modules(root: &str, prov: &Provenance, quiet: bool) -> Result<i32, Fail> {
    let modules = script_module_inventory(prov.dict())?;
    let mut actual: BTreeSet<String> = BTreeSet::new();
    let rd = std::fs::read_dir(root).map_err(|e| Fail(format!("{root}: {e}")))?;
    for entry in rd {
        let entry = entry.map_err(|e| Fail(e.to_string()))?;
        actual.insert(entry.file_name().to_string_lossy().into_owned());
    }
    let names: BTreeSet<String> = modules.iter().map(|(n, _)| n.clone()).collect();
    let mut allowed = names.clone();
    allowed.extend(["PROVENANCE.json".to_string(), ".gitkeep".to_string()]);
    let mut required = names.clone();
    required.insert("PROVENANCE.json".to_string());
    let missing: Vec<&String> = required.difference(&actual).collect();
    let extra: Vec<&String> = actual.difference(&allowed).collect();
    if !missing.is_empty() || !extra.is_empty() {
        let fmt = |v: &[&String]| format!("[{}]", v.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "));
        return fail(format!("script-modules exact inventory mismatch: missing={} extra={}", fmt(&missing), fmt(&extra)));
    }
    for name in &actual {
        let path = Path::new(root).join(name);
        let meta = std::fs::symlink_metadata(&path).map_err(|e| Fail(format!("{name}: {e}")))?;
        if !meta.file_type().is_file() || meta.nlink() != 1 {
            return fail(format!("script-modules refuses nonregular/symlink/hardlink: {name}"));
        }
        let data = std::fs::read(&path).map_err(|e| Fail(format!("{name}: {e}")))?;
        if let Some((_, want)) = modules.iter().find(|(n, _)| n == name) {
            if &sha256_bytes(&data) != want {
                return fail(format!("script-modules sha256 mismatch: {name}"));
            }
        }
        if name == "PROVENANCE.json" {
            let same = decode_file_text(&data).ok().and_then(|t| loads_text(&t).ok()).map_or(false, |v| v == prov.value);
            if !same {
                return fail("script-modules provenance differs from selected authority");
            }
        }
        if name == ".gitkeep" && !data.is_empty() {
            return fail("tracked .gitkeep scaffold must be empty, not payload");
        }
    }
    if !quiet {
        log(&format!("script-modules verified: {} authored modules; files-map derive/bundle not applicable", modules.len()));
    }
    Ok(0)
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        if std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
            walk(base, &path, out);
        } else if let Ok(rel) = path.strip_prefix(base) {
            out.push(rel.to_string_lossy().into_owned());
        }
    }
}

fn verify(files_root: &str, files: &[(String, String)], quiet: bool) -> i32 {
    if !Path::new(files_root).is_dir() {
        eprintln!("{TAG} FAIL: payload 디렉터리 부재: {files_root}");
        return 1;
    }
    let (mut missing, mut mismatch, mut extra) = (Vec::new(), Vec::new(), Vec::new());
    for (rel, want) in files {
        let path = Path::new(files_root).join(rel);
        let is_link = std::fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if !path.is_file() || is_link {
            missing.push(rel.clone());
            continue;
        }
        let got = std::fs::read(&path).ok().map(|d| sha256_bytes(&d));
        if got.as_deref() != Some(want.as_str()) {
            mismatch.push(rel.clone());
        }
    }
    let declared: BTreeSet<&String> = files.iter().map(|(r, _)| r).collect();
    let mut found = Vec::new();
    walk(Path::new(files_root), Path::new(files_root), &mut found);
    for rel in found {
        if !declared.contains(&rel) {
            extra.push(rel);
        }
    }
    let ok = missing.is_empty() && mismatch.is_empty() && extra.is_empty();
    let line = format!(
        "무결성 {} — declared={} missing={} mismatch={} extra={}",
        if ok { "OK" } else { "FAIL" },
        files.len(),
        missing.len(),
        mismatch.len(),
        extra.len()
    );
    if ok {
        if !quiet {
            log(&line);
        }
        return 0;
    }
    eprintln!("{TAG} {line}");
    for (label, items) in [("MISSING", &missing), ("MISMATCH", &mismatch), ("EXTRA", &extra)] {
        for rel in items.iter().take(40) {
            eprintln!("{TAG}   {label} {rel}");
        }
        if items.len() > 40 {
            eprintln!("{TAG}   {label} … 외 {}건", items.len() - 40);
        }
    }
    1
}

// ── bundle: ustar (+ PAX `path` record for long names) inside a gzip member with mtime 0 ──

fn octal(field: &mut [u8], value: u64) {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    field.copy_from_slice(text.as_bytes());
}

fn header(name: &str, size: u64, mode: u32, typeflag: u8) -> [u8; 512] {
    let mut h = [0u8; 512];
    let nb = name.as_bytes();
    let n = nb.len().min(100);
    h[..n].copy_from_slice(&nb[..n]);
    octal(&mut h[100..108], mode as u64);
    octal(&mut h[108..116], 0);
    octal(&mut h[116..124], 0);
    octal(&mut h[124..136], size);
    octal(&mut h[136..148], 0);
    h[148..156].copy_from_slice(b"        ");
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|b| *b as u32).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    h
}

fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body_len = key.len() + value.len() + 3; // ' ' '=' '\n'
    let mut total = body_len + body_len.to_string().len();
    if total.to_string().len() != body_len.to_string().len() {
        total = body_len + total.to_string().len();
    }
    format!("{total} {key}={value}\n").into_bytes()
}

fn pad(out: &mut Vec<u8>) {
    while out.len() % 512 != 0 {
        out.push(0);
    }
}

fn build_tar(files_root: &str, files: &[(String, String)]) -> Result<Vec<u8>, Fail> {
    let mut out = Vec::new();
    for (rel, _) in files {
        let path = Path::new(files_root).join(rel);
        let data = std::fs::read(&path).map_err(|e| Fail(format!("{}: {e}", path.display())))?;
        let mode = std::fs::metadata(&path).map_err(|e| Fail(e.to_string()))?.permissions().mode();
        let mode = if mode & 0o100 != 0 { 0o755 } else { 0o644 };
        let name = format!("files/{rel}");
        if name.len() > 100 || !name.is_ascii() {
            let rec = pax_record("path", &name);
            out.extend_from_slice(&header("PaxHeader", rec.len() as u64, 0o644, b'x'));
            out.extend_from_slice(&rec);
            pad(&mut out);
        }
        out.extend_from_slice(&header(&name, data.len() as u64, mode, b'0'));
        out.extend_from_slice(&data);
        pad(&mut out);
    }
    out.extend_from_slice(&[0u8; 1024]);
    Ok(out)
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 255];
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, 9));
    out.extend_from_slice(&crc32fast::hash(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn parse(args: &[String]) -> Result<(String, Opts), i32> {
    let mut it = args.iter();
    let mut cmd = None;
    let mut o = Opts { root: String::new(), provenance: None, files_root: None, quiet: false, out: None };
    let mut have_root = false;
    while let Some(arg) = it.next() {
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = || -> Result<String, i32> {
            inline.clone().or_else(|| it.next().cloned()).ok_or_else(|| usage_error(&format!("argument {key}: expected one argument")))
        };
        match key.as_str() {
            "--repo" => {
                value()?;
            }
            "--root" => {
                o.root = value()?;
                have_root = true;
            }
            "--provenance" => o.provenance = Some(value()?),
            "--files-root" => o.files_root = Some(value()?),
            "--out" => o.out = Some(value()?),
            "--quiet" => o.quiet = true,
            "verify" | "bundle" if cmd.is_none() => cmd = Some(key.clone()),
            "derive" | "unbundle" | "snapshot-resolutions" if cmd.is_none() => {
                eprintln!("{TAG} FAIL: {key} 는 Rust 이식 범위 밖이다 — 원본 regen_build_patches_src.py 를 직접 쓴다(서브 stdin 실행·상류 clone)");
                return Err(2);
            }
            other => return Err(usage_error(&format!("unrecognized arguments: {other}"))),
        }
    }
    let Some(cmd) = cmd else { return Err(usage_error("the following arguments are required: cmd")) };
    if !have_root {
        return Err(usage_error("the following arguments are required: --root"));
    }
    if cmd == "bundle" && o.out.is_none() {
        return Err(usage_error("the following arguments are required: --out"));
    }
    Ok((cmd, o))
}

fn execute(cmd: &str, o: &Opts) -> Result<i32, Fail> {
    let (prov, path) = load_provenance(&o.root, &o.provenance)?;
    if prov.script_modules {
        if cmd == "bundle" {
            return fail("bundle is not applicable to script-modules: authored modules are verified and delivered directly; no files/ derivation or bundle");
        }
        if o.files_root.is_some() {
            return fail("script-modules verify does not accept --files-root");
        }
        log(&format!("authority={path}"));
        return verify_script_modules(&o.root, &prov, o.quiet);
    }
    let files_root = o.files_root.clone().unwrap_or_else(|| format!("{}/files", o.root));
    let files = prov.files();
    if cmd == "verify" {
        log(&format!("authority={path}"));
        return Ok(verify(&files_root, &files, o.quiet));
    }
    if verify(&files_root, &files, true) != 0 {
        return fail("payload 무결성 실패 — 번들을 만들지 않는다(검증되지 않은 바이트는 배달하지 않는다)");
    }
    let out = o.out.clone().expect("checked");
    let out_abs = std::path::absolute(&out).map_err(|e| Fail(e.to_string()))?;
    if let Some(dir) = out_abs.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Fail(format!("{}: {e}", dir.display())))?;
    }
    let bytes = gzip(&build_tar(&files_root, &files)?);
    let tmp = PathBuf::from(format!("{out}.partial"));
    std::fs::File::create(&tmp).and_then(|mut f| f.write_all(&bytes)).map_err(|e| Fail(format!("{}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, &out).map_err(|e| Fail(format!("{out}: {e}")))?;
    let sha = sha256_bytes(&bytes);
    log(&format!("번들 발행 {out} files={} bytes={} sha256={sha}", files.len(), bytes.len()));
    log(&format!("authority={path}"));
    println!("{sha}");
    Ok(0)
}

pub fn run(args: &[String]) -> i32 {
    let (cmd, opts) = match parse(args) {
        Ok(parsed) => parsed,
        Err(status) => return status,
    };
    let status = match execute(&cmd, &opts) {
        Ok(status) => status,
        Err(Fail(msg)) => {
            eprintln!("{TAG} FAIL: {msg}");
            1
        }
    };
    let _ = std::io::stdout().flush();
    status
}
