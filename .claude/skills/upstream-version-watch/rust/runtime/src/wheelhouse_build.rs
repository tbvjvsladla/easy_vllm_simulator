//! `native_wheelhouse.py build`: repacks the installed tree of an image (RECORD-verified, never recompiled) into an offline wheelhouse and a
//! runtime-lib bundle, and writes `closure.json` / `requirements-closure.txt`. The `verify` subcommand and `--self-test` of the original are
//! not part of this program (status 2).

use crate::wheelhouse_source::*;
use crate::wheelhouse_util::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub const TOOL: &str = "native_wheelhouse.py/v1";
pub const SITE_DEFAULT: &str = "/usr/local/lib/python3.12/dist-packages";
pub const SITES_DEFAULT: [&str; 3] = [SITE_DEFAULT, "/usr/lib/python3/dist-packages", "/usr/lib/python3.12/dist-packages"];
pub const CUDA_LINK_DEFAULT: &str = "/usr/local/cuda";
const DEFLATE_LEVEL: u8 = 1;
const EXCLUDE_PIP: &str = "설치기 자신 — venv 부트스트랩(ensurepip)이 소유한다. 설치 도중 실행 중인 설치기를 교체하지 않는다.";
const INSTALL_MARKERS: [&str; 3] = ["INSTALLER", "REQUESTED", "direct_url.json"];

// ── host-provided library classes (the closed tripwire lists of the original) ──

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
/// `(?:\.\d+)*` over the whole rest.
fn dot_digits(rest: &str) -> bool {
    rest.is_empty() || (rest.starts_with('.') && rest[1..].split('.').all(digits))
}
fn host_glibc(n: &str) -> bool {
    if n == "ld-linux-aarch64.so.1" || n == "ld-linux-x86-64.so.2" {
        return true;
    }
    let Some(b) = n.strip_prefix("lib") else { return false };
    let Some((stem, ver)) = b.split_once(".so.") else { return false };
    if !digits(ver) {
        return false;
    }
    matches!(stem, "c" | "m" | "dl" | "pthread" | "rt" | "util" | "resolv" | "anl" | "mvec" | "BrokenLocale" | "c_malloc_debug" | "thread_db" | "nsl")
        || stem.strip_prefix("nss_").map_or(false, |x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_lowercase()))
}
fn host_driver(n: &str) -> bool {
    let Some(b) = n.strip_prefix("lib") else { return false };
    let Some(i) = b.find(".so") else { return false };
    let (stem, rest) = (&b[..i], &b[i + 3..]);
    if !dot_digits(rest) {
        return false;
    }
    matches!(stem, "cuda" | "nvcuvid" | "nvoptix")
        || stem.strip_prefix("nvidia-").map_or(false, |x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'))
}
fn host_python(n: &str) -> bool {
    let Some(b) = n.strip_prefix("libpython") else { return false };
    let Some(i) = b.find(".so") else { return false };
    let mm = &b[..i];
    let Some((a, c)) = mm.split_once('.') else { return false };
    digits(a) && digits(c) && dot_digits(&b[i + 3..])
}
fn kernel_abi(n: &str) -> bool {
    let Some(b) = n.strip_prefix("lib") else { return false };
    let Some((stem, ver)) = b.split_once(".so.") else { return false };
    digits(ver) && matches!(stem, "ibverbs" | "rdmacm" | "mlx4" | "mlx5" | "efa" | "nl-3" | "nl-route-3" | "ibumad" | "ibmad")
}

// ── bounded parallel map ──

fn par_map<T: Sync, R: Send>(items: &[T], jobs: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1).min(items.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                out.lock().unwrap()[i] = Some(r);
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|x| x.unwrap()).collect()
}

// ── directory walk with the semantics of `os.walk` (top-down, symlinked directories listed but not entered) ──

fn walk(root: &Path, f: &mut dyn FnMut(&Path, &mut Vec<String>, &mut Vec<String>)) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    let (mut dns, mut fns) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if e.path().is_dir() {
            dns.push(n);
        } else {
            fns.push(n);
        }
    }
    f(root, &mut dns, &mut fns);
    for d in dns {
        let p = root.join(&d);
        if !p.symlink_metadata().map_or(false, |m| m.file_type().is_symlink()) {
            walk(&p, f);
        }
    }
}

fn du(p: &Path) -> i64 {
    let mut tot = 0i64;
    walk(p, &mut |dp, _d, fns| {
        for f in fns.iter() {
            if let Ok(m) = std::fs::symlink_metadata(dp.join(f)) {
                tot += m.blocks() as i64 * 512;
            }
        }
    });
    tot
}

// ── wheel writer ──

pub enum Content {
    Path(PathBuf),
    Bytes(Vec<u8>),
}
pub struct Entry {
    pub arc: String,
    pub content: Content,
    pub mode: u32,
}

/// `write_wheel`: members sorted by name, RECORD rebuilt from the bytes actually written. Returns (member count, payload bytes).
pub fn write_wheel(dest: &Path, entries: &[Entry], distinfo: &str) -> Result<(usize, u64), Err> {
    let tmp = PathBuf::from(format!("{}.part", dest.display()));
    let mut zip = ZipOut::create(&tmp)?;
    let mut order: Vec<&Entry> = entries.iter().collect();
    order.sort_by(|a, b| a.arc.cmp(&b.arc));
    let mut rec_rows: Vec<Vec<String>> = Vec::new();
    let mut total = 0u64;
    for e in order {
        let (digest, n) = match &e.content {
            Content::Bytes(b) => zip.add_stream(&e.arc, e.mode, &mut &b[..], DEFLATE_LEVEL)?,
            Content::Path(p) => {
                let mut f = std::fs::File::open(p)?;
                zip.add_stream(&e.arc, e.mode, &mut f, DEFLATE_LEVEL)?
            }
        };
        rec_rows.push(vec![e.arc.clone(), format!("sha256={}", b64url(&digest)), n.to_string()]);
        total += n;
    }
    rec_rows.push(vec![format!("{distinfo}/RECORD"), String::new(), String::new()]);
    zip.add_bytes(&format!("{distinfo}/RECORD"), 0o644, csv_write(&rec_rows).as_bytes(), DEFLATE_LEVEL)?;
    zip.finish()?;
    std::fs::rename(&tmp, dest)?;
    Ok((entries.len(), total))
}

// ── accept declarations ──

#[derive(Clone)]
pub struct AcceptMeta {
    pub sha256: String,
    pub approved_by: String,
    pub approved_utc: String,
    pub image_id: String,
    pub image_tag: Option<String>,
    pub image_ids: Vec<String>,
}
pub type Accept = BTreeMap<String, (String, String)>;

pub fn is_utc(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && (0..20).all(|i| match i {
            4 | 7 => b[i] == b'-',
            10 => b[i] == b'T',
            13 | 16 => b[i] == b':',
            19 => b[i] == b'Z',
            _ => b[i].is_ascii_digit(),
        })
}

pub fn parse_accept(items: &[String]) -> Result<Accept, Err> {
    let mut out = Accept::new();
    for it in items {
        let ok = (|| {
            let (head, reason) = it.split_once('=')?;
            let (dist, path) = head.split_once(':')?;
            if dist.is_empty() || path.is_empty() || reason.is_empty() || dist.contains(':') {
                return None;
            }
            Some((dist.to_string(), path.to_string(), reason.to_string()))
        })();
        let Some((dist, path, reason)) = ok else { return wh(format!("--accept-mismatch 형식은 DIST:PATH=REASON: {it:?}")) };
        out.insert(format!("{}:{}", canon(&dist), path), (reason, "human:--accept-mismatch".to_string()));
    }
    Ok(out)
}

