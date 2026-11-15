//! The image file-system sources of `native_wheelhouse.py` (`Source`, `DirSource`, `DockerSource`), the ELF dynamic-section reader and the
//! installed-distribution metadata readers. Docker is touched only through the original's probe-only rule: `image inspect`, a never-started
//! `create --pull never --network none`, and `cp` / `export` / `rm` on the container this program created.

use crate::wheelhouse_util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufReader, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Meta {
    pub tag: Option<String>,
    pub id: String,
    pub repo_digests: Vec<String>,
    pub architecture: Option<String>,
    pub os: Option<String>,
    pub env: Vec<String>,
}

impl Meta {
    pub fn from_json(j: &J) -> Result<Meta, Err> {
        let id = j.get("id").and_then(|x| x.str()).ok_or_else(|| Err::Crash("KeyError: 'id'".into()))?.to_string();
        let strs = |k: &str| -> Vec<String> { j.get(k).map(|a| a.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default() };
        Ok(Meta {
            tag: j.get("tag").and_then(|x| x.str()).map(String::from),
            id,
            repo_digests: strs("repo_digests"),
            architecture: j.get("architecture").and_then(|x| x.str()).map(String::from),
            os: j.get("os").and_then(|x| x.str()).map(String::from),
            env: strs("env"),
        })
    }
}

pub enum Backend {
    Dir,
    Docker { image: String, cid: String },
}

pub struct Source {
    pub root: PathBuf,
    pub index: BTreeMap<String, (char, String)>,
    pub meta: Meta,
    mounts: Vec<(String, PathBuf)>,
    backend: Backend,
}

fn tail_chars(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

pub fn read_text_u(path: &Path) -> std::io::Result<String> {
    let b = std::fs::read(path)?;
    let s = String::from_utf8_lossy(&b).into_owned();
    Ok(if s.contains('\r') { s.replace("\r\n", "\n").replace('\r', "\n") } else { s })
}

impl Source {
    pub fn dir(root: &Path, meta: Meta) -> Source {
        Source { root: root.to_path_buf(), index: BTreeMap::new(), meta, mounts: Vec::new(), backend: Backend::Dir }
    }

    /// `DockerSource(image, staging)`: reads `docker image inspect`.
    pub fn docker(image: &str, staging: &Path) -> Result<Source, Err> {
        let mut s = Source {
            root: staging.join("rootfs"),
            index: BTreeMap::new(),
            meta: Meta { tag: None, id: String::new(), repo_digests: vec![], architecture: None, os: None, env: vec![] },
            mounts: Vec::new(),
            backend: Backend::Docker { image: image.to_string(), cid: String::new() },
        };
        let out = s.docker_cmd(&["image", "inspect", image])?;
        let j = loads(&out).map_err(|_| Err::Crash("JSONDecodeError: docker image inspect".into()))?;
        let info = j.arr().first().ok_or_else(|| Err::Crash("IndexError: list index out of range".into()))?;
        let cfg = info.get("Config");
        s.meta = Meta {
            tag: Some(image.to_string()),
            id: info.get("Id").and_then(|x| x.str()).ok_or_else(|| Err::Crash("KeyError: 'Id'".into()))?.to_string(),
            repo_digests: info.get("RepoDigests").map(|a| a.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default(),
            architecture: info.get("Architecture").and_then(|x| x.str()).map(String::from),
            os: info.get("Os").and_then(|x| x.str()).map(String::from),
            env: cfg.and_then(|c| c.get("Env")).map(|a| a.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default(),
        };
        Ok(s)
    }

    fn cid(&self) -> &str {
        match &self.backend {
            Backend::Docker { cid, .. } => cid,
            Backend::Dir => "",
        }
    }

    /// `DockerSource._docker`: the probe-only rule, then the call; stdout bytes on success.
    fn docker_cmd(&self, argv: &[&str]) -> Result<Vec<u8>, Err> {
        let cid = self.cid();
        let pair = |a: &str, b: &str| argv.windows(2).any(|w| w[0] == a && w[1] == b);
        let ok = match argv[0] {
            "image" => argv.get(1) == Some(&"inspect"),
            "create" => pair("--pull", "never") && pair("--network", "none"),
            "cp" | "export" | "rm" => !cid.is_empty() && argv[1..].iter().any(|a| *a == cid || a.starts_with(&format!("{cid}:"))),
            _ => false,
        };
        if !ok {
            return wh(format!("docker 호출 거부(탐침 전용 규칙): {}", argv.join(" ")));
        }
        let r = Command::new("docker").args(argv).stdin(Stdio::null()).output()?;
        if !r.status.success() {
            let err = String::from_utf8_lossy(&r.stderr);
            return wh(format!("docker {} rc={}: {}", argv.join(" "), r.status.code().unwrap_or(-1), tail_chars(err.trim(), 400)));
        }
        Ok(r.stdout)
    }

    pub fn start(&mut self) -> Result<(), Err> {
        let Backend::Docker { image, .. } = &self.backend else { return Ok(()) };
        let image = image.clone();
        let out = self.docker_cmd(&["create", "--pull", "never", "--network", "none", &image, "true"])?;
        let text = String::from_utf8_lossy(&out).into_owned();
        let cid = text.trim().lines().last().unwrap_or("").trim().to_string();
        if cid.len() != 64 || !cid.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return wh(format!("docker create 가 컨테이너 id 를 내지 않았다: {text:?}"));
        }
        if let Backend::Docker { cid: c, .. } = &mut self.backend {
            *c = cid;
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Err> {
        let cid = self.cid().to_string();
        if cid.is_empty() {
            return Ok(());
        }
        let r = self.docker_cmd(&["rm", "-f", "-v", &cid]);
        if let Backend::Docker { cid: c, .. } = &mut self.backend {
            c.clear();
        }
        r.map(|_| ())
    }

    /// Image coordinate → local path (the longest mount prefix first).
    pub fn local(&self, ipath: &str) -> PathBuf {
        let mut ms: Vec<&(String, PathBuf)> = self.mounts.iter().collect();
        ms.sort_by_key(|m| std::cmp::Reverse(m.0.len()));
        for (pre, loc) in ms {
            if ipath == pre || ipath.starts_with(&format!("{pre}/")) {
                let rest = ipath[pre.len()..].trim_start_matches('/');
                return if rest.is_empty() { loc.clone() } else { loc.join(rest) };
            }
        }
        let rest = ipath.trim_start_matches('/');
        if rest.is_empty() {
            self.root.clone()
        } else {
            self.root.join(rest)
        }
    }

    pub fn add_mount(&mut self, ipath: &str, loc: &Path) {
        self.mounts.push((ipath.to_string(), loc.to_path_buf()));
    }

    /// Follows symbolic links component by component; `None` when the path does not exist.
    pub fn resolve(&self, path: &str) -> Option<String> {
        let mut hops: i32 = 40;
        let mut comps: Vec<String> = path.split('/').filter(|c| !c.is_empty() && *c != ".").map(String::from).collect();
        let mut cur: Vec<String> = Vec::new();
        while !comps.is_empty() {
            let c = comps.remove(0);
            if c == ".." {
                cur.pop();
                continue;
            }
            let mut cand_parts = cur.clone();
            cand_parts.push(c.clone());
            let cand = format!("/{}", cand_parts.join("/"));
            let ent = self.index.get(&cand)?;
            if ent.0 == 'l' {
                hops -= 1;
                if hops < 0 {
                    return None;
                }
                let link = &ent.1;
                if link.starts_with('/') {
                    cur.clear();
                }
                let mut nc: Vec<String> = link.split('/').filter(|x| !x.is_empty() && *x != ".").map(String::from).collect();
                nc.extend(comps);
                comps = nc;
                continue;
            }
            cur.push(c);
        }
        Some(format!("/{}", cur.join("/")))
    }

    /// Sorted names directly under `ipath`.
    pub fn listdir(&self, ipath: &str) -> Vec<String> {
        let t = ipath.trim_end_matches('/');
        let prefix = format!("{t}/");
        let mut v: Vec<String> = self
            .index
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, _)| {
                let rest = &k[prefix.len()..];
                if rest.is_empty() || rest.contains('/') { None } else { Some(rest.to_string()) }
            })
            .collect();
        v.sort();
        v
    }

    pub fn is_file(&self, real: &Option<String>) -> bool {
        match real {
            Some(r) => self.index.get(r).map_or(false, |e| e.0 == 'f'),
            None => false,
        }
    }

    pub fn stage_site(&self, site: &str) -> Result<PathBuf, Err> {
        match &self.backend {
            Backend::Dir => Ok(self.local(site)),
            Backend::Docker { cid, .. } => {
                let dest = self.local(site);
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                self.docker_cmd(&["cp", &format!("{cid}:{site}"), &dest.to_string_lossy()])?;
                Ok(dest)
            }
        }
    }

    /// The index of every path, plus the regular files `prefetch` picks (one `docker export` stream for the Docker source).
    pub fn build_index(&mut self, prefetch: &dyn Fn(&str) -> bool) -> Result<(), Err> {
        if matches!(self.backend, Backend::Docker { .. }) {
            return self.build_index_docker(prefetch);
        }
        let mut idx: BTreeMap<String, (char, String)> = BTreeMap::new();
        walk_all(&self.root, &self.root, &mut idx)?;
        self.index = idx;
        Ok(())
    }

    pub fn fetch(&self, real: &str) -> Result<PathBuf, Err> {
        match &self.backend {
            Backend::Dir => Ok(self.local(real)),
            Backend::Docker { cid, .. } => {
                let dest = self.local(real);
                if dest.exists() {
                    return Ok(dest);
                }
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                self.docker_cmd(&["cp", &format!("{cid}:{real}"), &dest.to_string_lossy()])?;
                Ok(dest)
            }
        }
    }

    pub fn fetch_tree(&self, real: &str, dest: &Path) -> Result<(), Err> {
        match &self.backend {
            Backend::Dir => copytree(&self.local(real), dest),
            Backend::Docker { cid, .. } => {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                self.docker_cmd(&["cp", &format!("{cid}:{real}"), &dest.to_string_lossy()])?;
                Ok(())
            }
        }
    }
}

fn walk_all(root: &Path, dir: &Path, idx: &mut BTreeMap<String, (char, String)>) -> Result<(), Err> {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for e in rd {
        let e = e?;
        let full = e.path();
        let ip = format!("/{}", relpath(&full.to_string_lossy(), &root.to_string_lossy()));
        let md = std::fs::symlink_metadata(&full)?;
        let ft = md.file_type();
        if ft.is_symlink() {
            idx.insert(ip, ('l', std::fs::read_link(&full)?.to_string_lossy().into_owned()));
        } else if ft.is_dir() {
            idx.insert(ip, ('d', String::new()));
            walk_all(root, &full, idx)?;
        } else if ft.is_file() {
            idx.insert(ip, ('f', String::new()));
        } else {
            idx.insert(ip, ('o', String::new()));
        }
    }
    Ok(())
}

/// `shutil.copytree(src, dst, symlinks=True)`.
pub fn copytree(src: &Path, dst: &Path) -> Result<(), Err> {
    if dst.symlink_metadata().is_ok() {
        return Err(Err::Crash(format!("FileExistsError: {}", dst.display())));
    }
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        let ft = e.file_type()?;
        if ft.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(e.path())?, &to)?;
        } else if ft.is_dir() {
            copytree(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), &to)?;
        }
    }
    let md = std::fs::metadata(src)?;
    std::fs::set_permissions(dst, std::fs::Permissions::from_mode(md.mode() & 0o7777))?;
    Ok(())
}

// ───────────────────────────── docker export ─────────────────────────────

fn tar_name(m: &TarMember) -> String {
    format!("/{}", m.name.trim_start_matches(['.', '/']).trim_end_matches('/'))
}

impl Source {
    /// `DockerSource.build_index`: one `docker export` stream builds the index and extracts what `prefetch` selects.
    pub fn build_index_docker(&mut self, prefetch: &dyn Fn(&str) -> bool) -> Result<(), Err> {
        let cid = self.cid().to_string();
        if cid.is_empty() {
            return wh("docker export 는 시작한 컨테이너에서만 부른다");
        }
        self.docker_guard_export(&cid)?;
        let mut child = Command::new("docker").args(["export", &cid]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let mut err_pipe = child.stderr.take().unwrap();
        let drain = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = err_pipe.read_to_end(&mut b);
            b
        });
        let mut idx: BTreeMap<String, (char, String)> = BTreeMap::new();
        let mut rd = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
        let me: &Source = self;
        let result = tar_walk(
            &mut rd,
            |m, take| {
                let name = tar_name(m);
                if name == "/" {
                    return Ok(());
                }
                match m.kind {
                    Kind::Sym => {
                        idx.insert(name, ('l', m.linkname.clone()));
                    }
                    Kind::Dir => {
                        idx.insert(name, ('d', String::new()));
                    }
                    Kind::Reg => {
                        idx.insert(name.clone(), ('f', String::new()));
                        if prefetch(&name) {
                            let dest = me.local(&name);
                            if let Some(p) = dest.parent() {
                                std::fs::create_dir_all(p)?;
                            }
                            let mut out = std::fs::File::create(&dest)?;
                            take(&mut out)?;
                            drop(out);
                            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(m.mode & 0o777))?;
                        }
                    }
                    Kind::Hard => {
                        idx.insert(name, ('f', String::new()));
                    }
                    Kind::Other => {
                        idx.insert(name, ('o', String::new()));
                    }
                }
                Ok(())
            },
            |m| m.kind == Kind::Reg && prefetch(&tar_name(m)),
        );
        // drain whatever the stream still holds so the child can exit
        let mut sink = std::io::sink();
        let _ = std::io::copy(&mut rd, &mut sink);
        drop(rd);
        let status = child.wait()?;
        let stderr = drain.join().unwrap_or_default();
        result?;
        if !status.success() {
            return wh(format!(
                "docker export rc={}: {}",
                status.code().unwrap_or(-1),
                tail_chars(&String::from_utf8_lossy(&stderr), 400)
            ));
        }
        self.index = idx;
        Ok(())
    }

    fn docker_guard_export(&self, cid: &str) -> Result<(), Err> {
        if self.cid() == cid && !cid.is_empty() {
            Ok(())
        } else {
            wh(format!("docker 호출 거부(탐침 전용 규칙): export {cid}"))
        }
    }
}