pub fn load_accept_file(path: &Path) -> Result<(Accept, AcceptMeta), Err> {
    let raw = std::fs::read(path).map_err(|e| Err::Crash(format!("OSError: {e}")))?;
    let Ok(doc) = loads(&raw) else { return wh(format!("승인 파일 JSON 오류: {}", path.display())) };
    let need = ["schema_version", "approved_by", "approved_utc", "accepted_mismatches", "image_inherent_conflicts"];
    let mut miss: Vec<&str> = need.iter().copied().filter(|k| doc.get(k).is_none()).collect();
    let imgs: Vec<J> = match (doc.get("images"), doc.get("image")) {
        (Some(J::Arr(a)), _) => a.clone(),
        (_, Some(o @ J::Obj(_))) => vec![o.clone()],
        _ => vec![],
    };
    if imgs.is_empty() {
        miss.push("images");
    }
    let approved_by = doc.get("approved_by").and_then(|x| x.str()).unwrap_or("").trim().to_string();
    let approved_utc = doc.get("approved_utc").and_then(|x| x.str()).unwrap_or("").to_string();
    let img_ok = imgs.iter().all(|i| i.get("id").and_then(|x| x.str()).map_or(false, |s| s.starts_with("sha256:")));
    if !miss.is_empty() || !img_ok || !is_utc(&approved_utc) || approved_by.is_empty() {
        return wh(format!("승인 파일 형식 위반(누락 {miss:?} · images[].id sha256 · approved_utc · approved_by): {}", path.display()));
    }
    let mut acc = Accept::new();
    for e in doc.get("accepted_mismatches").map(|a| a.arr()).unwrap_or(&[]) {
        let g = |k: &str| e.get(k).filter(|x| x.truthy()).and_then(|x| x.str()).map(String::from);
        let (Some(d), Some(p), Some(r)) = (g("dist"), g("path"), g("reason")) else {
            return wh("accepted_mismatches 항목에 dist/path/reason 필수");
        };
        acc.insert(format!("{}:{}", canon(&d), p), (r, format!("accepted-mismatch(human: {approved_by})")));
    }
    for e in doc.get("image_inherent_conflicts").map(|a| a.arr()).unwrap_or(&[]) {
        if e.get("pip_check_line").and_then(|x| x.str()).map_or(true, |s| s.trim().is_empty()) {
            return wh("image_inherent_conflicts 항목에 pip_check_line 필수");
        }
    }
    let img = &imgs[0];
    let meta = AcceptMeta {
        sha256: sha256_hex(&raw),
        approved_by,
        approved_utc,
        image_id: img.get("id").and_then(|x| x.str()).unwrap_or("").to_string(),
        image_tag: img.get("tag").and_then(|x| x.str()).map(String::from),
        image_ids: imgs.iter().filter_map(|i| i.get("id").and_then(|x| x.str()).map(String::from)).collect(),
    };
    Ok((acc, meta))
}

// ── closure records ──

#[derive(Default)]
struct Rv {
    result: Option<String>,
    verified: i64,
    unhashed: Vec<String>,
    mismatches: Vec<String>,
    missing: Vec<String>,
    shared: Vec<(String, String)>,
    accepted: Vec<(String, String, String)>,
}
struct EditableRec {
    direct_url: Option<String>,
    mapping: Vec<(String, String)>,
    tree_files: i64,
}
struct Rec {
    name: String,
    version: String,
    dist_info: String,
    wheel: Option<String>,
    source: Option<String>,
    file_count: i64,
    payload_bytes: i64,
    rv: Rv,
    regen: Vec<String>,
    packed: Vec<String>,
    shebang: i64,
    data_files: i64,
    dropped: Vec<(String, String)>,
    site: String,
    record_source: Option<String>,
    editable: Option<EditableRec>,
    image_wheel: Option<(String, String)>,
}

impl Rec {
    fn json(&self) -> J {
        let rv = &self.rv;
        let mut items: Vec<(&str, J)> = vec![
            ("name", J::s(&self.name)),
            ("version", J::s(&self.version)),
            ("dist_info", J::s(&self.dist_info)),
            ("wheel", J::opt(&self.wheel)),
            ("source", J::opt(&self.source)),
            ("file_count", J::Int(self.file_count)),
            ("payload_bytes", J::Int(self.payload_bytes)),
            (
                "record_verify",
                obj(vec![
                    ("result", J::opt(&rv.result)),
                    ("verified", J::Int(rv.verified)),
                    ("unhashed", J::strs(&rv.unhashed)),
                    ("mismatches", J::strs(&rv.mismatches)),
                    ("missing", J::strs(&rv.missing)),
                    ("shared_path_omitted", J::Arr(rv.shared.iter().map(|(p, o)| obj(vec![("path", J::s(p)), ("owner", J::s(o))])).collect())),
                    (
                        "accepted",
                        J::Arr(rv.accepted.iter().map(|(p, r, v)| obj(vec![("path", J::s(p)), ("reason", J::s(r)), ("provenance", J::s(v))])).collect()),
                    ),
                ]),
            ),
            (
                "scripts",
                obj(vec![
                    ("regenerated_entry_points", J::strs(&self.regen)),
                    ("packed", J::strs(&self.packed)),
                    ("shebang_rewritten", J::Int(self.shebang)),
                ]),
            ),
            ("data_files", J::Int(self.data_files)),
            ("dropped", J::Arr(self.dropped.iter().map(|(p, r)| obj(vec![("path", J::s(p)), ("reason", J::s(r))])).collect())),
            ("site", J::s(&self.site)),
            ("record_source", J::opt(&self.record_source)),
        ];
        if let Some(e) = &self.editable {
            items.push((
                "editable",
                obj(vec![
                    ("direct_url", J::opt(&e.direct_url)),
                    ("mapping", J::Obj(e.mapping.iter().map(|(k, v)| (k.clone(), J::s(v))).collect())),
                    ("tree_files", J::Int(e.tree_files)),
                    ("tree_verify", J::s("n/a(소스 트리는 RECORD 가 없다 — 설치 트리 그대로 싣고 파일 목록을 기재)")),
                ]),
            ));
        }
        if let Some((path, sha)) = &self.image_wheel {
            items.push((
                "image_wheel",
                obj(vec![("path", J::s(path)), ("sha256", J::s(sha)), ("matched", J::s("direct_url.archive_info"))]),
            ));
        }
        obj(items)
    }
}

fn failure(items: Vec<(&str, J)>) -> J {
    obj(items)
}

fn s_of(v: &str) -> J {
    J::s(v)
}

// ── ld.so.conf ──