// ───────────────────────────── ELF ─────────────────────────────

pub struct ElfDyn {
    pub needed: Vec<String>,
    pub rpath: Vec<String>,
    pub runpath: Vec<String>,
    pub error: Option<String>,
}

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_u64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// ELF64 LE `DT_NEEDED` / `RPATH` / `RUNPATH`; `None` when the file is not ELF.
pub fn elf_dynamic(path: &Path) -> Option<ElfDyn> {
    let empty = |e: &str| Some(ElfDyn { needed: vec![], rpath: vec![], runpath: vec![], error: Some(e.to_string()) });
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return empty(&format!("read:{e}")),
    };
    use std::io::{Seek, SeekFrom};
    let mut hdr = [0u8; 64];
    let n = read_upto(&mut f, &mut hdr);
    if n < 64 || &hdr[..4] != b"\x7fELF" {
        return None;
    }
    if hdr[4] != 2 || hdr[5] != 1 {
        return empty("not-elf64-le");
    }
    let shoff = rd_u64(&hdr, 0x28);
    let shentsize = rd_u16(&hdr, 0x3A) as usize;
    let shnum = rd_u16(&hdr, 0x3C) as usize;
    if shoff == 0 || shnum == 0 {
        return empty("no-section-headers");
    }
    let mut raw = vec![0u8; shentsize * shnum];
    let got = f.seek(SeekFrom::Start(shoff)).ok().map(|_| read_upto(&mut f, &mut raw)).unwrap_or(0);
    if got < raw.len() || shentsize < 64 {
        return empty("read:short section headers");
    }
    // (type, offset, size, link) of each section
    let secs: Vec<(u32, u64, u64, u32)> = (0..shnum)
        .map(|i| {
            let o = i * shentsize;
            (rd_u32(&raw, o + 4), rd_u64(&raw, o + 24), rd_u64(&raw, o + 32), rd_u32(&raw, o + 40))
        })
        .collect();
    let Some(d) = secs.iter().find(|s| s.0 == 6) else {
        return Some(ElfDyn { needed: vec![], rpath: vec![], runpath: vec![], error: None });
    };
    let Some(strsec) = secs.get(d.3 as usize) else { return empty("read:bad link") };
    let mut strtab = vec![0u8; strsec.2 as usize];
    let mut draw = vec![0u8; d.2 as usize];
    let ok = f.seek(SeekFrom::Start(strsec.1)).is_ok()
        && read_upto(&mut f, &mut strtab) == strtab.len()
        && f.seek(SeekFrom::Start(d.1)).is_ok();
    if !ok {
        return empty("read:short dynamic");
    }
    let got_d = read_upto(&mut f, &mut draw);
    draw.truncate(got_d);
    let s = |off: usize| -> String {
        let tail = &strtab[off.min(strtab.len())..];
        let end = tail.iter().position(|&x| x == 0).unwrap_or(tail.len());
        String::from_utf8_lossy(&tail[..end]).into_owned()
    };
    let mut out = ElfDyn { needed: vec![], rpath: vec![], runpath: vec![], error: None };
    let mut i = 0;
    while i + 16 <= draw.len() {
        let tag = i64::from_le_bytes(draw[i..i + 8].try_into().unwrap());
        let val = rd_u64(&draw, i + 8) as usize;
        if tag == 0 {
            break;
        }
        match tag {
            1 => out.needed.push(s(val)),
            15 => out.rpath.extend(s(val).split(':').filter(|x| !x.is_empty()).map(String::from)),
            29 => out.runpath.extend(s(val).split(':').filter(|x| !x.is_empty()).map(String::from)),
            _ => {}
        }
        i += 16;
    }
    Some(out)
}

fn read_upto<R: Read>(r: &mut R, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

// ───────────────────────────── dist-info ─────────────────────────────

pub fn canon(name: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !prev_sep {
                out.push('-');
            }
            prev_sep = true;
        } else {
            out.extend(c.to_lowercase());
            prev_sep = false;
        }
    }
    out
}

pub fn whl_escape(name: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !prev_sep {
                out.push('_');
            }
            prev_sep = true;
        } else {
            out.push(c);
            prev_sep = false;
        }
    }
    out
}

pub fn is_pyc(rel: &str) -> bool {
    rel.ends_with(".pyc") || rel.ends_with(".pyo") || format!("/{rel}").contains("/__pycache__/")
}

pub fn looks_shared_object(name: &str, mode: u32) -> bool {
    let mut t = name;
    loop {
        match t.rfind('.') {
            Some(i) if i + 1 < t.len() && t[i + 1..].bytes().all(|b| b.is_ascii_digit()) => t = &t[..i],
            _ => break,
        }
    }
    t.ends_with(".so") || mode & 0o111 != 0
}

/// `email.parser.HeaderParser().parsestr(text)` then `get(name)`: the first header of that name (case-insensitive).
pub fn header_get(text: &str, name: &str) -> Option<String> {
    let mut cur: Option<(String, String)> = None;
    let mut found: Vec<(String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        if bare.is_empty() {
            break;
        }
        if bare.starts_with([' ', '\t']) {
            match cur.as_mut() {
                Some((_, v)) => v.push_str(line),
                None => break,
            }
            continue;
        }
        let Some(colon) = bare.find(':') else { break };
        let key = &bare[..colon];
        if key.is_empty() || key.contains([' ', '\t']) {
            break;
        }
        if let Some(c) = cur.take() {
            found.push(c);
        }
        cur = Some((key.to_string(), line[colon + 1..].trim_start_matches([' ', '\t']).to_string()));
    }
    if let Some(c) = cur.take() {
        found.push(c);
    }
    found.into_iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim_end_matches(['\r', '\n']).to_string())
}