fn ld_conf_dirs(src: &Source) -> Result<Vec<String>, Err> {
    fn parse(src: &Source, path: &str, depth: i32, out: &mut Vec<String>) -> Result<(), Err> {
        let real = src.resolve(path);
        if depth > 8 || !src.is_file(&real) {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&std::fs::read(src.fetch(real.as_ref().unwrap())?)?).into_owned();
        for ln in splitlines(&text) {
            let ln = ln.split('#').next().unwrap_or("").trim();
            if ln.is_empty() {
                continue;
            }
            if ln.starts_with("include") {
                let Some(pat) = ln.split_whitespace().nth(1).map(|_| ln.splitn(2, char::is_whitespace).nth(1).unwrap_or("").trim().to_string()) else {
                    return Err(Err::Crash("IndexError: list index out of range".into()));
                };
                let pat = if pat.starts_with('/') { pat } else { join(&dirname(path), &pat) };
                let hits: Vec<String> = src.index.keys().filter(|p| fnmatch(p, &pat)).cloned().collect();
                for p in hits {
                    parse(src, &p, depth + 1, out)?;
                }
            } else {
                out.push(ln.trim_end_matches('/').to_string());
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    parse(src, "/etc/ld.so.conf", 0, &mut out)?;
    Ok(out)
}

fn dpkg_rows(src: &Source, dsite: &str, dist_info: &str) -> Result<(Option<String>, Vec<(String, String)>), Err> {
    let want = format!("{dsite}/{dist_info}");
    for lp in src.index.keys().filter(|p| p.starts_with("/var/lib/dpkg/info/") && p.ends_with(".list")) {
        let real = src.resolve(lp);
        if !src.is_file(&real) {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(src.fetch(real.as_ref().unwrap())?)?).into_owned();
        let lines = splitlines(&text);
        if !lines.contains(&want.as_str()) {
            continue;
        }
        let pkg = &basename(lp)[..basename(lp).len() - ".list".len()];
        let mut md5: HashMap<String, String> = HashMap::new();
        let mp = src.resolve(&format!("/var/lib/dpkg/info/{pkg}.md5sums"));
        if src.is_file(&mp) {
            let t = String::from_utf8_lossy(&std::fs::read(src.fetch(mp.as_ref().unwrap())?)?).into_owned();
            for ln in splitlines(&t) {
                if let Some((hx, path)) = ln.split_once("  ") {
                    let hx = hx.trim();
                    if hx.len() % 2 != 0 || !hx.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(Err::Crash("ValueError: non-hexadecimal number found in fromhex() arg".into()));
                    }
                    let bytes: Vec<u8> = (0..hx.len() / 2).map(|i| u8::from_str_radix(&hx[2 * i..2 * i + 2], 16).unwrap()).collect();
                    md5.insert(format!("/{}", path.trim().trim_start_matches('/')), format!("md5={}", b64url(&bytes)));
                }
            }
        }
        let mut rows = Vec::new();
        for ln in &lines {
            if ln.starts_with(&format!("{dsite}/")) && src.is_file(&src.resolve(ln)) {
                rows.push((relpath(ln, dsite), md5.get(*ln).cloned().unwrap_or_default()));
            }
        }
        return Ok((Some(pkg.to_string()), rows));
    }
    Ok((None, vec![]))
}

// ── the runtime-lib closure walk ──

struct BEnt {
    sonames: Vec<String>,
    reason: String,
    needed_by: Vec<String>,
}
#[derive(Default)]
struct OMap {
    items: Vec<(String, BEnt)>,
    idx: HashMap<String, usize>,
}
impl OMap {
    fn add(&mut self, real: &str, soname: &str, reason: &str, by: &str) {
        let i = match self.idx.get(real) {
            Some(i) => *i,
            None => {
                self.items.push((real.to_string(), BEnt { sonames: vec![], reason: reason.to_string(), needed_by: vec![] }));
                self.idx.insert(real.to_string(), self.items.len() - 1);
                self.items.len() - 1
            }
        };
        let e = &mut self.items[i].1;
        if !e.sonames.iter().any(|s| s == soname) {
            e.sonames.push(soname.to_string());
        }
        if e.needed_by.len() < 3 && !e.needed_by.iter().any(|b| b == by) {
            e.needed_by.push(by.to_string());
        }
    }
}

struct Walker<'a> {
    src: &'a Source,
    img_ld: &'a [String],
    conf_dirs: &'a [String],
    defaults: &'a [String],
    hostcache: &'a HashSet<String>,
    staged: &'a [(String, PathBuf)],
    cuda_real: Option<String>,
    packaged_ips: HashSet<String>,
    queue: Vec<(String, PathBuf)>,
    seen: HashSet<String>,
    bundled: OMap,
    classes: BTreeMap<String, String>,
    unresolved: BTreeMap<String, Vec<String>>,
    covered_wheel: i64,
    covered_cuda: i64,
    warnings: Vec<J>,
    rcache: HashMap<(String, Vec<String>), Option<String>>,
}

impl<'a> Walker<'a> {
    fn search(&mut self, name: &str, origin: &str, dyn_: &ElfDyn) -> Option<String> {
        let sub = |p: &str| p.replace("${ORIGIN}", origin).replace("$ORIGIN", origin);
        let mut dirs: Vec<String> = Vec::new();
        if dyn_.runpath.is_empty() {
            dirs.extend(dyn_.rpath.iter().map(|x| sub(x)));
        }
        dirs.extend(self.img_ld.iter().cloned());
        dirs.extend(dyn_.runpath.iter().map(|x| sub(x)));
        dirs.extend(self.conf_dirs.iter().cloned());
        dirs.extend(self.defaults.iter().cloned());
        let key = (name.to_string(), dirs.clone());
        if let Some(h) = self.rcache.get(&key) {
            return h.clone();
        }
        let mut hit = None;
        for dd in &dirs {
            let r = self.src.resolve(&format!("{}/{}", normpath(dd), name));
            if self.src.is_file(&r) {
                hit = r;
                break;
            }
        }
        self.rcache.insert(key, hit.clone());
        hit
    }

    fn classify_host(&self, name: &str) -> Option<&'static str> {
        if host_glibc(name) {
            Some("host-glibc")
        } else if host_driver(name) {
            Some("host-driver")
        } else if host_python(name) {
            Some("host-python")
        } else if kernel_abi(name) && self.hostcache.contains(name) {
            Some("host-kernel-abi")
        } else {
            None
        }
    }

    fn walk_queue(&mut self) -> Result<(), Err> {
        while let Some((ip, lp)) = self.queue.pop() {
            if self.seen.contains(&ip) {
                continue;
            }
            self.seen.insert(ip.clone());
            let Some(dyn_) = elf_dynamic(&lp) else { continue };
            if let Some(e) = &dyn_.error {
                self.warnings.push(obj(vec![("kind", s_of("elf-unparsed")), ("path", J::s(&ip)), ("detail", J::s(e))]));
            }
            let origin = dirname(&ip);
            for n in &dyn_.needed {
                if let Some(hc) = self.classify_host(n) {
                    self.classes.insert(n.clone(), hc.to_string());
                    continue;
                }
                let Some(r) = self.search(n, &origin, &dyn_) else {
                    if self.hostcache.contains(n) {
                        self.classes.insert(n.clone(), "host-ldcache".to_string());
                    } else {
                        let l = self.unresolved.entry(n.clone()).or_default();
                        if l.len() < 3 {
                            l.push(ip.clone());
                        }
                    }
                    continue;
                };
                if self.staged.iter().any(|(sd, _)| r.starts_with(&format!("{sd}/"))) {
                    self.covered_wheel += 1;
                    if !self.packaged_ips.contains(&r) && !self.seen.contains(&r) {
                        self.warnings.push(obj(vec![("kind", s_of("site-lib-not-packaged")), ("path", J::s(&r)), ("needed_by", J::s(&ip))]));
                    }
                    let p = self.src.fetch(&r)?;
                    self.queue.push((r, p));
                    continue;
                }
                if self.cuda_real.as_ref().map_or(false, |c| r.starts_with(&format!("{c}/"))) {
                    self.covered_cuda += 1;
                    let p = self.src.fetch(&r)?;
                    self.queue.push((r, p));
                    continue;
                }
                self.bundled.add(&r, n, "DT_NEEDED", &ip);
                let p = self.src.fetch(&r)?;
                self.queue.push((r, p));
            }
        }
        Ok(())
    }
}

fn host_ldcache() -> HashSet<String> {
    let Ok(o) = std::process::Command::new("ldconfig").arg("-p").output() else { return HashSet::new() };
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    splitlines(&text)
        .into_iter()
        .skip(1)
        .filter(|l| l.contains("=>"))
        .map(|l| l.trim().split(' ').next().unwrap_or("").to_string())
        .collect()
}