/// Option names of `[console_scripts]` / `[gui_scripts]` the way `configparser` (delimiters `=`, no interpolation, case kept) reads them;
/// a parse error leaves the set empty like the original.
pub fn entry_script_names(text: &str) -> BTreeSet<String> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_option = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let indented = raw.starts_with([' ', '\t']);
        if indented && in_option {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
            sections.push((line[1..line.len() - 1].to_string(), Vec::new()));
            in_option = false;
            continue;
        }
        let Some(cur) = sections.last_mut() else { return BTreeSet::new() };
        match line.find('=') {
            Some(i) => {
                cur.1.push(line[..i].trim().to_string());
                in_option = true;
            }
            None => return BTreeSet::new(),
        }
    }
    let mut out = BTreeSet::new();
    for (name, opts) in sections {
        if name == "console_scripts" || name == "gui_scripts" {
            out.extend(opts.into_iter().map(|k| k.trim().to_string()));
        }
    }
    out
}

pub struct Dist {
    pub dist_info: String,
    pub name: String,
    pub version: String,
    pub rows: Vec<(String, String)>,
    pub has_record: bool,
    pub tags: Vec<String>,
    pub entry_scripts: BTreeSet<String>,
    pub direct_url: Option<J>,
    pub editable: bool,
    pub egg_info: bool,
    pub site: String,
    pub site_local: PathBuf,
    pub prefix: String,
    pub key: String,
    pub record_source: Option<String>,
}

fn rsplit_last<'a>(s: &'a str, sep: char) -> &'a str {
    s.rsplit(sep).next().unwrap_or(s)
}

pub fn read_dist(site_local: &Path, dname: &str) -> Result<Dist, Err> {
    let di = site_local.join(dname);
    let mut meta_p = di.join("METADATA");
    if !meta_p.exists() {
        meta_p = di.join("PKG-INFO");
    }
    let text = if meta_p.exists() { Some(read_text_u(&meta_p)?) } else { None };
    let hn = text.as_deref().and_then(|t| header_get(t, "Name")).filter(|s| !s.is_empty());
    let hv = text.as_deref().and_then(|t| header_get(t, "Version")).filter(|s| !s.is_empty());
    let name = hn.unwrap_or_else(|| dname.rsplit_once('-').map(|x| x.0).unwrap_or(dname).to_string());
    let version = hv.unwrap_or_else(|| {
        let stem = &dname[..dname.len() - ".dist-info".len().min(dname.len())];
        rsplit_last(stem, '-').to_string()
    });
    let mut rows = Vec::new();
    let rec = di.join("RECORD");
    if rec.exists() {
        let t = String::from_utf8_lossy(&std::fs::read(&rec)?).into_owned();
        for row in csv_rows(&t) {
            if !row.is_empty() && !row[0].is_empty() {
                rows.push((row[0].clone(), row.get(1).cloned().unwrap_or_default()));
            }
        }
    }
    let mut tags = Vec::new();
    let whl = di.join("WHEEL");
    if whl.exists() {
        for ln in splitlines(&read_text_u(&whl)?) {
            if ln.starts_with("Tag:") {
                tags.push(ln.split_once(':').unwrap().1.trim().to_string());
            }
        }
    }
    let epf = di.join("entry_points.txt");
    let entry_scripts = if epf.exists() { entry_script_names(&read_text_u(&epf)?) } else { BTreeSet::new() };
    let du = di.join("direct_url.json");
    let direct = if du.exists() { loads(read_text_u(&du)?.as_bytes()).ok() } else { None };
    let editable = direct
        .as_ref()
        .map_or(false, |d| d.truthy() && d.get("dir_info").map_or(false, |x| x.get("editable").map_or(false, |e| e.truthy())));
    Ok(Dist {
        dist_info: dname.to_string(),
        name,
        version,
        rows,
        has_record: rec.exists(),
        tags,
        entry_scripts,
        direct_url: direct,
        editable,
        egg_info: false,
        site: String::new(),
        site_local: PathBuf::new(),
        prefix: String::new(),
        key: String::new(),
        record_source: None,
    })
}

// ── editable MAPPING literal ──