fn wheel_txt(has_so: bool, tags: &[String]) -> String {
    format!(
        "Wheel-Version: 1.0\nGenerator: {TOOL}\nRoot-Is-Purelib: {}\n{}",
        if has_so { "false" } else { "true" },
        tags.iter().map(|t| format!("Tag: {t}\n")).collect::<String>()
    )
}

fn warn(items: Vec<(&str, J)>) -> J {
    obj(items)
}

// ───────────────────────────── build ─────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn build(
    src: &mut Source,
    out: &Path,
    sites: &[String],
    cuda_link: &str,
    generated_utc: &str,
    accept: &Accept,
    jobs: usize,
    staging: Option<&Path>,
    accept_meta: Option<&AcceptMeta>,
) -> Result<J, Err> {
    let t0 = std::time::Instant::now();
    if let Some(am) = accept_meta {
        let ids: Vec<String> = if am.image_ids.is_empty() { vec![am.image_id.clone()] } else { am.image_ids.clone() };
        if !ids.contains(&src.meta.id) {
            return wh(format!("승인 파일의 이미지 {ids:?} ∌ 재포장 대상 {} — 다른 이미지의 승인은 쓰지 않는다", src.meta.id));
        }
    }
    let mut accept_used: BTreeSet<String> = BTreeSet::new();
    let wh_dir = out.join("wheelhouse");
    let rl = out.join("runtime-lib");
    for d in [&wh_dir, &rl] {
        if d.exists() {
            return Err(Err::Crash(format!("FileExistsError: {}", d.display())));
        }
        std::fs::create_dir_all(d)?;
    }
    let image_ref = format!("image@{}", src.meta.id);
    let mut failures: Vec<J> = Vec::new();
    let mut warnings: Vec<J> = Vec::new();
    let site = sites[0].clone();
    let probe = format!("{site}/");
    let py_tag = {
        let mut found = None;
        let mut from = 0;
        while let Some(i) = probe[from..].find("/python") {
            let rest = &probe[from + i + 7..];
            let mut it = rest.splitn(2, '.');
            let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
            let nd: String = b.chars().take_while(|c| c.is_ascii_digit()).collect();
            if a.len() == 1 && a.bytes().all(|c| c.is_ascii_digit()) && !nd.is_empty() && b[nd.len()..].starts_with('/') {
                found = Some(format!("cp{a}{nd}"));
                break;
            }
            from += i + 1;
        }
        match found {
            Some(t) => t,
            None => return wh(format!("site-packages 경로에서 파이썬 버전을 파생하지 못했다: {site}")),
        }
    };
    let arch_raw = src.meta.architecture.clone().unwrap_or_default();
    let arch = match arch_raw.as_str() {
        "arm64" => "aarch64".to_string(),
        "amd64" => "x86_64".to_string(),
        o => o.to_string(),
    };
    let plat_tag = format!("linux_{arch}");
    let triplet = format!("{arch}-linux-gnu");

    let mut staged: Vec<(String, PathBuf)> = Vec::new();
    let mut sites_note: Vec<J> = Vec::new();
    for (i, sd) in sites.iter().enumerate() {
        let loc = match src.stage_site(sd) {
            Ok(l) => l,
            Err(Err::Wh(e)) => {
                if i == 0 {
                    return Err(Err::Wh(e));
                }
                let t: Vec<char> = e.chars().collect();
                let tail: String = t[t.len().saturating_sub(200)..].iter().collect();
                sites_note.push(obj(vec![("site", J::s(sd)), ("present", J::Bool(false)), ("detail", J::s(&tail))]));
                continue;
            }
            Err(o) => return Err(o),
        };
        if !loc.is_dir() {
            if i == 0 {
                return wh(format!("주 site-packages 부재: {sd}"));
            }
            sites_note.push(obj(vec![("site", J::s(sd)), ("present", J::Bool(false))]));
            continue;
        }
        sites_note.push(obj(vec![("site", J::s(sd)), ("present", J::Bool(true))]));
        staged.push((sd.clone(), loc));
    }

    let mut dists: Vec<Dist> = Vec::new();
    let mut excluded: Vec<J> = Vec::new();
    let mut seen_canon: HashMap<String, String> = HashMap::new();
    for (sd, loc) in &staged {
        let pre = match sd.rfind("/lib/") {
            Some(i) => sd[..i].to_string(),
            None => sd.clone(),
        };
        let mut names: Vec<String> = std::fs::read_dir(loc)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for dn in names {
            let mut d = if dn.ends_with(".dist-info") {
                read_dist(loc, &dn)?
            } else if dn.ends_with(".egg-info") {
                let pk = loc.join(&dn).join("PKG-INFO");
                let text = if pk.exists() { Some(read_text_u(&pk)?) } else { None };
                let hn = text.as_deref().and_then(|t| header_get(t, "Name")).filter(|s| !s.is_empty());
                let hv = text.as_deref().and_then(|t| header_get(t, "Version")).filter(|s| !s.is_empty());
                Dist {
                    dist_info: dn.clone(),
                    name: hn.unwrap_or_else(|| dn.split('-').next().unwrap_or("").to_string()),
                    version: hv.unwrap_or_default(),
                    rows: vec![],
                    has_record: false,
                    tags: vec![],
                    entry_scripts: BTreeSet::new(),
                    direct_url: None,
                    editable: false,
                    egg_info: true,
                    site: String::new(),
                    site_local: PathBuf::new(),
                    prefix: String::new(),
                    key: String::new(),
                    record_source: None,
                }
            } else {
                continue;
            };
            d.site = sd.clone();
            d.site_local = loc.clone();
            d.prefix = pre.clone();
            d.key = format!("{sd}/{dn}");
            let c = canon(&d.name);
            if let Some(prev) = seen_canon.get(&c) {
                excluded.push(obj(vec![
                    ("name", J::s(&d.name)),
                    ("version", J::s(&d.version)),
                    ("dist_info", J::s(&d.key)),
                    ("reason", J::s(&format!("shadowed — 앞선 sys.path 의 {prev} 가 이긴다(이미지에서도 import 되지 않는다)"))),
                    ("provenance", s_of("derived:site-order")),
                ]));
                continue;
            }
            seen_canon.insert(c, d.key.clone());
            if d.egg_info {
                failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("egg-info-unsupported")), ("path", J::s(&d.key))]));
                continue;
            }
            dists.push(d);
        }
    }

    // 1) index + the RECORD files outside the tree · ld.so.conf · dpkg lists
    let mut outside: HashSet<String> = HashSet::new();
    for d in &dists {
        for (rel, _) in &d.rows {
            let ip = normpath(&join(&d.site, rel));
            if !ip.starts_with(&format!("{}/", d.site)) {
                outside.insert(ip);
            }
        }
    }
    let prefetch = move |p: &str| -> bool {
        outside.contains(p)
            || p == "/etc/ld.so.conf"
            || p.starts_with("/etc/ld.so.conf.d/")
            || (p.starts_with("/var/lib/dpkg/info/") && (p.ends_with(".list") || p.ends_with(".md5sums")))
    };
    src.build_index(&prefetch)?;
    let src_ro: &Source = src;

    for d in dists.iter_mut() {
        if d.has_record {
            d.record_source = Some("RECORD".into());
            continue;
        }
        let (pkg, rows) = dpkg_rows(src_ro, &d.site, &d.dist_info)?;
        if let (Some(pkg), false) = (pkg, rows.is_empty()) {
            d.rows = rows;
            d.has_record = true;
            d.record_source = Some(format!("dpkg:{pkg}.list+md5sums"));
        }
    }

    // 2) current-byte hashes
    let mut plan: HashMap<(String, String), (String, Option<String>)> = HashMap::new();
    let mut need_hash: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for d in &dists {
        for (rel, h) in &d.rows {
            if is_pyc(rel) || *rel == format!("{}/RECORD", d.dist_info) {
                continue;
            }
            let ip = normpath(&join(&d.site, rel));
            let real = src_ro.resolve(&ip);
            let is_f = src_ro.is_file(&real);
            plan.insert((d.key.clone(), rel.clone()), (ip, if is_f { real.clone() } else { None }));
            if is_f && !h.is_empty() {
                need_hash.entry(real.unwrap()).or_default().insert(h.split('=').next().unwrap().to_string());
            }
        }
    }
    let reals: Vec<&String> = need_hash.keys().collect();
    let hashed = par_map(&reals, jobs, |real| -> Result<(String, BTreeMap<String, String>), Err> {
        let p = src_ro.fetch(real)?;
        let mut m = BTreeMap::new();
        for a in &need_hash[*real] {
            m.insert(a.clone(), hash_file(&p, a)?.0);
        }
        Ok(((*real).clone(), m))
    });
    let mut cur_hash: HashMap<String, BTreeMap<String, String>> = HashMap::new();
    for r in hashed {
        let (real, hs) = r?;
        cur_hash.insert(real, hs);
    }
    let mut owners: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for d in &dists {
        for (rel, h) in &d.rows {
            owners.entry(normpath(&join(&d.site, rel))).or_default().push((d.key.clone(), h.clone()));
        }
    }
    let matches = |real: &Option<String>, h: &str| -> bool {
        let Some(real) = real else { return false };
        if h.is_empty() {
            return false;
        }
        let (algo, val) = h.split_once('=').unwrap_or((h, ""));
        cur_hash.get(real).and_then(|m| m.get(algo)).map_or(false, |v| v == val)
    };

    // 3) decisions per distribution
    let mut records: Vec<Rec> = Vec::new();
    struct Job {
        rec: usize,
        dest: PathBuf,
        entries: Vec<Entry>,
        distinfo: String,
    }
    let mut jobs_list: Vec<Job> = Vec::new();
    let mut elf_roots: Vec<(String, PathBuf)> = Vec::new();
    let image_wheels: HashSet<String> = src_ro.index.iter().filter(|(p, e)| p.ends_with(".whl") && e.0 == 'f').map(|(p, _)| p.clone()).collect();
    let stage_root: PathBuf = staging.map(|s| s.to_path_buf()).unwrap_or_else(|| out.join(".staging"));

    for d in &dists {
        let c = canon(&d.name);
        if c == "pip" {
            excluded.push(obj(vec![
                ("name", J::s(&d.name)),
                ("version", J::s(&d.version)),
                ("dist_info", J::s(&d.dist_info)),
                ("reason", J::s(EXCLUDE_PIP)),
                ("provenance", s_of("tripwire:EXCLUDE_DISTS")),
            ]));
            continue;
        }
        let mut rec = Rec {
            name: d.name.clone(),
            version: d.version.clone(),
            dist_info: d.dist_info.clone(),
            wheel: None,
            source: None,
            file_count: 0,
            payload_bytes: 0,
            rv: Rv::default(),
            regen: vec![],
            packed: vec![],
            shebang: 0,
            data_files: 0,
            dropped: vec![],
            site: d.site.clone(),
            record_source: d.record_source.clone(),
            editable: None,
            image_wheel: None,
        };
        if !d.has_record {
            failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("record-absent")), ("detail", J::s(&format!("{}/RECORD 없음", d.dist_info)))]));
            rec.rv.result = Some("FAIL".into());
            records.push(rec);
            continue;
        }
        let datadir = format!("{}-{}.data", whl_escape(&d.name), d.version);
        let mut entries: Vec<Entry> = Vec::new();
        for (rel, h) in &d.rows {
            let base = basename(rel).to_string();
            if is_pyc(rel) || *rel == format!("{}/RECORD", d.dist_info) {
                continue;
            }
            if rel.starts_with(&format!("{}/", d.dist_info)) && INSTALL_MARKERS.contains(&base.as_str()) {
                rec.dropped.push((rel.clone(), "install-marker(pip 가 설치 시 다시 쓴다)".into()));
                continue;
            }
            if d.editable && (base.starts_with("__editable__") || (rel.len() >= 23 && rel.starts_with("__editable___") && rel.ends_with("_finder.py"))) {
                rec.dropped.push((rel.clone(), "editable-shim(실제 패키지 트리로 대체)".into()));
                continue;
            }
            let (ip, real) = plan[&(d.key.clone(), rel.clone())].clone();
            let Some(real) = real else {
                rec.rv.missing.push(rel.clone());
                failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("record-file-missing")), ("path", J::s(rel))]));
                continue;
            };
            let real_o = Some(real.clone());
            if !h.is_empty() {
                if matches(&real_o, h) {
                    rec.rv.verified += 1;
                } else {
                    let owner = owners
                        .get(&ip)
                        .and_then(|v| v.iter().find(|(o, oh)| *o != d.key && matches(&real_o, oh)).map(|(o, _)| o.clone()));
                    let key = format!("{c}:{rel}");
                    if let Some(owner) = owner {
                        rec.rv.shared.push((rel.clone(), owner));
                        continue;
                    }
                    if let Some((reason, prov)) = accept.get(&key) {
                        accept_used.insert(key);
                        rec.rv.accepted.push((rel.clone(), reason.clone(), prov.clone()));
                    } else {
                        rec.rv.mismatches.push(rel.clone());
                        failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("record-hash-mismatch")), ("path", J::s(rel))]));
                    }
                }
            } else {
                rec.rv.unhashed.push(rel.clone());
            }
            let local = src_ro.fetch(&real)?;
            let mode = std::fs::metadata(&local)?.mode();
            if ip.starts_with(&format!("{}/", d.site)) {
                let arc = relpath(&ip, &d.site);
                if looks_shared_object(&arc, mode) {
                    elf_roots.push((ip.clone(), local.clone()));
                }
                entries.push(Entry { arc, content: Content::Path(local), mode });
            } else if ip.starts_with(&format!("{}/bin/", d.prefix)) {
                if d.entry_scripts.contains(&base) {
                    rec.regen.push(base);
                    continue;
                }
                let mut data = std::fs::read(&local)?;
                if data.starts_with(b"#!") {
                    let nl = data.iter().position(|&b| b == b'\n');
                    let first = &data[..nl.unwrap_or(data.len())];
                    if first.windows(6).any(|w| w == b"python") {
                        let mut nd = b"#!python\n".to_vec();
                        if let Some(n) = nl {
                            nd.extend_from_slice(&data[n + 1..]);
                        }
                        data = nd;
                        rec.shebang += 1;
                    }
                }
                entries.push(Entry { arc: format!("{datadir}/scripts/{base}"), content: Content::Bytes(data), mode: mode | 0o755 });
                rec.packed.push(base);
            } else if ip.starts_with(&format!("{}/", d.prefix)) {
                entries.push(Entry { arc: format!("{datadir}/data/{}", relpath(&ip, &d.prefix)), content: Content::Path(local), mode });
                rec.data_files += 1;
            } else {
                failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("unplaceable-path")), ("path", J::s(rel))]));
            }
        }
        let mut tags = d.tags.clone();
        if d.editable {
            let mapping = editable_mapping(&d.site_local, d)?;
            if mapping.is_empty() {
                failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("editable-mapping-absent")), ("detail", s_of("finder MAPPING/direct_url 없음"))]));
            }
            rec.editable = Some(EditableRec {
                direct_url: d.direct_url.as_ref().and_then(|x| x.get("url")).and_then(|x| x.str()).map(String::from),
                mapping: mapping.clone(),
                tree_files: 0,
            });
            let mut has_elf = false;
            let mut sorted_map = mapping.clone();
            sorted_map.sort();
            for (top, srcdir) in &sorted_map {
                let Some(real) = src_ro.resolve(srcdir) else {
                    failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("editable-source-missing")), ("path", J::s(srcdir))]));
                    continue;
                };
                let tdest = stage_root.join("editable").join(top);
                src_ro.fetch_tree(&real, &tdest)?;
                let mut collected: Vec<(PathBuf, Vec<String>)> = Vec::new();
                walk(&tdest, &mut |dp, dns, fns| {
                    dns.retain(|x| x != "__pycache__");
                    dns.sort();
                    fns.sort();
                    collected.push((dp.to_path_buf(), fns.clone()));
                });
                for (dp, fns) in collected {
                    for f in fns {
                        if f.ends_with(".pyc") || f.ends_with(".pyo") {
                            continue;
                        }
                        let mut lp = dp.join(&f);
                        let relp = relpath(&lp.to_string_lossy(), &tdest.to_string_lossy());
                        if lp.symlink_metadata().map_or(false, |m| m.file_type().is_symlink()) {
                            let tgt = src_ro.resolve(&format!("{real}/{relp}"));
                            if !src_ro.is_file(&tgt) {
                                failures.push(failure(vec![("dist", J::s(&d.name)), ("kind", s_of("editable-dangling-symlink")), ("path", J::s(&lp.to_string_lossy()))]));
                                continue;
                            }
                            lp = src_ro.fetch(tgt.as_ref().unwrap())?;
                        }
                        let mode = std::fs::metadata(&lp)?.mode();
                        entries.push(Entry { arc: format!("{top}/{relp}"), content: Content::Path(lp.clone()), mode });
                        rec.editable.as_mut().unwrap().tree_files += 1;
                        if looks_shared_object(&f, mode) {
                            elf_roots.push((format!("{real}/{relp}"), lp));
                            has_elf = has_elf || f.ends_with(".so");
                        }
                    }
                }
            }
            tags = vec![if has_elf { format!("{py_tag}-{py_tag}-{plat_tag}") } else { "py3-none-any".to_string() }];
            let wt = wheel_txt(has_elf, &tags);
            let wheel_arc = format!("{}/WHEEL", d.dist_info);
            entries.retain(|e| e.arc != wheel_arc);
            entries.push(Entry { arc: wheel_arc, content: Content::Bytes(wt.into_bytes()), mode: 0o644 });
            rec.source = Some(format!("editable-repacked({image_ref})"));
        } else {
            if tags.is_empty() {
                let has_so = entries.iter().any(|e| e.arc.ends_with(".so") || e.arc.contains(".so."));
                tags = vec![if has_so { format!("{py_tag}-{py_tag}-{plat_tag}") } else { "py3-none-any".to_string() }];
                entries.push(Entry { arc: format!("{}/WHEEL", d.dist_info), content: Content::Bytes(wheel_txt(has_so, &tags).into_bytes()), mode: 0o644 });
                warnings.push(warn(vec![("dist", J::s(&d.name)), ("kind", s_of("wheel-file-absent")), ("detail", J::s(&format!("WHEEL 생성(Tag {})", tags[0])))]));
            }
            rec.source = Some(format!("repacked({image_ref})"));
        }
        rec.rv.result = Some(
            if !rec.rv.mismatches.is_empty() || !rec.rv.missing.is_empty() {
                "FAIL"
            } else if !rec.rv.accepted.is_empty() {
                "ACCEPTED"
            } else {
                "PASS"
            }
            .to_string(),
        );

        // the image's own wheel, when the install record names its hash and the installed tree agrees with RECORD
        let url = d.direct_url.as_ref().and_then(|x| x.get("url")).and_then(|x| x.str()).unwrap_or("");
        let arch_info = d.direct_url.as_ref().and_then(|x| x.get("archive_info"));
        let whl_ip = if url.starts_with("file://") && url.ends_with(".whl") { url["file://".len()..].replace("%2B", "+") } else { String::new() };
        let want = {
            let h1 = arch_info.and_then(|a| a.get("hashes")).and_then(|h| h.get("sha256")).and_then(|x| x.str()).unwrap_or("");
            if !h1.is_empty() {
                h1.to_string()
            } else {
                let hh = arch_info.and_then(|a| a.get("hash")).and_then(|x| x.str()).unwrap_or("");
                if hh.starts_with("sha256=") { hh.rsplit('=').next().unwrap_or("").to_string() } else { String::new() }
            }
        };
        if image_wheels.contains(&whl_ip) && !want.is_empty() && rec.rv.result.as_deref() == Some("PASS") && rec.rv.shared.is_empty() {
            let lp = src_ro.fetch(&whl_ip)?;
            let got = sha256_hex_file(&lp)?;
            if got == want {
                let dst = wh_dir.join(basename(&whl_ip));
                std::fs::copy(&lp, &dst)?;
                rec.wheel = Some(basename(&whl_ip).to_string());
                rec.source = Some("wheel(image)".into());
                rec.file_count = entries.len() as i64;
                rec.image_wheel = Some((whl_ip.clone(), got));
                records.push(rec);
                continue;
            }
            warnings.push(warn(vec![("dist", J::s(&d.name)), ("kind", s_of("image-wheel-hash-differs")), ("detail", J::s(&format!("{whl_ip} {got} ≠ {want} → 재포장")))]));
        }
        let fname = format!("{}-{}-{}.whl", whl_escape(&d.name), d.version.replace('-', "_"), tag_string(&tags)?);
        rec.wheel = Some(fname.clone());
        rec.file_count = entries.len() as i64;
        records.push(rec);
        jobs_list.push(Job { rec: records.len() - 1, dest: wh_dir.join(&fname), entries, distinfo: d.dist_info.clone() });
    }

    let written = par_map(&jobs_list, jobs, |j| write_wheel(&j.dest, &j.entries, &j.distinfo));
    for (j, r) in jobs_list.iter().zip(written) {
        let (n, b) = r?;
        records[j.rec].file_count = n as i64;
        records[j.rec].payload_bytes = b as i64;
    }
    drop(jobs_list);

    // 4) runtime-lib: the CUDA tree (compat included) + DT_NEEDED closure + dlopen families
    let mut cuda_real = src_ro.resolve(cuda_link);
    if cuda_real.as_ref().map_or(true, |c| src_ro.index.get(c).map_or(true, |e| e.0 != 'd')) {
        failures.push(failure(vec![("kind", s_of("cuda-tree-absent")), ("path", J::s(cuda_link))]));
        cuda_real = None;
    } else {
        src_ro.fetch_tree(cuda_real.as_ref().unwrap(), &rl.join("cuda"))?;
    }
    // the mount is applied by re-borrowing the source mutably once the shared borrow of the walk phase is over
    let cuda_real_c = cuda_real.clone();
    let _ = src_ro;
    if let Some(c) = &cuda_real_c {
        src.add_mount(c, &rl.join("cuda"));
    }
    let src_ro: &Source = src;

    let mut env: BTreeMap<String, String> = BTreeMap::new();
    for e in &src_ro.meta.env {
        if let Some((k, v)) = e.split_once('=') {
            env.insert(k.to_string(), v.to_string());
        }
    }
    let img_ld: Vec<String> = env.get("LD_LIBRARY_PATH").map(|v| v.split(':').filter(|p| !p.is_empty()).map(|p| p.trim_end_matches('/').to_string()).collect()).unwrap_or_default();
    let conf_dirs = ld_conf_dirs(src_ro)?;
    let defaults: Vec<String> = vec![format!("/lib/{triplet}"), format!("/usr/lib/{triplet}"), "/lib".into(), "/usr/lib".into()];
    let hostcache = host_ldcache();
    let packaged_ips: HashSet<String> = elf_roots.iter().map(|(ip, _)| ip.clone()).collect();
    let mut w = Walker {
        src: src_ro,
        img_ld: &img_ld,
        conf_dirs: &conf_dirs,
        defaults: &defaults,
        hostcache: &hostcache,
        staged: &staged,
        cuda_real: cuda_real.clone(),
        packaged_ips,
        queue: elf_roots.clone(),
        seen: HashSet::new(),
        bundled: OMap::default(),
        classes: BTreeMap::new(),
        unresolved: BTreeMap::new(),
        covered_wheel: 0,
        covered_cuda: 0,
        warnings: Vec::new(),
        rcache: HashMap::new(),
    };
    w.walk_queue()?;
    // dlopen families: siblings of a bundled parent
    let snapshot: Vec<String> = w.bundled.items.iter().map(|(r, _)| r.clone()).collect();
    for real in snapshot {
        let sonames = w.bundled.items[w.bundled.idx[&real]].1.sonames.clone();
        for son in sonames {
            let Some(n) = son.strip_prefix("libcudnn.so.").filter(|n| digits(n)) else { continue };
            let ddir = dirname(&real);
            let sibs = w.src.listdir(&ddir);
            for sib in sibs {
                let ok = sib.strip_prefix("libcudnn_").and_then(|r| r.strip_suffix(&format!(".so.{n}"))).map_or(false, |m| !m.is_empty() && m.bytes().all(|c| c.is_ascii_lowercase() || c == b'_'));
                if ok {
                    let r = w.src.resolve(&format!("{ddir}/{sib}"));
                    if w.src.is_file(&r) {
                        let r = r.unwrap();
                        w.bundled.add(&r, &sib, &format!("dlopen-family:{son}"), &real);
                        let p = w.src.fetch(&r)?;
                        w.queue.push((r, p));
                    }
                }
            }
        }
    }
    let plug = env.get("NCCL_NET_PLUGIN").cloned().unwrap_or_default();
    if !plug.is_empty() && plug.to_lowercase() != "none" {
        let name = format!("libnccl-net-{plug}.so");
        let mut hit: Option<String> = None;
        for dd in img_ld.iter().chain(conf_dirs.iter()).chain(defaults.iter()) {
            let r = w.src.resolve(&format!("{}/{}", normpath(dd), name));
            if w.src.is_file(&r) {
                hit = r;
                break;
            }
        }
        match hit {
            Some(r) => {
                w.bundled.add(&r, &name, "dlopen:image-env NCCL_NET_PLUGIN", "env");
                let p = w.src.fetch(&r)?;
                w.queue.push((r, p));
            }
            None => w.warnings.push(warn(vec![("kind", s_of("nccl-plugin-not-found")), ("detail", J::s(&name))])),
        }
    }
    w.walk_queue()?;
    let Walker { bundled, classes, unresolved, covered_wheel, covered_cuda, warnings: wwarn, .. } = w;
    warnings.extend(wwarn);

    let libdir = rl.join("lib");
    std::fs::create_dir(&libdir)?;
    let mut bundled_sorted: Vec<&(String, BEnt)> = bundled.items.iter().collect();
    bundled_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut bundled_out: Vec<J> = Vec::new();
    for (real, ent) in bundled_sorted {
        let first = &ent.sonames[0];
        let lp = src_ro.fetch(real)?;
        std::fs::copy(&lp, libdir.join(first))?;
        if let Ok(m) = std::fs::metadata(&lp) {
            let _ = std::fs::set_permissions(libdir.join(first), std::fs::Permissions::from_mode(m.mode() & 0o7777));
        }
        for other in &ent.sonames[1..] {
            if !libdir.join(other).exists() {
                std::os::unix::fs::symlink(first, libdir.join(other))?;
            }
        }
        let (dg, sz) = hash_file(&libdir.join(first), "sha256")?;
        bundled_out.push(obj(vec![
            ("sonames", J::strs(&ent.sonames)),
            ("image_path", J::s(real)),
            ("sha256", J::s(&dg)),
            ("bytes", J::Int(sz as i64)),
            ("reason", J::s(&ent.reason)),
            ("needed_by", J::strs(&ent.needed_by)),
            ("provenance", J::s(&format!("copied({image_ref})"))),
        ]));
    }

    // LD_LIBRARY_PATH template (image env order mapped to venv / bundle coordinates)
    let mut compat_dir: Option<String> = None;
    let mut template: Vec<(Option<String>, J)> = Vec::new();
    let in_cuda = |r: &str, c: &str| r == c || r.starts_with(&format!("{c}/"));
    for p in &img_ld {
        if p.starts_with(&format!("{site}/")) {
            let path = format!("{{VENV_SITE}}/{}", relpath(p, &site));
            template.push((Some(path.clone()), obj(vec![("path", J::s(&path)), ("from", J::s(p))])));
        } else if p.trim_end_matches('/').ends_with("/compat/lib") && cuda_real.is_some() {
            let cr = cuda_real.as_ref().unwrap();
            let x = if std::path::Path::new("/dev/nvgpu").exists() { "lib.real-nvgpu" } else { "lib.real" };
            if src_ro.resolve(&format!("{cr}/compat/{x}")).is_some() {
                let path = format!("{{OUT}}/runtime-lib/cuda/compat/{x}");
                compat_dir = Some(path.clone());
                template.push((Some(path.clone()), obj(vec![("path", J::s(&path)), ("from", J::s(p)), ("compat", J::Bool(true))])));
            }
        } else {
            let r = src_ro.resolve(p);
            match (&r, &cuda_real) {
                (Some(r), Some(cr)) if in_cuda(r, cr) => {
                    let path = format!("{{OUT}}/runtime-lib/cuda/{}", relpath(r, cr));
                    template.push((Some(path.clone()), obj(vec![("path", J::s(&path)), ("from", J::s(p))])));
                }
                _ => template.push((
                    None,
                    obj(vec![("path", J::Null), ("from", J::s(p)), ("dropped", s_of("이미지 밖 주입 경로(컨테이너 툴킷) 또는 부재 — 호스트 로더가 대신한다"))]),
                )),
            }
        }
    }
    template.push((Some("{OUT}/runtime-lib/lib".into()), obj(vec![("path", s_of("{OUT}/runtime-lib/lib")), ("from", s_of("bundled DT_NEEDED closure"))])));
    for p in &conf_dirs {
        let r = src_ro.resolve(p);
        if let (Some(r), Some(cr)) = (&r, &cuda_real) {
            if in_cuda(r, cr) && !r.starts_with(&format!("{cr}/compat")) {
                let q = format!("{{OUT}}/runtime-lib/cuda/{}", relpath(r, cr));
                if template.iter().all(|(t, _)| t.as_deref() != Some(q.as_str())) {
                    template.push((Some(q.clone()), obj(vec![("path", J::s(&q)), ("from", J::s(&format!("ld.so.conf:{p}")))])));
                }
            }
        }
    }
    let mut env_map: Vec<(String, J)> = Vec::new();
    for (k, v) in &env {
        if let Some(cr) = &cuda_real {
            if v.split(':').any(|seg| seg.starts_with(cuda_link) || seg.starts_with(cr.as_str())) && !["PATH", "LD_LIBRARY_PATH", "LIBRARY_PATH"].contains(&k.as_str()) {
                let mut pres = vec![cr.clone(), cuda_link.to_string()];
                pres.sort_by_key(|x| std::cmp::Reverse(x.len()));
                pres.dedup();
                let mut nv = v.clone();
                for pre in pres {
                    nv = nv.replace(&pre, "{OUT}/runtime-lib/cuda");
                }
                env_map.push((k.clone(), J::s(&nv)));
            }
        }
    }

    let mut unowned: Vec<String> = Vec::new();
    let listed: HashSet<String> = dists.iter().flat_map(|d| d.rows.iter().map(move |(rel, _)| normpath(&join(&d.site, rel)))).collect();
    for (sd, loc) in &staged {
        walk(loc, &mut |dp, dns, fns| {
            dns.retain(|x| x != "__pycache__" && !x.ends_with(".dist-info") && !x.ends_with(".egg-info"));
            for f in fns.iter() {
                let ip = format!("{sd}/{}", relpath(&dp.join(f).to_string_lossy(), &loc.to_string_lossy()));
                if !listed.contains(&ip) && !f.ends_with(".pyc") && !f.ends_with(".pyo") {
                    unowned.push(ip);
                }
            }
        });
    }

    let stale: Vec<String> = accept.keys().filter(|k| !accept_used.contains(*k)).cloned().collect();
    for k in &stale {
        warnings.push(warn(vec![("kind", s_of("stale-accept")), ("entry", J::s(k)), ("detail", s_of("승인 목록에 있으나 이번 재포장에서 불일치가 관측되지 않았다"))]));
        eprintln!("native_wheelhouse: WARNING stale-accept {k} — 승인 항목이 관측되지 않았다(선언 갱신 필요)");
    }
    let status = if failures.is_empty() { "PASS" } else { "FAIL" };
    let cnt = |f: &dyn Fn(&Rec) -> bool| records.iter().filter(|r| f(r)).count() as i64;
    let accept_j = match accept_meta {
        Some(am) => obj(vec![
            ("source", s_of("accept-file")),
            ("sha256", J::s(&am.sha256)),
            ("approved_by", J::s(&am.approved_by)),
            ("approved_utc", J::s(&am.approved_utc)),
            ("image_id", J::s(&am.image_id)),
            ("image_tag", J::opt(&am.image_tag)),
            ("image_ids", J::strs(&am.image_ids)),
            ("applied", J::strs(&accept_used)),
            ("stale", J::strs(&stale)),
        ]),
        None if !accept.is_empty() => obj(vec![("source", s_of("cli --accept-mismatch")), ("applied", J::strs(&accept_used)), ("stale", J::strs(&stale))]),
        None => J::Null,
    };
    unowned.sort();
    let meta = &src_ro.meta;
    let closure = obj(vec![
        ("schema_version", J::Int(1)),
        ("kind", s_of("native_wheelhouse_closure")),
        ("tool", s_of(TOOL)),
        ("generated_utc", J::s(generated_utc)),
        ("status", s_of(status)),
        ("failures", J::Arr(failures.clone())),
        ("warnings", J::Arr(warnings)),
        (
            "image",
            obj(vec![
                ("tag", J::opt(&meta.tag)),
                ("id", J::s(&meta.id)),
                ("repo_digests", J::strs(&meta.repo_digests)),
                ("architecture", J::opt(&meta.architecture)),
                ("os", J::opt(&meta.os)),
            ]),
        ),
        ("image_env", J::strs(&meta.env)),
        ("site_packages", J::Arr(sites_note)),
        ("python_tag", J::s(&py_tag)),
        ("platform_tag", J::s(&plat_tag)),
        (
            "provenance",
            obj(vec![
                ("method", s_of("repack-from-installed-tree(RECORD)")),
                ("recompiled", J::Bool(false)),
                ("network", J::Bool(false)),
                ("container_access", s_of("create(--pull never --network none)/cp/export/rm only")),
            ]),
        ),
        (
            "counts",
            obj(vec![
                ("distributions_seen", J::Int(dists.len() as i64)),
                ("wheels", J::Int(cnt(&|r| r.wheel.is_some()))),
                ("repacked", J::Int(cnt(&|r| r.source.as_deref().map_or(false, |s| s.starts_with("repacked"))))),
                ("editable_repacked", J::Int(cnt(&|r| r.source.as_deref().map_or(false, |s| s.starts_with("editable-repacked"))))),
                ("image_wheels", J::Int(cnt(&|r| r.source.as_deref() == Some("wheel(image)")))),
                ("excluded", J::Int(excluded.len() as i64)),
                ("record_mismatches", J::Int(records.iter().map(|r| r.rv.mismatches.len() as i64).sum())),
                ("shared_path_omitted", J::Int(records.iter().map(|r| r.rv.shared.len() as i64).sum())),
            ]),
        ),
        ("accept", accept_j),
        ("distributions", J::Arr(records.iter().map(|r| r.json()).collect())),
        ("excluded", J::Arr(excluded)),
        (
            "unowned_site_files",
            obj(vec![
                ("count", J::Int(unowned.len() as i64)),
                ("sample", J::strs(unowned.iter().take(40))),
                ("note", s_of("어느 RECORD 에도 없는 site-packages 파일 — 휠에 실리지 않는다(venv 에 없다)")),
            ]),
        ),
        (
            "runtime_lib",
            obj(vec![
                (
                    "cuda_tree",
                    obj(vec![
                        ("image_link", J::s(cuda_link)),
                        ("image_real", J::opt(&cuda_real)),
                        ("local", s_of("runtime-lib/cuda")),
                        ("compat_dir", J::opt(&compat_dir)),
                    ]),
                ),
                ("bundled", J::Arr(bundled_out)),
                ("host_provided", J::Obj(classes.iter().map(|(k, v)| (k.clone(), J::s(v))).collect())),
                ("unresolved", J::Obj(unresolved.iter().map(|(k, v)| (k.clone(), J::strs(v))).collect())),
                ("covered_needed_edges", obj(vec![("wheel", J::Int(covered_wheel)), ("cuda-tree", J::Int(covered_cuda))])),
                ("ld_library_path_template", J::Arr(template.into_iter().map(|(_, j)| j).collect())),
                ("env_template", J::Obj(env_map)),
                ("path_prepend_template", if cuda_real.is_some() { J::strs(["{OUT}/runtime-lib/cuda/bin"]) } else { J::Arr(vec![]) }),
            ]),
        ),
        ("sizes", obj(vec![("wheelhouse_bytes", J::Int(du(&wh_dir))), ("runtime_lib_bytes", J::Int(du(&rl)))])),
        ("elapsed_s_monotonic", J::Float((t0.elapsed().as_secs_f64() * 10.0).round() / 10.0)),
    ]);
    let mut req = vec![
        "# native_wheelhouse.py 생성 — 정확 핀(이미지 설치본 그대로) · pip install --no-index 전용".to_string(),
        format!("# image {} {} · generated_utc {generated_utc}", meta.tag.clone().unwrap_or_else(|| "None".into()), meta.id),
    ];
    let mut pins: Vec<String> = records.iter().filter(|r| r.wheel.is_some()).map(|r| format!("{}=={}", r.name, r.version)).collect();
    pins.sort();
    req.extend(pins);
    std::fs::write(out.join("requirements-closure.txt"), req.join("\n") + "\n")?;
    std::fs::write(out.join("closure.json"), dumps(&closure) + "\n")?;
    Ok(closure)
}