struct Lit<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Lit<'a> {
    fn ws(&mut self) {
        loop {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            }
            if self.b.get(self.i) == Some(&b'#') {
                while self.i < self.b.len() && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
            } else {
                return;
            }
        }
    }
    fn one_string(&mut self) -> Result<String, ()> {
        let mut raw = false;
        while matches!(self.b.get(self.i), Some(b'r' | b'R')) {
            raw = true;
            self.i += 1;
        }
        let q = *self.b.get(self.i).ok_or(())?;
        if q != b'\'' && q != b'"' {
            return Err(());
        }
        let triple = self.b[self.i..].starts_with(&[q, q, q]);
        self.i += if triple { 3 } else { 1 };
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or(())?;
            if triple {
                if self.b[self.i..].starts_with(&[q, q, q]) {
                    self.i += 3;
                    break;
                }
            } else if c == q {
                self.i += 1;
                break;
            } else if c == b'\n' {
                return Err(());
            }
            if c == b'\\' && !raw {
                let e = *self.b.get(self.i + 1).ok_or(())?;
                self.i += 2;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'\\' | b'\'' | b'"' => out.push(e),
                    b'\n' => {}
                    _ => return Err(()),
                }
            } else {
                out.push(c);
                self.i += 1;
            }
        }
        String::from_utf8(out).map_err(|_| ())
    }
    fn string(&mut self) -> Result<String, ()> {
        let mut s = self.one_string()?;
        loop {
            let save = self.i;
            self.ws();
            if matches!(self.b.get(self.i), Some(b'\'' | b'"' | b'r' | b'R')) {
                s.push_str(&self.one_string()?);
            } else {
                self.i = save;
                return Ok(s);
            }
        }
    }
    fn dict(&mut self) -> Result<Vec<(String, String)>, ()> {
        self.ws();
        if self.b.get(self.i) != Some(&b'{') {
            return Err(());
        }
        self.i += 1;
        let mut out: Vec<(String, String)> = Vec::new();
        loop {
            self.ws();
            if self.b.get(self.i) == Some(&b'}') {
                self.i += 1;
                return Ok(out);
            }
            let k = self.string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') {
                return Err(());
            }
            self.i += 1;
            self.ws();
            let v = self.string()?;
            if let Some(slot) = out.iter_mut().find(|(a, _)| *a == k) {
                slot.1 = v;
            } else {
                out.push((k, v));
            }
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {}
                _ => return Err(()),
            }
        }
    }
}

fn find_mapping(text: &str) -> Option<Result<Vec<(String, String)>, ()>> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("MAPPING") {
            let rest = &line["MAPPING".len()..];
            let after = rest.trim_start_matches([' ', '\t']);
            if after.starts_with(':') || (after.starts_with('=') && !after.starts_with("==")) {
                let abs = at + "MAPPING".len() + (rest.len() - after.len());
                let eq = if after.starts_with(':') { text[abs..].find('=').map(|e| abs + e + 1)? } else { abs + 1 };
                let mut l = Lit { b: text.as_bytes(), i: eq };
                return Some(l.dict());
            }
        }
        at += line.len();
    }
    None
}

/// setuptools' editable finder `MAPPING` (read as a literal, never executed), or the `direct_url` / `top_level.txt` fallback.
pub fn editable_mapping(site_local: &Path, d: &Dist) -> Result<Vec<(String, String)>, Err> {
    for (rel, _) in &d.rows {
        if rel.len() >= 23 && rel.starts_with("__editable___") && rel.ends_with("_finder.py") {
            let text = read_text_u(&site_local.join(rel))?;
            if let Some(r) = find_mapping(&text) {
                return r.map_err(|_| Err::Crash("ValueError: malformed node or string in MAPPING".into()));
            }
        }
    }
    let url = d.direct_url.as_ref().and_then(|x| x.get("url")).and_then(|x| x.str()).unwrap_or("");
    if let Some(srcroot) = url.strip_prefix("file://") {
        let tl = site_local.join(&d.dist_info).join("top_level.txt");
        let tops: Vec<String> = if tl.exists() {
            splitlines(&read_text_u(&tl)?).into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
        } else {
            vec![]
        };
        return Ok(tops.into_iter().map(|t| (t.clone(), format!("{srcroot}/{t}"))).collect());
    }
    Ok(vec![])
}

pub fn tag_string(tags: &[String]) -> Result<String, Err> {
    let (mut pys, mut abis, mut plats) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for t in tags {
        let parts: Vec<&str> = t.split('-').collect();
        if parts.len() != 3 {
            return Err(Err::Crash(format!("ValueError: not 3 parts in tag {t:?}")));
        }
        pys.extend(parts[0].split('.').map(String::from));
        abis.extend(parts[1].split('.').map(String::from));
        plats.extend(parts[2].split('.').map(String::from));
    }
    let j = |s: BTreeSet<String>| s.into_iter().collect::<Vec<_>>().join(".");
    Ok(format!("{}-{}-{}", j(pys), j(abis), j(plats)))
}
