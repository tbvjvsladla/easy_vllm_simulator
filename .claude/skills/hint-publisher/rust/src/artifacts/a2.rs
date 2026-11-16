//! artifacts part 2 — port of `scripts/hintlib/artifacts.py` lines 1533–2943: patch list · self-gate reconstruction
//! (`_GitPath`), old-image reconstruction v2 (static script analysis · image probe), env shape, serving runner derivation.
#![allow(non_snake_case, non_upper_case_globals, dead_code, unused_imports)]
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::pyre::{self, Re};
use crate::value::{Map, J};
use crate::{jd, jl};
use super::*;

// ── private Python-text helpers ─────────────────────────────────────────────────────────────────
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}
fn lstrip(s: &str) -> &str {
    s.trim_start_matches(py_isspace)
}
fn rstrip(s: &str) -> &str {
    s.trim_end_matches(py_isspace)
}
fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}
/// `str.splitlines(keepends)`.
fn splitlines_ke(s: &str, keepends: bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if is_line_break(c) {
            let mut end = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(_, '\n')) = it.peek() {
                    it.next();
                    end += 1;
                }
            }
            out.push(if keepends { &s[start..end] } else { &s[start..i] });
            start = end;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}
fn splitlines(s: &str) -> Vec<&str> {
    splitlines_ke(s, false)
}
/// `str.split()` (whitespace runs).
fn split_ws(s: &str) -> Vec<&str> {
    s.split(py_isspace).filter(|w| !w.is_empty()).collect()
}
/// `PurePath(s).name`.
fn pp_name(s: &str) -> String {
    s.split('/').filter(|p| !p.is_empty() && *p != ".").last().unwrap_or("").to_string()
}
/// `PurePath(s).parent` as text (`str(PurePath(...).parent)`).
fn pp_parent(s: &str) -> String {
    let abs = s.starts_with('/');
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.len() <= 1 {
        return if abs { "/".into() } else { ".".into() };
    }
    let body = parts[..parts.len() - 1].join("/");
    if abs {
        format!("/{body}")
    } else {
        body
    }
}
fn chars_len(s: &str) -> usize {
    s.chars().count()
}
/// `s[a:]` by code points.
fn from_char(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[i..],
        None => "",
    }
}
/// `str(list_of_int)`.
fn int_list_repr(v: &[i64]) -> String {
    format!("[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
}
/// `str(list_of_str)`.
fn str_list_repr(v: &[String]) -> String {
    J::from(v.to_vec()).repr()
}
fn re_cell(cell: &'static std::sync::OnceLock<Re>, pat: &str, flags: u32) -> &'static Re {
    cell.get_or_init(|| Re::new(pat, flags))
}
macro_rules! rx {
    ($name:ident, $pat:expr) => {
        rx!($name, $pat, 0);
    };
    ($name:ident, $pat:expr, $flags:expr) => {
        fn $name() -> &'static Re {
            static CELL: std::sync::OnceLock<Re> = std::sync::OnceLock::new();
            re_cell(&CELL, $pat, $flags)
        }
    };
}

// Module constants of artifacts.py lines 114–205 used by this part (a1 owns their public form; same Python text).
const KNOWN_NON_PATCH_: [&str; 2] = [".gitkeep", "PROVENANCE.json"];
rx!(rx_patch_name, r"^\d+-.*\.sh$");
rx!(rx_model_trigger, r"^\s*#\s*model-trigger\s*:\s*(.+?)\s*$", pyre::M | pyre::I);
rx!(rx_gate_default, r#"^\s*:\s*"?\$\{([A-Z][A-Z0-9_]*):=([^}]*)\}"?\s*$"#, pyre::M);
rx!(rx_gate_skip_if, r#"^\s*if\s+\[\s*"?\$\{?([A-Z][A-Z0-9_]*)\}?"?\s*!=\s*"?1"?\s*\]\s*;\s*then\s*$"#);
rx!(rx_pre_gate_0, r"^\s*set\s+-[A-Za-z]+(?:\s+[a-z]+)*\s*$");
rx!(rx_pre_gate_1, r#"^\s*[A-Za-z_][A-Za-z0-9_]*=(?:"[^"`]*"|'[^']*'|[^\s`;|&]*)\s*$"#);
rx!(rx_pre_gate_2, r#"^\s*:\s*"?\$\{[A-Za-z_][A-Za-z0-9_]*:=[^}]*\}"?\s*$"#);
rx!(rx_pre_gate_3, r"^\s*if\s+\[.*\]\s*;\s*then\s*$");
rx!(rx_pre_gate_4, r"^\s*fi\s*$");
rx!(rx_pre_gate_5, r"^\s*echo\s.*$");
rx!(rx_history_run_args, r"^RUN \|(\d+) (.*)$");
rx!(rx_configs_ref, r"/app/configs/([A-Za-z0-9_.${}:-]+\.sh)");
rx!(rx_sourced, r#"(?:^\s*|[;&|]\s*|\bthen\s+)(?:source|\.)\s+\"?/app/configs/([A-Za-z0-9_.-]+\.sh)"#, pyre::M);
rx!(rx_runner_gpu_eq, r#""\$\{GPU_COUNT\}"\s*=\s*"(\d+)""#);
rx!(rx_runner_gpu_ray, r"(\d+)\.0/(\d+)\.0 GPU");
rx!(rx_placeholder_value, r"^<[^<>\s][^<>]*>$");
rx!(rx_heredoc, r#"(?<!<)<<(-?)\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\2"#);
rx!(rx_sh_exit, r#"(?:^|[;&|{(\s'\"])exit(?:\s+([^\s;&|)}#'\"]+))?\s*(?=$|[;&|)}#'\"])"#);
rx!(rx_sh_swallow, r"\|\|\s*(?:true|:)\s*(?=$|[;&)}#])");
const SWALLOW_OK_CMDS_: [&str; 4] = ["rm", "rmdir", "unlink", "find"];
rx!(rx_sh_trap, r"(?:^|[;&|{(\s])trap\s");
rx!(rx_py_exit0, r"\b(?:sys\.exit|os\._exit|quit|exit)\(\s*(?:0\s*)?\)|\braise\s+SystemExit\s*(?:\(\s*(?:0\s*)?\))?\s*(?:$|;)");
rx!(rx_set_e, r"^\s*set\s+(?:-[A-Za-z]*e[A-Za-z]*\b|-o\s+errexit\b)");
rx!(rx_set_no_e, r"^\s*set\s+(?:\+[A-Za-z]*e[A-Za-z]*\b|\+o\s+errexit\b)");
rx!(rx_skip_word, r"(?i)\bskip");
rx!(rx_sh_assign, r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$");
rx!(rx_sh_var, r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)");
rx!(rx_grep_q, r#"\bgrep\s+((?:-[A-Za-z]+\s+)+)(?:--\s+)?(?:'([^']*)'|\"([^\"$`\\]*)\")\s+(\"[^\"]*\"|'[^']*'|[^\s;|&)]+)"#);
rx!(rx_assert_in, r#"\bassert\s+(['\"])(.+?)\1\s+in\s+[A-Za-z_]"#);
rx!(rx_module_as, r"\bimport\s+([A-Za-z_][\w.]*)\s+as\s+([A-Za-z_]\w*)");
const BRE_SPECIAL_: &str = ".[]*^$\\";
const TUNING_KEYS_: [&str; 13] = [
    "RAY_PORT", "MAX_JOBS", "PYTORCH_CUDA_ALLOC_CONF", "RAY_OBJECT_STORE_MEMORY", "VLLM_VERSION", "IMAGE_TAG",
    "TENSOR_PARALLEL_SIZE", "GPU_MEMORY_UTILIZATION", "MAX_MODEL_LEN", "MAX_NUM_SEQS", "KV_CACHE_DTYPE", "MOE_BACKEND",
    "ATTENTION_BACKEND",
];
rx!(rx_fi, r"^\s*fi\b");
rx!(rx_exit0, r"^\s*exit\s+0\b");
rx!(rx_split_ops, r"&&|\|\||;|\|");
rx!(rx_var_eq_subst, r"^[A-Za-z_][A-Za-z0-9_]*=\$\(");
rx!(rx_var_eq, r"^[A-Za-z_][A-Za-z0-9_]*=");
rx!(rx_cond_kw, r"^\s*(?:if|elif|while|until)\b");

fn grp<'t>(m: &pyre::Captures<'t>, i: usize) -> Option<&'t str> {
    pyre::group(m, i)
}

// ── 패치 목록 · 자기게이트 재구성 ──────────────────────────────────────────────────────────────
/// git 커밋 트리 안 경로의 읽기 전용 Path 흉내(F24). mtime 은 없다(`stat` = OSError).
#[derive(Clone, Debug)]
pub struct _GitPath {
    pub repo: PathBuf,
    pub commit: String,
    pub rel: String,
    ent: RefCell<Option<Option<(String, String, String)>>>,
}

impl _GitPath {
    pub fn new(repo: &Path, commit: &str, rel: &str) -> _GitPath {
        _GitPath { repo: repo.to_path_buf(), commit: commit.to_string(), rel: rel.trim_matches('/').to_string(), ent: RefCell::new(None) }
    }
    pub fn name(&self) -> String {
        pp_name(&self.rel)
    }
    /// `self / part`.
    pub fn join(&self, part: &str) -> _GitPath {
        _GitPath::new(&self.repo, &self.commit, &format!("{}/{}", self.rel, part.trim_matches('/')))
    }
    fn entry(&self) -> HR<Option<(String, String, String)>> {
        if let Some(e) = self.ent.borrow().as_ref() {
            return Ok(e.clone());
        }
        let r = core::git_nocheck(&self.repo, &["-c", "core.quotePath=false", "ls-tree", &self.commit, "--", &self.rel])?;
        let parts: Vec<&str> = if r.returncode == 0 && !strip(&r.stdout).is_empty() { split_max(&r.stdout, 3) } else { vec![] };
        let e = if parts.len() >= 3 { Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string())) } else { None };
        *self.ent.borrow_mut() = Some(e.clone());
        Ok(e)
    }
    pub fn is_dir(&self) -> HR<bool> {
        Ok(matches!(self.entry()?, Some((_, t, _)) if t == "tree"))
    }
    pub fn is_file(&self) -> HR<bool> {
        Ok(matches!(self.entry()?, Some((_, t, _)) if t == "blob"))
    }
    pub fn exists(&self) -> HR<bool> {
        Ok(self.entry()?.is_some())
    }
    pub fn iterdir(&self) -> HR<Vec<_GitPath>> {
        let spec = format!("{}:{}", self.commit, self.rel);
        let r = core::git_nocheck(&self.repo, &["-c", "core.quotePath=false", "ls-tree", &spec])?;
        let mut out = Vec::new();
        if r.returncode == 0 {
            for line in splitlines(&r.stdout) {
                if let Some((_, nm)) = line.split_once('\t') {
                    if !nm.is_empty() {
                        out.push(self.join(nm));
                    }
                }
            }
        }
        Ok(out)
    }
    pub fn read_bytes(&self) -> HR<Vec<u8>> {
        let e = self.entry()?;
        let b = match &e {
            Some((_, t, sha)) if t == "blob" => core::git_bytes(&self.repo, &["cat-file", "blob", sha], false)?,
            _ => None,
        };
        b.ok_or_else(|| HintError::py("OSError", format!("git blob 읽기 실패: {self}")))
    }
    pub fn read_text(&self) -> HR<String> {
        let b = self.read_bytes()?;
        String::from_utf8(b).map_err(|e| HintError::py("UnicodeDecodeError", e.to_string()))
    }
    pub fn mode(&self) -> HR<u32> {
        Ok(match self.entry()? {
            Some((m, _, _)) if m == "100755" => 0o755,
            _ => 0o644,
        })
    }
}

impl std::fmt::Display for _GitPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", core::head_chars(&self.commit, 12), self.rel)
    }
}

/// `str.split(None, maxsplit)`.
fn split_max(s: &str, maxsplit: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = lstrip(s);
    while !rest.is_empty() {
        if out.len() == maxsplit {
            out.push(rest);
            break;
        }
        match rest.find(py_isspace) {
            Some(i) => {
                out.push(&rest[..i]);
                rest = lstrip(&rest[i..]);
            }
            None => {
                out.push(rest);
                break;
            }
        }
    }
    out
}

/// A patch directory/file: a working-tree path or a `_GitPath` (Python duck-types the two).
#[derive(Clone, Debug)]
pub enum PatchPath {
    Fs(PathBuf),
    Git(_GitPath),
}

impl PatchPath {
    pub fn join(&self, part: &str) -> PatchPath {
        match self {
            PatchPath::Fs(p) => PatchPath::Fs(p.join(part)),
            PatchPath::Git(g) => PatchPath::Git(g.join(part)),
        }
    }
    pub fn name(&self) -> String {
        match self {
            PatchPath::Fs(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            PatchPath::Git(g) => g.name(),
        }
    }
    pub fn is_dir(&self) -> HR<bool> {
        match self {
            PatchPath::Fs(p) => Ok(p.is_dir()),
            PatchPath::Git(g) => g.is_dir(),
        }
    }
    pub fn is_file(&self) -> HR<bool> {
        match self {
            PatchPath::Fs(p) => Ok(p.is_file()),
            PatchPath::Git(g) => g.is_file(),
        }
    }
    pub fn iterdir(&self) -> HR<Vec<PatchPath>> {
        match self {
            PatchPath::Fs(p) => {
                let rd = std::fs::read_dir(p).map_err(|e| core::io_err(&e, p))?;
                Ok(rd.filter_map(|e| e.ok()).map(|e| PatchPath::Fs(e.path())).collect())
            }
            PatchPath::Git(g) => Ok(g.iterdir()?.into_iter().map(PatchPath::Git).collect()),
        }
    }
    pub fn read_bytes(&self) -> HR<Vec<u8>> {
        match self {
            PatchPath::Fs(p) => std::fs::read(p).map_err(|e| core::io_err(&e, p)),
            PatchPath::Git(g) => g.read_bytes(),
        }
    }
    /// `artifacts._read(p)` (both forms).
    pub fn read_text(&self) -> HR<String> {
        match self {
            PatchPath::Fs(p) => _read(p),
            PatchPath::Git(g) => g.read_text(),
        }
    }
    /// The working-tree path (`None` for a git tree entry — no mtime, `stat` = OSError).
    pub fn fs(&self) -> Option<&Path> {
        match self {
            PatchPath::Fs(p) => Some(p),
            PatchPath::Git(_) => None,
        }
    }
    pub fn is_git(&self) -> bool {
        matches!(self, PatchPath::Git(_))
    }
    /// `_GitPath.mode` (working-tree files: the file mode bits).
    pub fn mode(&self) -> HR<u32> {
        match self {
            PatchPath::Git(g) => g.mode(),
            PatchPath::Fs(p) => {
                use std::os::unix::fs::PermissionsExt;
                let md = std::fs::metadata(p).map_err(|e| core::io_err(&e, p))?;
                Ok(md.permissions().mode() & 0o777)
            }
        }
    }
}

impl std::fmt::Display for PatchPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchPath::Fs(p) => f.write_str(&core::posix(p)),
            PatchPath::Git(g) => write!(f, "{g}"),
        }
    }
}

/// 빌드 패치 디렉터리들의 뿌리(F24): 이미지를 지은 레시피 통로가 이 체크아웃 밖이면 그 커밋 트리(`_GitPath`) · 아니면 `output/<topo>/`.
/// SIGNATURE: returns `PatchPath` (the generated `HR<J>` stub could not carry a `_GitPath`).
pub fn _patch_base(c: &_Ctx) -> HR<PatchPath> {
    if let Some(g) = memo_get(c, "patch_base").filter(J::truthy) {
        let l = &g["__gitpath__"];
        return Ok(PatchPath::Git(_GitPath::new(&c.repo, &l.at(0).display(), &l.at(1).display())));
    }
    Ok(PatchPath::Fs(ctx_out(c)))
}

/// SIGNATURE: takes `&PatchPath` (stub took `&J`).
pub fn _sha256_of(p: &PatchPath) -> HR<String> {
    match p {
        PatchPath::Git(g) => Ok(core::sha256_bytes(&g.read_bytes()?)),
        PatchPath::Fs(f) => core::sha256_file(f),
    }
}

/// (패치 `<NN>-*.sh`, 알려진 비-패치, 규약 밖 이름). 디렉터리 부재·비어 있음은 정상 no-op.
/// SIGNATURE: takes/returns `PatchPath` (Python passes a `Path` or a `_GitPath`).
pub fn _list_patch_dir(d: &PatchPath) -> HR<(Vec<PatchPath>, Vec<String>, Vec<String>)> {
    let (mut patches, mut known, mut odd) = (Vec::new(), Vec::new(), Vec::new());
    if !d.is_dir()? {
        return Ok((patches, known, odd));
    }
    let mut items = d.iterdir()?;
    items.sort_by_key(|p| p.name());
    for p in items {
        let name = p.name();
        if KNOWN_NON_PATCH_.contains(&name.as_str()) {
            known.push(name);
        } else if p.is_file()? && rx_patch_name().is_full(&name) {
            patches.push(p);
        } else if p.is_dir()? {
            known.push(format!("{name}/"));
        } else {
            odd.push(name);
        }
    }
    Ok((patches, known, odd))
}

pub fn _patch_trigger(text: &str) -> HR<Option<String>> {
    Ok(rx_model_trigger().search(text).and_then(|m| grp(&m, 1).map(|s| strip(s).to_string())))
}

/// 스크립트의 자기게이트. None = 자기게이트 없음(ungated 또는 내용 조건부 skip — 재구성 불가).
pub fn _self_gate(text: &str) -> HR<Option<J>> {
    let mut defaults = Map::new();
    for m in rx_gate_default().finditer(text) {
        defaults.insert(grp(&m, 1).unwrap_or(""), grp(&m, 2).unwrap_or(""));
    }
    let lines = splitlines(text);
    let mut guards: Vec<String> = Vec::new();
    let mut guard_lines: BTreeSet<i64> = BTreeSet::new();
    let mut first: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let Some(m) = rx_gate_skip_if().match_at_start(line) else { continue };
        let hi = (i + 12).min(lines.len());
        for k in (i + 1)..hi {
            let body = lines[k];
            if rx_fi().is_match(body) {
                break;
            }
            if rx_exit0().is_match(body) {
                guards.push(grp(&m, 1).unwrap_or("").to_string());
                if first.is_none() {
                    first = Some(i);
                }
                let end = (k..hi).find(|&j| rx_fi().is_match(lines[j])).unwrap_or(k);
                for x in (i + 1)..(end + 2) {
                    guard_lines.insert(x as i64);
                }
                break;
            }
        }
    }
    if guards.is_empty() {
        return Ok(None);
    }
    let pre_ok = [rx_pre_gate_0(), rx_pre_gate_1(), rx_pre_gate_2(), rx_pre_gate_3(), rx_pre_gate_4(), rx_pre_gate_5()];
    for line in &lines[..first.unwrap_or(0)] {
        let st = strip(line);
        if st.is_empty() || st.starts_with('#') {
            continue;
        }
        if st.contains("$(") || st.contains('`') || !pre_ok.iter().any(|rx| rx.is_match(line)) {
            return Ok(None);
        }
    }
    let mut vars: BTreeSet<String> = defaults.keys().map(str::to_string).collect();
    vars.extend(guards.iter().cloned());
    let gset: BTreeSet<String> = guards.iter().cloned().collect();
    Ok(Some(jd! {
        "vars" => vars.into_iter().collect::<Vec<_>>(),
        "guards" => gset.into_iter().collect::<Vec<_>>(),
        "defaults" => J::Dict(defaults),
        "guard_lines" => J::List(guard_lines.into_iter().map(J::Int).collect()),
    }))
}

/// docker history 의 `RUN |N K=V …` 에서 쓰인 Dockerfile 이 선언한 ARG 만 모은다(층간 불일치 키는 제외).
pub fn _history_build_args(c: &_Ctx, dockerfile_text: &str) -> HR<(J, String)> {
    let (image, image_src) = _image_ref(c)?;
    let Some(image) = image.filter(|s| !s.is_empty()) else {
        return Ok((J::dict(), format!("unobserved({image_src})")));
    };
    let (cls, proc_) = ctx_docker(c, &["history", "--no-trunc", "--format", "{{.CreatedBy}}", &image], true)?;
    let Some(proc_) = proc_.filter(|_| cls == "ok") else {
        return Ok((J::dict(), format!("unobserved(docker history {cls} · {image_src})")));
    };
    let names: BTreeSet<String> = _dockerfile_args(c, dockerfile_text)?.into_iter().collect();
    Ok(history_args_from(&proc_.stdout, &names, &image, &image_src))
}

fn history_args_from(stdout: &str, names: &BTreeSet<String>, image: &str, image_src: &str) -> (J, String) {
    let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in splitlines(stdout) {
        let Some(m) = rx_history_run_args().match_at_start(strip(line)) else { continue };
        let n: usize = grp(&m, 1).unwrap_or("0").parse().unwrap_or(usize::MAX);
        let toks: Vec<&str> = grp(&m, 2).unwrap_or("").split(' ').take(n).collect();
        let pairs: Vec<(&str, bool, &str)> = toks
            .iter()
            .map(|t| match t.split_once('=') {
                Some((k, v)) => (k, true, v),
                None => (*t, false, ""),
            })
            .collect();
        if pairs.is_empty() || pairs.iter().any(|(k, eq, _)| !eq || !names.contains(*k)) {
            continue;
        }
        for (k, _, v) in pairs {
            seen.entry(k.to_string()).or_default().insert(v.to_string());
        }
    }
    let mut args = Map::new();
    let mut conflict = Vec::new();
    for (k, v) in &seen {
        if v.len() == 1 {
            args.insert(k.clone(), v.iter().next().unwrap().clone());
        } else {
            conflict.push(k.clone());
        }
    }
    let src = format!("docker history --no-trunc {image}(RUN |N 인자 · 쓰인 Dockerfile 의 ARG 만 · {image_src})");
    let tail = if conflict.is_empty() { String::new() } else { format!(" · 층간 불일치 제외 {}", str_list_repr(&conflict)) };
    (J::Dict(args), src + &tail)
}

// ── 옛 이미지(원장 없음) 재구성 v2 — 스크립트 정적 분석 · 이미지 탐침 ────────────────────────────
/// 셸 값 원문 → 리터럴. 명령 치환·백틱·역슬래시·미해소 변수가 있으면 None.
pub fn _sh_literal(raw: &str, known: &J) -> HR<Option<String>> {
    Ok(sh_literal(raw, known.as_dict().unwrap_or(&Map::new())))
}

fn sh_literal(raw: &str, known: &Map) -> Option<String> {
    let raw = strip(raw);
    let (body, rest): (&str, &str);
    if raw.starts_with('\'') || raw.starts_with('"') {
        let q = &raw[..1];
        let end = raw[1..].find(q)? + 1;
        body = &raw[1..end];
        rest = &raw[end + 1..];
        if q == "'" {
            return if strip(rest).is_empty() || lstrip(rest).starts_with('#') { Some(body.to_string()) } else { None };
        }
    } else {
        let parts = split_max(raw, 1);
        body = parts.first().copied().unwrap_or("");
        rest = parts.get(1).copied().unwrap_or("");
    }
    if !strip(rest).is_empty() && !lstrip(rest).starts_with('#') {
        return None;
    }
    if body.contains("$(") || body.contains('`') || body.contains('\\') {
        return None;
    }
    let mut miss = false;
    let out = rx_sh_var().sub_fn(body, |m| {
        let name = grp(m, 1).or_else(|| grp(m, 2)).unwrap_or("");
        match known.get(name) {
            Some(J::Str(v)) => v.clone(),
            _ => {
                miss = true;
                String::new()
            }
        }
    });
    if miss || out.contains('$') {
        None
    } else {
        Some(out)
    }
}

/// `str.isdigit()` (Unicode decimal digits) and its `int()` value.
fn py_digit_value(v: &str) -> Option<u128> {
    if v.is_empty() {
        return None;
    }
    let mut acc: u128 = 0;
    for ch in v.chars() {
        let d = ch.to_digit(10).or_else(|| unicode_decimal(ch))?;
        acc = acc.saturating_mul(10).saturating_add(d as u128);
    }
    Some(acc)
}
/// Decimal value of a non-ASCII Nd digit (the common Unicode digit blocks; `None` = not a digit).
fn unicode_decimal(ch: char) -> Option<u32> {
    const ZEROS: [u32; 22] = [
        0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20,
        0x1040, 0x1090, 0x17E0, 0x1810, 0xFF10, 0x1D7CE,
    ];
    let c = ch as u32;
    ZEROS.iter().find(|&&z| c >= z && c < z + 10).map(|&z| c - z)
}

/// 패치 스크립트의 정적 모양 — 판정에 쓰는 사실만 모은다(추측 ✗).
pub fn _script_shape(text: &str) -> HR<J> {
    let lines = splitlines(text);
    let mut code: Vec<(i64, &str)> = Vec::new();
    let mut doc: Vec<(i64, &str)> = Vec::new();
    let mut pending: Vec<(bool, String)> = Vec::new();
    for (idx, ln) in lines.iter().enumerate() {
        let n = idx as i64 + 1;
        if !pending.is_empty() {
            let (dash, delim) = &pending[0];
            let cmp = if *dash { strip(ln) } else { rstrip(ln) };
            if cmp == delim {
                pending.remove(0);
            } else if !strip(ln).is_empty() && !lstrip(ln).starts_with('#') {
                doc.push((n, ln));
            }
            continue;
        }
        if strip(ln).is_empty() || lstrip(ln).starts_with('#') {
            continue;
        }
        code.push((n, ln));
        for hm in rx_heredoc().finditer(ln) {
            pending.push((grp(&hm, 1) == Some("-"), grp(&hm, 3).unwrap_or("").to_string()));
        }
    }
    let mut logical: Vec<(i64, String)> = Vec::new();
    let (mut buf, mut start) = (String::new(), 0i64);
    for (n, ln) in &code {
        if buf.is_empty() {
            start = *n;
        }
        let r = rstrip(ln);
        if r.ends_with('\\') {
            buf.push_str(&r[..r.len() - 1]);
            buf.push(' ');
            continue;
        }
        logical.push((start, format!("{buf}{ln}")));
        buf.clear();
    }
    if !buf.is_empty() {
        logical.push((start, buf));
    }
    let mut known = Map::new();
    for (_, ln) in &code {
        if let Some(m) = rx_sh_assign().match_at_start(ln) {
            let name = grp(&m, 1).unwrap_or("").to_string();
            let val: J = sh_literal(grp(&m, 2).unwrap_or(""), &known).into();
            let keep = match known.get(&name) {
                None => true,
                Some(old) => *old == val,
            };
            known.insert(name, if keep { val } else { J::Null });
        }
    }
    let mut exits = Vec::new();
    for (n, ln) in &code {
        for m in rx_sh_exit().finditer(ln) {
            let v = grp(&m, 1);
            let counts = match v {
                None => true,
                Some(s) => match py_digit_value(s) {
                    None => true,
                    Some(x) => x == 0,
                },
            };
            if counts {
                exits.push(jd! {"line" => *n, "code" => v.unwrap_or("(생략)")});
            }
        }
    }
    let mut swallow: BTreeSet<i64> = BTreeSet::new();
    for (n, ln) in &logical {
        for m in rx_sh_swallow().finditer(ln) {
            let mm = m.get(0).unwrap();
            if lstrip(&ln[mm.end()..]).starts_with(')') {
                continue;
            }
            let pieces = rx_split_ops().split(&ln[..mm.start()]);
            let left = strip(pieces.last().map(String::as_str).unwrap_or("")).to_string();
            if rx_var_eq_subst().is_match(&left) {
                continue;
            }
            let words: Vec<&str> = split_ws(&left).into_iter().filter(|w| !rx_var_eq().is_match(w)).collect();
            if let Some(w0) = words.first() {
                if SWALLOW_OK_CMDS_.contains(&pp_name(w0).as_str()) {
                    continue;
                }
            }
            swallow.insert(*n);
        }
    }
    let traps: Vec<J> = logical.iter().filter(|(_, ln)| rx_sh_trap().is_search(ln)).map(|(n, _)| J::Int(*n)).collect();
    let mut markers = Vec::new();
    for (n, ln) in &logical {
        if rx_cond_kw().is_match(ln) {
            continue;
        }
        for m in rx_grep_q().finditer(ln) {
            let st = m.get(0).unwrap().start();
            if rstrip(&ln[..st]).ends_with('!') {
                continue;
            }
            let opts = grp(&m, 1).unwrap_or("").replace('-', "").replace(' ', "");
            let needle = match grp(&m, 2) {
                Some(s) => s,
                None => grp(&m, 3).unwrap_or(""),
            };
            if !opts.contains('q')
                || needle.is_empty()
                || (!opts.contains('F') && needle.chars().any(|ch| BRE_SPECIAL_.contains(ch)))
            {
                continue;
            }
            if let Some(target) = sh_literal(grp(&m, 4).unwrap_or(""), &known) {
                if target.starts_with('/') {
                    markers.push(jd! {"kind" => "grep", "needle" => needle, "path" => target, "line" => *n});
                }
            }
        }
    }
    let every: Vec<(i64, &str)> = code.iter().chain(doc.iter()).cloned().collect();
    let errexit = code.iter().any(|(_, ln)| rx_set_e().is_match(ln)) && !code.iter().any(|(_, ln)| rx_set_no_e().is_match(ln));
    let py_exit0: Vec<J> = every.iter().filter(|(_, ln)| rx_py_exit0().is_search(ln)).map(|(n, _)| J::Int(*n)).collect();
    let mut skip_lines: Vec<i64> = every.iter().filter(|(_, ln)| rx_skip_word().is_search(ln)).map(|(n, _)| *n).collect();
    skip_lines.sort();
    let status_protocol = every.iter().any(|(_, ln)| ln.contains("EASY_VLLM_PATCH_STATUS"));
    let mut needles = Vec::new();
    for (_, ln) in &every {
        for m in rx_assert_in().finditer(ln) {
            needles.push(J::from(grp(&m, 2).unwrap_or("")));
        }
    }
    let mut modules: BTreeSet<String> = BTreeSet::new();
    for (_, ln) in &every {
        for m in rx_module_as().finditer(ln) {
            if text.contains(&format!("{}.__file__", grp(&m, 2).unwrap_or(""))) {
                modules.insert(grp(&m, 1).unwrap_or("").to_string());
            }
        }
    }
    Ok(jd! {
        "errexit" => errexit,
        "exits" => J::List(exits),
        "swallow" => J::List(swallow.into_iter().map(J::Int).collect()),
        "traps" => J::List(traps),
        "py_exit0" => J::List(py_exit0),
        "skip_lines" => J::List(skip_lines.into_iter().map(J::Int).collect()),
        "status_protocol" => status_protocol,
        "markers" => J::List(markers),
        "assert_needles" => J::List(needles),
        "modules" => modules.into_iter().collect::<Vec<_>>(),
    })
}

fn ints(v: &J) -> Vec<i64> {
    v.items().iter().filter_map(J::as_i64).collect()
}

/// (ii) 규칙이 성립하지 않는 이유 목록 — 빈 목록 = "성공 종료는 곧 끝까지 실행". `exclude` = 열린 자기게이트 블록 줄
/// (int list; `J::Null` = frozenset()).
pub fn _fail_loud_problems(shape: &J, exclude: &J) -> HR<Vec<String>> {
    let ex: BTreeSet<i64> = ints(&J::List(crate::api::unwrap_seq(exclude))).into_iter().collect();
    Ok(fail_loud_problems(shape, &ex))
}

fn fail_loud_problems(shape: &J, exclude: &BTreeSet<i64>) -> Vec<String> {
    let mut probs = Vec::new();
    if !shape["errexit"].truthy() {
        probs.push("set -e 부재(또는 해제) — 중간 실패가 종료코드에 남지 않을 수 있다".to_string());
    }
    let ex: Vec<&J> = shape["exits"].items().iter().filter(|e| !e["line"].as_i64().map_or(false, |l| exclude.contains(&l))).collect();
    if !ex.is_empty() {
        let parts: Vec<String> =
            ex.iter().take(5).map(|e| format!("L{}(exit {})", e["line"].display(), e["code"].display())).collect();
        probs.push(format!("성공 종료 가능 exit {}", parts.join(", ")));
    }
    let keep = |k: &str| -> Vec<i64> { ints(&shape[k]).into_iter().filter(|n| !exclude.contains(n)).collect() };
    let head5 = |v: &[i64]| int_list_repr(&v[..v.len().min(5)]);
    let sw = keep("swallow");
    if !sw.is_empty() {
        probs.push(format!("실패 삼킴 `|| true`·`|| :`(정리 명령 밖 — set -e 가 그 실패를 못 본다) L{}", head5(&sw)));
    }
    let tr = keep("traps");
    if !tr.is_empty() {
        probs.push(format!("trap(종료 경로·종료코드를 바꿀 수 있다) L{}", head5(&tr)));
    }
    let py = keep("py_exit0");
    if !py.is_empty() {
        probs.push(format!("내장 파이썬 성공 조기 종료 L{}", head5(&py)));
    }
    let sk = keep("skip_lines");
    if !sk.is_empty() {
        probs.push(format!("skip 문구(내용 조건부 skip 경로) L{}", head5(&sk)));
    }
    if shape["status_protocol"].truthy() {
        probs.push("EASY_VLLM_PATCH_STATUS 상태 프로토콜(스스로 skipped 를 선언할 수 있다)".to_string());
    }
    probs
}

// ── 이미지 탐침 결과(바이트는 메모리에만 · 직렬화 ✗) ───────────────────────────────────────────
/// `_image_probe` 결과 `{"record", "scripts", "files", "modules", "ran"}` 의 typed 형(바이트를 J 에 싣지 않는다).
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub record: J,
    /// phase → `{name: (bytes, mode)}` | None (insertion order).
    pub scripts: Vec<(String, Option<Vec<(String, Vec<u8>, u32)>>)>,
    pub files: Vec<(String, Option<Vec<u8>>)>,
    pub modules: Vec<(String, Option<String>)>,
    pub ran: bool,
}

impl Probe {
    pub fn script_dir(&self, ph: &str) -> Option<&Vec<(String, Vec<u8>, u32)>> {
        self.scripts.iter().find(|(p, _)| p == ph).and_then(|(_, d)| d.as_ref())
    }
    pub fn file(&self, path: &str) -> Option<&Vec<u8>> {
        self.files.iter().find(|(p, _)| p == path).and_then(|(_, b)| b.as_ref())
    }
    fn set_file(&mut self, path: &str, data: Option<Vec<u8>>) {
        match self.files.iter_mut().find(|(p, _)| p == path) {
            Some(slot) => slot.1 = data,
            None => self.files.push((path.to_string(), data)),
        }
    }
    pub fn module(&self, m: &str) -> Option<&str> {
        self.modules.iter().find(|(k, _)| k == m).and_then(|(_, v)| v.as_deref())
    }
    fn set_module(&mut self, m: &str, v: Option<String>) {
        match self.modules.iter_mut().find(|(k, _)| k == m) {
            Some(slot) => slot.1 = v,
            None => self.modules.push((m.to_string(), v)),
        }
    }
    /// The bridge form (`{"ran", "files": {path: bytes|str|None}, "modules": {..}, "scripts": {..}}`) → `Probe`.
    /// `None` = Python falsy (`None`/`{}`).
    pub fn from_j(v: &J) -> Option<Probe> {
        let m = v.as_dict()?;
        if m.is_empty() {
            return None;
        }
        let bytes = |x: &J| -> Option<Vec<u8>> {
            match x {
                J::Str(s) => Some(s.as_bytes().to_vec()),
                J::Dict(d) => d.get("__bytes__").and_then(J::as_str).map(hex_decode),
                _ => None,
            }
        };
        let mut p = Probe { record: v.get("record").cloned().unwrap_or(J::Null), ran: v["ran"].truthy(), ..Probe::default() };
        if let Some(f) = v["files"].as_dict() {
            for (k, x) in f.iter() {
                p.files.push((k.to_string(), bytes(x)));
            }
        }
        if let Some(f) = v["modules"].as_dict() {
            for (k, x) in f.iter() {
                p.modules.push((k.to_string(), x.as_str().map(str::to_string)));
            }
        }
        if let Some(f) = v["scripts"].as_dict() {
            for (ph, d) in f.iter() {
                let dir = d.as_dict().map(|dm| {
                    dm.iter()
                        .map(|(n, pair)| (n.to_string(), bytes(pair.at(0)).unwrap_or_default(), pair.at(1).as_i64().unwrap_or(0) as u32))
                        .collect()
                });
                p.scripts.push((ph.to_string(), dir));
            }
        }
        Some(p)
    }
}

fn hex_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    (0..b.len() / 2).filter_map(|i| u8::from_str_radix(std::str::from_utf8(&b[2 * i..2 * i + 2]).ok()?, 16).ok()).collect()
}

fn bytes_contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || hay.windows(needle.len()).any(|w| w == needle)
}

/// 스크립트가 **스스로 선언한** 적용 마커가 이미지 안 대상 파일에 있는가. `probe` = the bridge form (see [`Probe::from_j`]).
pub fn _marker_check(shape: &J, probe: &J) -> HR<J> {
    Ok(marker_check(shape, Probe::from_j(probe).as_ref()))
}

pub fn marker_check(shape: &J, probe: Option<&Probe>) -> J {
    let Some(probe) = probe.filter(|p| p.ran) else {
        return jd! {"status" => "probe-not-run", "declared" => 0, "found" => 0};
    };
    let mut declared: Vec<(String, Vec<String>)> = shape["markers"]
        .items()
        .iter()
        .map(|m| (m["needle"].display(), vec![m["path"].display()]))
        .collect();
    let mod_paths: Vec<String> =
        shape["modules"].items().iter().filter_map(|m| probe.module(&m.display()).map(str::to_string)).filter(|p| !p.is_empty()).collect();
    for n in shape["assert_needles"].items() {
        declared.push((n.display(), mod_paths.clone()));
    }
    if declared.is_empty() {
        return jd! {"status" => "no-declared-markers", "declared" => 0, "found" => 0};
    }
    let (mut found, mut missing, mut where_) = (0i64, Vec::new(), BTreeSet::new());
    for (needle, paths) in &declared {
        let hit = paths.iter().find(|p| probe.file(p).map_or(false, |b| bytes_contains(b, needle.as_bytes())));
        match hit {
            Some(h) => {
                found += 1;
                where_.insert(h.clone());
            }
            None => missing.push(needle.clone()),
        }
    }
    let status = if missing.is_empty() { "all-found" } else { "missing" };
    let n = declared.len();
    missing.truncate(5);
    jd! {"status" => status, "declared" => n, "found" => found, "missing" => missing,
         "targets" => where_.into_iter().collect::<Vec<_>>()}
}

/// `_reconstruct` 의 env(`{"loop", "image_exists", "probe", "identity_ok"}`) typed 형.
pub struct RecEnv<'a> {
    pub loop_: Option<(Option<bool>, String)>,
    pub image_exists: bool,
    pub probe: Option<&'a Probe>,
    pub identity_ok: bool,
}

/// 원장 없는 이미지의 패치 한 개 판정(라벨된 재구성). `env` = bridge form (`loop` = `[bool|None, src]`).
pub fn _reconstruct(name: &str, text: &str, phase: &str, args: &J, env: &J) -> HR<J> {
    let probe = Probe::from_j(&env["probe"]);
    let loop_ = match env.get("loop") {
        Some(l) if l.truthy() => Some((if l.at(0).is_null() { None } else { Some(l.at(0).truthy()) }, l.at(1).display())),
        _ => None,
    };
    let re = RecEnv { loop_, image_exists: env["image_exists"].truthy(), probe: probe.as_ref(), identity_ok: env["identity_ok"].truthy() };
    reconstruct(name, text, phase, args, &re)
}

fn with_row(row: &Map, extra: Vec<(&str, J)>) -> J {
    let mut m = row.clone();
    for (k, v) in extra {
        m.insert(k, v);
    }
    J::Dict(m)
}

pub fn reconstruct(name: &str, text: &str, phase: &str, args: &J, env: &RecEnv) -> HR<J> {
    let trig = _patch_trigger(text)?;
    let mut row = Map::new();
    row.insert("phase", phase);
    row.insert("file", name);
    row.insert("declared_model_trigger", J::from(trig));
    let gate = _self_gate(text)?;
    let mut exclude: BTreeSet<i64> = BTreeSet::new();
    let mut gate_note = String::new();
    let argd = args.as_dict().cloned().unwrap_or_default();
    if let Some(gate) = &gate {
        let vars: Vec<String> = gate["vars"].items().iter().map(J::display).collect();
        let guards: Vec<String> = gate["guards"].items().iter().map(J::display).collect();
        let absent: Vec<String> = vars.iter().filter(|v| !argd.contains_key(v)).cloned().collect();
        if !absent.is_empty() {
            return Ok(with_row(&row, vec![
                ("result", "unobserved".into()),
                ("result_source", "none".into()),
                ("reason", format!("원장 부재 · 게이트 변수 {} 의 빌드 인자 미관측(docker history) — 적용 미관측", str_list_repr(&absent)).into()),
            ]));
        }
        let defaults = gate["defaults"].as_dict().cloned().unwrap_or_default();
        let argv = |v: &str| argd.get(v).map(J::display).unwrap_or_default();
        let eff: BTreeMap<String, String> = vars
            .iter()
            .map(|v| {
                let a = argv(v);
                let e = if a != "" || !defaults.contains_key(v) { a } else { defaults.get(v).map(J::display).unwrap_or_default() };
                (v.clone(), e)
            })
            .collect();
        let on: Vec<String> = vars.iter().filter(|v| eff[*v] == "1").cloned().collect();
        if on.is_empty() {
            let vals: Vec<String> = vars
                .iter()
                .map(|v| {
                    let a = argv(v);
                    let shown = if a.is_empty() { "<empty>".to_string() } else { a.clone() };
                    let tail = if eff[v] != a { format!("(:={})", eff[v]) } else { String::new() };
                    format!("{v}={shown}{tail}")
                })
                .collect();
            return Ok(with_row(&row, vec![
                ("result", "skipped".into()),
                ("result_source", "reconstructed(docker-history+gate)".into()),
                ("reason", format!("{}(docker history) — 자기게이트 `!= 1 → exit 0`", vals.join(" · ")).into()),
            ]));
        }
        if !guards.iter().all(|g| eff.get(g).map_or(false, |x| x == "1")) {
            return Ok(with_row(&row, vec![
                ("result", "unobserved".into()),
                ("result_source", "none".into()),
                ("reason", format!(
                    "원장 부재 · 게이트 변수 {}=1 이지만 가드 {} 가 전부 열리지는 않았다 — 실행 결과 미관측",
                    str_list_repr(&on), str_list_repr(&guards)).into()),
            ]));
        }
        exclude = ints(&gate["guard_lines"]).into_iter().collect();
        gate_note = format!(
            " · 자기게이트 열림({} — 게이트 블록의 exit 0 은 닿지 않는다)",
            guards.iter().map(|g| format!("{g}=1")).collect::<Vec<_>>().join(", ")
        );
    }
    let shape = _script_shape(text)?;
    let probs = fail_loud_problems(&shape, &exclude);
    let (loop_ok, loop_src) = env.loop_.clone().unwrap_or((None, "루프 미관측".to_string()));
    let loop_ok = loop_ok == Some(true);
    let mk = if env.identity_ok {
        marker_check(&shape, env.probe)
    } else {
        jd! {"status" => "script-identity-unproven", "declared" => 0, "found" => 0}
    };
    let status = mk.get_str("status").unwrap_or("").to_string();
    let static_: Vec<String> = if probs.is_empty() { vec!["skip 경로 없음".to_string()] } else { probs.clone() };
    let evidence = jd! {"static" => static_, "loop" => loop_src.clone(), "markers" => mk.clone()};
    let base = probs.is_empty() && loop_ok && env.image_exists;
    let unobs = |reason: String| {
        with_row(&row, vec![
            ("result", "unobserved".into()),
            ("result_source", "none".into()),
            ("evidence", evidence.clone()),
            ("reason", reason.into()),
        ])
    };
    if base && env.identity_ok && status != "missing" {
        let tail = if mk["declared"].truthy() {
            format!(" · 이미지 마커 {}/{} 확인", mk["found"].display(), mk["declared"].display())
        } else {
            String::new()
        };
        return Ok(with_row(&row, vec![
            ("result", "applied".into()),
            ("result_source", "reconstructed(fail-loud+built)".into()),
            ("evidence", evidence.clone()),
            ("reason", format!(
                "skip 경로 없음(set -e · 성공 종료 exit·skip 문구·상태 프로토콜 없음) × {loop_src} × 측정 이미지 실재 × \
                 판정한 바이트 = 이미지에 구워진 바이트 — 빌드가 끝났으므로 끝까지 돌았다{gate_note}{tail}").into()),
        ]));
    }
    if base && !env.identity_ok {
        return Ok(unobs(format!(
            "원장 부재 · 정적 규칙(ii)(skip 경로 없음 × fail-loud 루프 × 이미지 실재)은 성립하나 판정한 스크립트 바이트가 \
             이 빌드가 실행한 바이트인지 미확인(이미지 탐침 결과 없음 — 탐침은 publish 기본이다: `--docker-read-only` \
             없이 측정 이미지가 있는 호스트에서 다시 publish 하면 가른다 · 탐침 실패면 탐침 기록을 본다 · 작업트리 \
             스크립트는 빌드 뒤 추가·수정됐을 수 있다) — 적용 미관측{gate_note}")));
    }
    let missing_repr = mk.get("missing").map(J::repr).unwrap_or_else(|| "None".to_string());
    if base && status == "missing" {
        return Ok(unobs(format!(
            "원장 부재 · 정적 규칙(ii)은 성립하나 이미지 탐침이 선언 마커 부재 {missing_repr} 를 봤다 — fail-loud \
             스크립트가 끝까지 돌았다는 추론과 모순(뒤 층이 대상 파일을 덮었거나 대상 경로 해소가 틀렸다) · 적용으로 적지 \
             않는다 — 적용 미관측{gate_note}")));
    }
    let probs_or_loop = if probs.is_empty() { loop_src.clone() } else { probs.join("; ") };
    if status == "all-found" {
        return Ok(with_row(&row, vec![
            ("result", "applied".into()),
            ("result_source", "image-probe(script-sha+marker)".into()),
            ("evidence", evidence.clone()),
            ("reason", format!(
                "이미지 탐침: 판정한 스크립트 = 이미지에 구워진 바이트 · 선언 마커 {}/{} 가 대상 \
                 파일에 있다(패치 **후** 상태 관측 — 이 빌드가 썼는지 베이스가 이미 가졌는지는 가르지 않는다) · \
                 정적 판정 불가 사유: {probs_or_loop}{gate_note}", mk["found"].display(), mk["declared"].display()).into()),
        ]));
    }
    let why_probe = match status.as_str() {
        "probe-not-run" => "이미지 탐침 안 함".to_string(),
        "script-identity-unproven" => "스크립트 바이트가 이미지 바이트인지 미확인".to_string(),
        "no-declared-markers" => "스크립트가 선언한 적용 마커 없음 — 이미지 탐침으로도 가르지 못한다".to_string(),
        "missing" => format!("선언 마커 부재 {missing_repr}(빌드는 끝났다 — 모순 · 대상 경로 해소를 확인한다)"),
        other => other.to_string(),
    };
    Ok(unobs(format!("원장 부재 · 정적 판정 불가({probs_or_loop}) · {why_probe} — 적용 미관측{gate_note}")))
}

/// 원장 행 → applied_set 행(SPEC §2.1 모양 · 어휘 밖 result = unobserved).
pub fn _ledger_rows(ledger: &J) -> HR<Vec<J>> {
    let mut rows: Vec<J> = Vec::new();
    for (src_key, phase_default) in [("patches", None), ("inline_patches", Some("inline"))] {
        let coll = ledger.get(src_key).cloned().unwrap_or(J::Null);
        let items: Vec<J> = match &coll {
            J::List(l) => l.clone(),
            J::Int(_) | J::Float(_) | J::Bool(true) => {
                return Err(HintError::py("TypeError", format!("'{}' object is not iterable", coll.type_name())))
            }
            _ => Vec::new(), // falsy, or a str/dict whose items are never dicts
        };
        for r in &items {
            if !r.is_dict() {
                continue;
            }
            let phase: J = match phase_default {
                Some(p) => J::from(p),
                None => r.get("phase").cloned().unwrap_or(J::Null),
            };
            let name: J = match phase_default {
                None => r.get("file").cloned().unwrap_or(J::Null),
                Some(_) => {
                    let n = r.get("name").cloned().unwrap_or(J::Null);
                    if n.truthy() { n } else { r.get("file").cloned().unwrap_or(J::Null) }
                }
            };
            let ph = phase.as_str().unwrap_or("");
            if !(phase.is_str() && ["pre", "post", "inline"].contains(&ph)) || !name.truthy() {
                continue;
            }
            let res = r.get("result").cloned().unwrap_or(J::Null);
            let lrs = r.get("result_source").cloned().filter(J::truthy).unwrap_or_else(|| J::from("exit-code"));
            let mut row = Map::new();
            row.insert("phase", phase.clone());
            row.insert("file", name.display());
            row.insert("reason", r.get("reason").cloned().unwrap_or(J::Null));
            row.insert("result_source", "build-ledger");
            row.insert("ledger_result_source", lrs);
            row.insert("declared_model_trigger", r.get("declared_model_trigger").cloned().unwrap_or(J::Null));
            row.insert("script_sha256", r.get("script_sha256").cloned().unwrap_or(J::Null));
            if res.as_str().map_or(false, |s| s == "applied" || s == "skipped") {
                row.insert("result", res.clone());
            } else {
                row.insert("result", "unobserved");
                row.insert("reason", format!("원장 result 어휘 밖: {}", res.repr()));
            }
            row.0.retain(|(_, v)| !v.is_null());
            rows.push(J::Dict(row));
        }
    }
    let order = |p: &str| match p {
        "pre" => 0,
        "post" => 1,
        _ => 2,
    };
    rows.sort_by(|a, b| {
        (order(a.get_str("phase").unwrap_or("")), a.get_str("file").unwrap_or(""))
            .cmp(&(order(b.get_str("phase").unwrap_or("")), b.get_str("file").unwrap_or("")))
    });
    Ok(rows)
}

/// X10: post 패치 = 공유 이미지 입력.
pub fn _post_label(row: &J) -> HR<J> {
    if row.get_str("phase") != Some("post") {
        return Ok(row.clone());
    }
    let trig = row.get("declared_model_trigger").cloned().unwrap_or(J::Null);
    let label = format!(
        "공유 이미지 입력 — 이 모델의 요구가 아니라 공유 이미지의 일부{}",
        if trig.truthy() { format!("(model-trigger: {})", trig.display()) } else { "(model-trigger 헤더 없음)".to_string() }
    );
    let mut m = row.as_dict().cloned().unwrap_or_default();
    m.insert("shared_image_input", true);
    m.insert("label", label);
    Ok(J::Dict(m))
}

/// 노드 비대칭의 단계별 한 줄(슬롯 사유 · 00 §0.4 공용).
pub fn node_asymmetry_phase_text(asym: &J, ph: &str) -> HR<Option<String>> {
    let Some(nodes) = asym.get("nodes").and_then(J::as_dict) else { return Ok(None) };
    let mut items: Vec<(&str, &J)> = nodes.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    let rep = asym.get("representative").cloned().unwrap_or(J::Null);
    let mut parts = Vec::new();
    for (n, nd) in items {
        let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in nd["patches"].items() {
            if r.get_str("phase") == Some(ph) {
                by.entry(r.get("result").cloned().unwrap_or(J::Null).display()).or_default().push(r.get("file").cloned().unwrap_or(J::Null).display());
            }
        }
        let body: Vec<String> = by.iter().map(|(k, v)| format!("{k} {}", v.join(", "))).collect();
        let body = if body.is_empty() { "이 단계 행 없음".to_string() } else { body.join(" · ") };
        let tag = if rep.as_str() == Some(n) { "(대표)" } else { "" };
        parts.push(format!("{n}{tag} 원장: {body}"));
    }
    Ok(Some(format!("노드 비대칭(cluster) — {}", parts.join(" / "))))
}

// ── env 형상 ────────────────────────────────────────────────────────────────────────────────
/// 옛 `hint_collect.shape_env_value` 그대로(규칙 순서가 의미다).
pub fn _shape_key_axis(key: &str, value: &str) -> HR<String> {
    let upper = key.to_uppercase();
    if TUNING_KEYS_.contains(&upper.as_str()) {
        return Ok(value.to_string());
    }
    if value.starts_with('/') {
        return _manifest_ph(&key.to_lowercase());
    }
    let wrapped = format!("_{upper}_");
    if IDENTITY_KEY_AXIS.iter().any(|s| upper.ends_with(s) || wrapped.contains(&format!("{s}_"))) {
        return _manifest_ph(&format!("nodes[].{}", key.to_lowercase()));
    }
    Ok(value.to_string())
}

pub fn _manifest_ph(field_name: &str) -> HR<String> {
    crate::pii::manifest_placeholder(field_name)
}

/// QUANT_MODEL_PATH(호스트 바인드 원천) 값만 수신자 좌표로 바꾼다 — 원본 파일은 고치지 않는다.
pub fn _docker_triplet_env_shape(text: &str) -> HR<(String, bool)> {
    let mut out = String::new();
    let mut substituted = false;
    for line in splitlines_ke(text, true) {
        let body = line.trim_end_matches(|c| c == '\r' || c == '\n');
        let ending = &line[body.len()..];
        let (left, sep, value) = match body.split_once('=') {
            Some((l, v)) => (l, true, v),
            None => (body, false, ""),
        };
        let key = strip(&re_export().sub_literal(strip(left), "")).to_string();
        let vtrim = strip(strip(value).trim_matches(|c| c == '"' || c == '\''));
        if sep && key == "QUANT_MODEL_PATH" && !vtrim.is_empty() {
            out.push_str(left);
            out.push('=');
            out.push_str(&_manifest_ph("quant_model_path")?);
            out.push_str(ending);
            substituted = true;
        } else {
            out.push_str(line);
        }
    }
    Ok((out, substituted))
}
rx!(re_export, r"^export\s+");

/// env_tier 세부 → manifest 필드 이름(`manifest.` 접두 제거).
pub fn _field_of(detail: &J) -> HR<String> {
    let d = detail.display();
    Ok(d.strip_prefix("manifest.").map(str::to_string).unwrap_or(d))
}

// ── 서빙 러너 파생 (compose 가 실제로 부르는 것만) ──────────────────────────────────────────────
rx!(rx_services_head, r"^services:\s*$", pyre::M);
rx!(rx_service_block, r"^  ([A-Za-z0-9_.-]+):\s*\n(.*?)(?=^  [A-Za-z0-9_.-]+:\s*\n|^\S|\Z)", pyre::M | pyre::S);
rx!(rx_profiles, r"profiles:\s*\[([^\]]*)\]");
rx!(rx_command_line, r"^(\s*)(?:command|entrypoint):\s*(.*)$");

pub fn _compose_services(text: &str) -> HR<J> {
    let Some(m) = rx_services_head().search(text) else { return Ok(J::dict()) };
    let body = &text[m.get(0).unwrap().end()..];
    let mut out = Map::new();
    for sm in rx_service_block().finditer(body) {
        out.insert(grp(&sm, 1).unwrap_or(""), grp(&sm, 2).unwrap_or(""));
    }
    Ok(J::Dict(out))
}

/// debug 전용 프로필 서비스를 뺀 서비스 블록.
pub fn _serving_blocks(text: &str) -> HR<Vec<(String, String)>> {
    let svc = _compose_services(text)?;
    let mut items: Vec<(String, String)> = svc.as_dict().unwrap().iter().map(|(k, v)| (k.to_string(), v.display())).collect();
    items.sort();
    let mut out = Vec::new();
    for (name, block) in items {
        if let Some(pm) = rx_profiles().search(&block) {
            let profiles: BTreeSet<String> =
                grp(&pm, 1).unwrap_or("").split(',').map(|p| strip(p).trim_matches(|c| c == '\'' || c == '"').to_string()).collect();
            if !profiles.is_empty() && profiles.iter().all(|p| p == "debug") {
                continue;
            }
        }
        out.push((name, block));
    }
    Ok(out)
}

/// 서비스 블록의 `command:`·`entrypoint:` 값(한 줄 흐름 목록과 블록 목록 둘 다).
pub fn _command_texts(block: &str) -> HR<Vec<String>> {
    let lines = splitlines(block);
    let mut out = Vec::new();
    for (i, ln) in lines.iter().enumerate() {
        let Some(m) = rx_command_line().match_at_start(ln) else { continue };
        let indent = chars_len(grp(&m, 1).unwrap_or(""));
        let mut parts = vec![grp(&m, 2).unwrap_or("").to_string()];
        for nxt in &lines[i + 1..] {
            if !strip(nxt).is_empty() && chars_len(nxt) - chars_len(lstrip(nxt)) <= indent {
                break;
            }
            parts.push(strip(nxt).to_string());
        }
        out.push(parts.join(" "));
    }
    Ok(out)
}

/// (stem, 어느 키에서 왔나) — bash `${A:-…}` 의미: 빈 값은 부재와 같다.
pub fn _arm_stem(chain: &J, values: &J) -> HR<(String, String)> {
    for k in chain["chain"].items() {
        let k = k.display();
        let v = values.get(&k).cloned().unwrap_or(J::Null);
        let v = if v.truthy() { strip(&v.display()).to_string() } else { String::new() };
        if !v.is_empty() {
            return Ok((v, k));
        }
    }
    Ok((chain["default"].display(), "default".to_string()))
}

/// 무장 관측 한 문장(01 파일별 검증 근거 · 슬롯 note 공용).
pub fn arm_log_text(obs: &J) -> HR<String> {
    if !obs["signature"].truthy() {
        return Ok(format!("arming 관측 불가({}) — 적용 관측 ✗", obs.get("source").cloned().unwrap_or(J::Null).display()));
    }
    let meas = obs["measured"].items();
    let fail_ = obs["failed"].items();
    if meas.is_empty() && fail_.is_empty() {
        return Ok("arming 서명 미관측(측정·실패 기동 로그에 무장 줄 없음) — 적용 관측 ✗".to_string());
    }
    let lines_of = |x: &J| x["lines"].items().iter().map(J::display).collect::<Vec<_>>().join(",");
    let mut parts = Vec::new();
    if !meas.is_empty() {
        parts.push(format!(
            "측정 기동 로그 {}",
            meas.iter().map(|x| format!("{}:{}", x["path"].display(), lines_of(x))).collect::<Vec<_>>().join(" · ")
        ));
    } else {
        parts.push("측정 기동 로그에는 무장 줄 없음".to_string());
    }
    if !fail_.is_empty() {
        parts.push(format!(
            "실패 기동 로그(측정 기동 아님) {}",
            fail_.iter().map(|x| format!("{} {}:{}", x["role"].display(), x["path"].display(), lines_of(x))).collect::<Vec<_>>().join(" · ")
        ));
    }
    let pats: BTreeSet<String> = meas.iter().chain(fail_.iter()).flat_map(|x| x["patch"].items().iter().map(J::display)).collect();
    Ok(format!(
        "arming 관측 — {} (`{} {}` = .pth 무장 · 패치 효과 자체는 별도 관측)",
        parts.join(" · "),
        obs["signature"].display(),
        pats.into_iter().collect::<Vec<_>>().join(" · ")
    ))
}

pub fn _runner_gpu_literal(text: &str) -> HR<J> {
    let mut vals: BTreeSet<String> = BTreeSet::new();
    let mut lines: BTreeSet<i64> = BTreeSet::new();
    let ls = splitlines(text);
    for (i, ln) in ls.iter().enumerate() {
        for m in rx_runner_gpu_eq().finditer(ln) {
            vals.insert(grp(&m, 1).unwrap_or("").to_string());
            lines.insert(i as i64 + 1);
        }
    }
    for (i, ln) in ls.iter().enumerate() {
        for m in rx_runner_gpu_ray().finditer(ln) {
            vals.insert(grp(&m, 2).unwrap_or("").to_string());
            lines.insert(i as i64 + 1);
        }
    }
    let vals: Vec<String> = vals.into_iter().collect();
    let value = if vals.len() == 1 { J::from(vals[0].clone()) } else if vals.is_empty() { J::Null } else { J::from(vals) };
    Ok(jd! {
        "value" => value,
        "lines" => J::List(lines.into_iter().map(J::Int).collect()),
        "note" => "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 \
                   클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합).",
    })
}

// ── `_Ctx` access (a1 owns `_Ctx` · shape = coordinator ARTIFACTS_CTX) ────────────────────────────
// This part's memo entries live in `c.memo` (J, string keys; Python tuple keys joined by "\u{1f}" with `str()` parts):
//   "patch_base"        = {"__gitpath__": [commit, rel]}                      (Python: a `_GitPath`)
//   "probe"             = `Probe::to_memo_j` (bytes as {"__bytes__": hex})     (read by a3 via `probe_memo`)
//   "applied\u{1f}True" / "applied\u{1f}False" = [applied, sel, ledger|null, per_node]   (read via `applied_memo`)
//   "ship_patch"        = {"<phase>\u{1f}<name>": {"__bytes__": hex, "mode": n}}        (read by a3 via `ship_patch`)
//   "arm_key_chain" · "arm_log_obs" = the J results.
const SEP: char = '\u{1f}';
fn memo_get(c: &_Ctx, k: &str) -> Option<J> {
    c.memo_get(&J::from(k))
}
fn memo_set(c: &_Ctx, k: &str, v: J) {
    c.memo_set(&J::from(k), v);
}
fn ctx_repo(c: &_Ctx) -> PathBuf {
    c.repo.clone()
}
fn ctx_out(c: &_Ctx) -> PathBuf {
    c.out.clone()
}
fn ctx_topo(c: &_Ctx) -> String {
    c.topo.clone()
}
fn ctx_cell(c: &_Ctx) -> String {
    c.cell.clone()
}
fn ctx_node(c: &_Ctx) -> String {
    c.node.clone()
}
fn ctx_ev(c: &_Ctx) -> &J {
    &c.ev
}
/// `c.docker(*args, cache=cache)` → (분류, 프로세스|None).
fn ctx_docker(c: &_Ctx, args: &[&str], cache: bool) -> HR<(String, Option<core::Proc>)> {
    Ok(c.docker(args, cache))
}
/// `getattr(c.runner, PROBE_ATTR, False) is True`.
fn ctx_runner_probe(c: &_Ctx) -> bool {
    c.runner_probes()
}
fn ctx_default_runner(c: &_Ctx) -> bool {
    c.default_runner.get()
}
/// `c.triplet[key]` (`yaml`/`sh`/`env`).
fn ctx_triplet(c: &_Ctx, key: &str) -> Option<PathBuf> {
    c.triplet.iter().find(|(k, _)| k == key).map(|(_, p)| p.clone())
}
fn ctx_compose_path(c: &_Ctx) -> Option<PathBuf> {
    c.compose_path.clone()
}
/// `c.sf` — the owner module is not a value in Rust (`_read_env(&J::Null, ..)` = slave_forward's reader).
fn ctx_sf<'a>(c: &'a _Ctx, attr: &str) -> HR<&'a _ForwardModule> {
    c.sf(attr)
}
/// `Path(c.render().SHARED_ASSET_DIR)` = `<repo>/.claude/skills/upstream-version-watch/assets/configs`.
fn ctx_shared_asset_dir(c: &_Ctx) -> HR<PathBuf> {
    Ok(PathBuf::from(c.render()?.SHARED_ASSET_DIR))
}
/// `c.sf.derive(env, dotenv, compose, cluster_env)` → forward rows `[(key, value)]` through the owner's native CLI
/// `slave_forward.sh json --cell-env E --project-env P --compose C --cluster-env X`. A refusal (`[slave-forward] FAIL(<code>): <msg>`)
/// = `HintError{code: "py:ForwardError", message: "<code>\u{1f}<msg>"}`.
fn ctx_sf_derive(c: &_Ctx, env: Option<&Path>, dotenv: &Path, compose: Option<&Path>, cluster: &Path) -> HR<Vec<(String, String)>> {
    let p = |x: Option<&Path>| x.map(core::posix).unwrap_or_else(|| "None".to_string());
    let args: Vec<String> = vec![
        "json".into(), "--cell-env".into(), p(env), "--project-env".into(), p(Some(dotenv)),
        "--compose".into(), p(compose), "--cluster-env".into(), p(Some(cluster)),
    ];
    let r = core::run_owner(&c.repo, core::REL_SLAVE_FORWARD, &args, None, &[], false)?;
    if r.returncode != 0 {
        let line = r.stderr.lines().find(|l| l.contains("FAIL(")).unwrap_or("").to_string();
        let (code, msg) = match (line.find("FAIL("), line.find("): ")) {
            (Some(a), Some(b)) if b > a => (line[a + 5..b].to_string(), line[b + 3..].to_string()),
            _ => ("SF_FAILED".to_string(), strip(&r.stderr).to_string()),
        };
        return Err(HintError::py("ForwardError", format!("{code}{SEP}{msg}")));
    }
    let doc = crate::value::loads(&r.stdout).map_err(|e| HintError::py("ValueError", e))?;
    Ok(doc["forwards"].items().iter().map(|f| (f["key"].display(), if f["value"].is_null() { String::new() } else { f["value"].display() })).collect())
}
// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::NEVER_FORWARD
fn ctx_sf_never_forward(_c: &_Ctx) -> Vec<String> {
    vec!["CONFIG_FILE".to_string()]
}
fn ctx_sf_compose_default(c: &_Ctx, compose: Option<&Path>, key: &str) -> HR<Option<String>> {
    c.sf("compose_default")?.compose_default(compose.unwrap_or(Path::new("None")), key)
}

fn applied_key(probe: bool) -> String {
    format!("applied{SEP}{}", if probe { "True" } else { "False" })
}
/// `c.memo.get(("applied", probe))` for the other parts.
pub fn applied_memo(c: &_Ctx, probe: bool) -> Option<(J, J, Option<J>, J)> {
    let v = c.memo_get(&jl!["applied", probe])?;
    let l = v.as_list()?;
    Some((l[0].clone(), l[1].clone(), if l[2].is_null() { None } else { Some(l[2].clone()) }, l[3].clone()))
}
/// `c.memo.get("ship_patch") or {}` for a3 `_docker_slots`: (phase, name) → (image bytes, mode).
pub fn ship_patch(c: &_Ctx) -> Vec<((String, String), (Vec<u8>, u32))> {
    let Some(J::Dict(m)) = memo_get(c, "ship_patch") else { return vec![] };
    m.iter()
        .map(|(k, v)| {
            let (ph, n) = k.split_once(SEP).unwrap_or((k, ""));
            ((ph.to_string(), n.to_string()), (hex_decode(v.get_str("__bytes__").unwrap_or("")), v["mode"].as_i64().unwrap_or(0) as u32))
        })
        .collect()
}
/// `c.memo.get("probe")` for a3.
pub fn probe_memo(c: &_Ctx) -> Option<Probe> {
    memo_get(c, "probe").and_then(|v| Probe::from_j(&v))
}
fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
impl Probe {
    /// The memo/bridge form (bytes as `{"__bytes__": hex}`; script entries `[bytes, mode]`).
    pub fn to_memo_j(&self) -> J {
        let mut scripts = Map::new();
        for (ph, d) in &self.scripts {
            let v = match d {
                None => J::Null,
                Some(items) => {
                    let mut m = Map::new();
                    for (n, b, mode) in items {
                        m.insert(n.clone(), jl![jd! {"__bytes__" => hex_encode(b)}, *mode as i64]);
                    }
                    J::Dict(m)
                }
            };
            scripts.insert(ph.clone(), v);
        }
        let mut files = Map::new();
        for (p, b) in &self.files {
            files.insert(p.clone(), b.as_ref().map(|b| jd! {"__bytes__" => hex_encode(b)}).unwrap_or(J::Null));
        }
        let mut modules = Map::new();
        for (m, p) in &self.modules {
            modules.insert(m.clone(), J::from(p.clone()));
        }
        jd! {"record" => self.record.clone(), "scripts" => J::Dict(scripts), "files" => J::Dict(files),
             "modules" => J::Dict(modules), "ran" => self.ran}
    }
}

fn patch_dir(ph: &str) -> String {
    PATCH_DIRS().get_str(ph).unwrap_or("").to_string()
}
fn patch_dirs() -> Vec<(String, String)> {
    PATCH_DIRS().as_dict().unwrap().iter().map(|(k, v)| (k.to_string(), v.display())).collect()
}
fn read_lossy(p: &PatchPath) -> HR<String> {
    match p {
        PatchPath::Fs(f) => _read(f),
        PatchPath::Git(g) => match g.read_bytes() {
            Ok(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
            Err(e) => core::fail("HINT_ARTIFACT_READ_FAILED", format!("아티팩트 읽기 실패({}): {}", g.name(), e.message), None),
        },
    }
}
rx!(rx_cid, r"[0-9a-f]{12,64}");
rx!(rx_loop_bash, r#"bash\s+"\$p"\s*\|\|\s*exit\s+[1-9]"#);
rx!(rx_loop_rc, r#"\[\s*"\$rc"\s*=\s*0\s*\]\s*\|\|\s*\{[^}]*exit\s+[1-9]"#);
rx!(rx_py_root, r"^(/[^\s]*?/(?:dist|site)-packages)(?:/|$)");

/// 측정 이미지 history 의 패치 루프가 비-0 에서 빌드를 멈추는가.
pub fn _patch_loop_failloud(c: &_Ctx, df_text: &str, phase: &str) -> HR<(Option<bool>, String)> {
    let dests = _copy_dests(df_text)?;
    let d = patch_dir(phase);
    let Some(dst) = dests.get(&d) else { return Ok((None, format!("{d}/ COPY 없음"))) };
    let cdir = _dest_path(&d, &dst.display(), true)?;
    let (image, image_src) = _image_ref(c)?;
    let Some(image) = image.filter(|s| !s.is_empty()) else { return Ok((None, format!("unobserved({image_src})"))) };
    let (cls, proc_) = ctx_docker(c, &["history", "--no-trunc", "--format", "{{.CreatedBy}}", &image], true)?;
    let Some(proc_) = proc_.filter(|_| cls == "ok") else { return Ok((None, format!("unobserved(docker history {cls})"))) };
    Ok(loop_failloud_from(&proc_.stdout, &cdir))
}

fn loop_failloud_from(stdout: &str, cdir: &str) -> (Option<bool>, String) {
    let glob = format!("{cdir}/*.sh");
    for ln in splitlines(stdout) {
        if !ln.starts_with("RUN ") || !ln.contains(&glob) {
            continue;
        }
        if rx_loop_bash().is_search(ln) || rx_loop_rc().is_search(ln) {
            return (Some(true), format!("docker history 루프 `{cdir}/*.sh … || exit 1`(비-0 = 빌드 중단)"));
        }
        return (Some(false), format!("docker history 의 `{cdir}/*.sh` 루프가 비-0 에서 멈추는 꼴이 아니다"));
    }
    (None, format!("docker history 에 `{cdir}/*.sh` 루프 층이 없다"))
}

/// 탐침 여부 = 실행기의 **선언**(기본 실행기 = 선언함 · 주입 실행기는 `image_probe = True` 일 때만).
pub fn _probe_allowed(c: &_Ctx) -> HR<(bool, String)> {
    if ctx_runner_probe(c) {
        let why = if ctx_default_runner(c) { "artifacts 기본 실행기(탐침 전용 — create/cp/rm · run/start ✗)" } else { "주입 실행기(image_probe 선언)" };
        return Ok((true, why.to_string()));
    }
    Ok((false, "skipped(주입 실행기가 image_probe 를 선언하지 않았다 — 읽기 전용(--docker-read-only · 대역)으로 본다 · \
                create/cp/rm 을 부르지 않는다)".to_string()))
}

/// 이미지 ENV 값이 가리키는 `…/dist-packages`·`…/site-packages` 루트(관측 순서 · 중복 제거).
pub fn _python_roots(c: &_Ctx, image: &str) -> HR<Vec<String>> {
    let (cls, proc_) = ctx_docker(c, &["image", "inspect", "--format", "{{json .Config.Env}}", image], true)?;
    let Some(proc_) = proc_.filter(|_| cls == "ok") else { return Ok(vec![]) };
    let Ok(env) = crate::value::loads(&proc_.stdout) else { return Ok(vec![]) };
    let mut roots: Vec<String> = Vec::new();
    for kv in env.items() {
        let s = kv.display();
        let val = s.split_once('=').map(|x| x.1).unwrap_or("");
        for part in val.split(':') {
            if let Some(m) = rx_py_root().match_at_start(part) {
                let r = grp(&m, 1).unwrap_or("").to_string();
                if !roots.contains(&r) {
                    roots.push(r);
                }
            }
        }
    }
    Ok(roots)
}

pub fn _docker_cp(c: &_Ctx, cid: &str, src: &str, dest: &Path) -> HR<String> {
    let spec = format!("{cid}:{src}");
    let d = core::posix(dest);
    let (cls, proc_) = ctx_docker(c, &["cp", "-L", &spec, &d], false)?;
    if cls == "ok" {
        return Ok("ok".to_string());
    }
    let err = proc_.map(|p| p.stderr).unwrap_or_default();
    Ok(if err.contains("Could not find") || err.contains("No such") { "absent".to_string() } else { cls })
}

/// `docker create`(start ✗) 의 컨테이너 id(마지막 출력 줄).
fn created_cid(proc_: &Option<core::Proc>) -> String {
    let out = proc_.as_ref().map(|p| p.stdout.clone()).unwrap_or_default();
    splitlines(strip(&out)).last().map(|s| strip(s).to_string()).unwrap_or_default()
}

/// 측정 이미지 안 파일 하나를 **시작하지 않는** 컨테이너에서 꺼낸다(create → cp → finally rm -v).
/// Bytes = `{"__bytes__": hex}` (`_bytes_j` · decode with `_j_bytes`).
pub fn _cp_from_image(c: &_Ctx, image: &str, path: &str) -> HR<(Option<J>, String)> {
    let (cls, proc_) = ctx_docker(c, &["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", image], false)?;
    let cid = created_cid(&proc_);
    if cls != "ok" || !rx_cid().is_full(&cid) {
        return Ok((None, format!("failed(docker create {cls})")));
    }
    let tmp = core::TempDir::new("hint-probe-");
    let f = tmp.path().join("f");
    let body = (|| -> HR<(Option<J>, String)> {
        let got = _docker_cp(c, &cid, path, &f)?;
        let data = if got == "ok" && f.is_file() { Some(_bytes_j(&std::fs::read(&f).map_err(|e| core::io_err(&e, &f))?)) } else { None };
        Ok((data, got))
    })();
    let (rcls, _) = ctx_docker(c, &["rm", "-v", &cid], false)?;
    drop(tmp);
    let (data, mut got) = body?;
    if rcls != "ok" {
        let c12 = core::head_chars(&cid, 12);
        got = format!("{got} · rm-failed({c12} — `docker rm -v {c12}` 로 지운다)");
    }
    Ok((data, got))
}

/// Canonical local identity: own node or multi cluster owned by its unique main.
pub fn _local_image_owner(repo: &Path, ev: &J) -> HR<bool> {
    let (self_node, _) = crate::evidence::_self_node(repo)?;
    let target = _get(ev, "node", &J::Null)?;
    let decl = _get(ev, "declaration", &J::Null)?;
    let decl = if decl.truthy() { decl } else { J::dict() };
    let nodes = if decl.is_dict() { decl.get("nodes").cloned() } else { None };
    let main_ids: Vec<J> = match &nodes {
        Some(J::List(l)) => l
            .iter()
            .filter(|n| n.is_dict() && n.get_str("role") == Some("main") && n.get("node_id").map_or(false, J::truthy))
            .map(|n| n["node_id"].clone())
            .collect(),
        _ => vec![],
    };
    let manifest = _get(ev, "manifest", &J::Null)?;
    let topo = _get(ev, "topology", &J::Null)?;
    let mtopo = manifest.get("topology").cloned().unwrap_or(J::Null);
    let mrole = manifest.get("self_role").cloned().unwrap_or(J::Null);
    if target.as_str() == Some("sub")
        && (topo.as_str() != Some("single") || !manifest.is_dict() || mtopo.as_str() != Some("single") || mrole.as_str() != Some("sub"))
    {
        return Ok(false);
    }
    let self_j = J::from(self_node.clone());
    let local_cluster = target.as_str() == Some("cluster")
        && topo.as_str() == Some("multi")
        && manifest.is_dict()
        && mtopo.as_str() == Some("multi")
        && mrole.as_str() == Some("main")
        && main_ids.len() == 1
        && self_j == main_ids[0];
    let self_truthy = self_node.as_deref().map_or(false, |s| !s.is_empty());
    let bad = if target.as_str() == Some("cluster") { !local_cluster } else { self_j != target };
    Ok(self_truthy && !bad)
}

/// 측정 이미지의 격리된 Git 객체에서만 정확한 소스 태그를 바인딩한다(create/cp · start ✗). `ev["build_identity"]` 를 고친다.
/// SIGNATURE: `ev: &mut J` (the original mutates the evidence's `build_identity` dict in place).
pub fn bind_image_git_release(repo: &Path, ev: &mut J, runner: &J /* callable */) -> HR<()> {
    let r = _runner_from_j(runner)?;
    bind_image_git_release_with(repo, ev, r)
}

/// `bind_image_git_release` with a typed (test-injected) docker runner (`None` = the probe-only default runner).
pub fn bind_image_git_release_with(repo: &Path, ev: &mut J, runner: Option<std::rc::Rc<dyn DockerRunner>>) -> HR<()> {
    let b0 = _get(ev, "build_identity", &J::Null)?;
    let b = if b0.truthy() { b0 } else { J::dict() };
    let ref_ = if b["vllm_ref"].truthy() { b["vllm_ref"].display().to_lowercase() } else { String::new() };
    let digest = if b["image_digest"].truthy() { b["image_digest"].display() } else { String::new() };
    let sha = if b["vllm_sha"].truthy() { b["vllm_sha"].display().to_lowercase() } else { String::new() };
    rx!(rx_sha40, r"[0-9a-f]{40}");
    rx!(rx_digest, r"sha256:[0-9a-f]{64}");
    let repo_kind = crate::naming::vllm_repo_kind(&b.get("vllm_repo").cloned().unwrap_or(J::Null))?;
    if !rx_sha40().is_full(&ref_) || sha != ref_ || !rx_digest().is_full(&digest) || repo_kind.as_deref() != Some("upstream") {
        return Ok(());
    }
    if !_local_image_owner(repo, ev)? {
        return Ok(());
    }
    let c = _ctx_with(repo, ev, runner, None, None)?;
    if !_probe_allowed(&c)?.0 {
        return Ok(());
    }
    let (cls, proc_) = ctx_docker(&c, &["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", &digest], false)?;
    let cid = {
        let out = proc_.as_ref().map(|p| p.stdout.clone()).unwrap_or_default();
        if strip(&out).is_empty() { String::new() } else { splitlines(strip(&out)).last().copied().unwrap_or("").to_string() }
    };
    if cls != "ok" || !rx_cid().is_full(&cid) {
        return Ok(());
    }
    let found = probe_source_tag(&c, &cid, &ref_);
    let (rcls, rm) = ctx_docker(&c, &["rm", "-v", &cid], false)?;
    let tag = found?;
    if rcls != "ok" {
        // the original binds first and pops on cleanup failure — the evidence ends unbound either way
        let detail = rm.map(|p| if !p.stderr.is_empty() { p.stderr } else { p.stdout }).unwrap_or_default();
        return core::fail(
            "HINT_IMAGE_PROBE_CLEANUP_FAILED",
            format!("source Git probe container {cid} cleanup failed: {rcls}: {}", core::head_chars(&detail, 500)),
            None,
        );
    }
    if let Some(tag) = tag {
        if let Some(bi) = ev.get_mut("build_identity").filter(|x| x.truthy()) {
            bi.set("image_git_exact_tag", tag.clone());
            let src = format!("measured image {digest}: /workspace/vllm-src/.git HEAD and exact tag peeled to {ref_} (create/cp, no start)");
            let m = bi.as_dict_mut().unwrap();
            let s = m.setdefault("source", J::dict());
            if s.is_dict() {
                s.set("image_git_exact_tag", src);
            } else {
                return Err(HintError::py("TypeError", format!("'{}' object does not support item assignment", s.type_name())));
            }
        }
    }
    Ok(())
}

/// The isolated Git check of `bind_image_git_release` (the `try` body): `Some(tag)` = bind.
fn probe_source_tag(c: &_Ctx, cid: &str, ref_: &str) -> HR<Option<String>> {
    let tmp = core::TempDir::new("hint-source-git-");
    let root = tmp.path().to_path_buf();
    let copied = root.join("copied");
    let spec = format!("{cid}:/workspace/vllm-src/.git");
    let (st, _) = ctx_docker(c, &["cp", &spec, &core::posix(&copied)], false)?;
    let is_link = std::fs::symlink_metadata(&copied).map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if st != "ok" || is_link || !copied.is_dir() {
        return Ok(None);
    }
    if !tree_is_plain(&copied) {
        return Ok(None);
    }
    if ["alternates", "http-alternates"].iter().any(|n| copied.join("objects/info").join(n).exists()) {
        return Ok(None);
    }
    let safe = root.join("bare");
    std::fs::create_dir(&safe).map_err(|e| core::io_err(&e, &safe))?;
    for name in ["objects", "refs"] {
        let s = copied.join(name);
        if s.is_dir() {
            copy_tree(&s, &safe.join(name))?;
        }
    }
    for name in ["HEAD", "packed-refs", "shallow"] {
        let s = copied.join(name);
        if s.is_file() {
            std::fs::copy(&s, safe.join(name)).map_err(|e| core::io_err(&e, &s))?;
        }
    }
    std::fs::write(safe.join("config"), "[core]\n repositoryformatversion = 0\n bare = true\n").map_err(|e| core::io_err(&e, &safe))?;
    let head = sterile_git(&safe, &["rev-parse", "--verify", "HEAD^{commit}"]);
    let tags = sterile_git(&safe, &["tag", "--list", "--points-at", ref_]);
    let (Some(head), Some(tags)) = (head, tags) else { return Ok(None) };
    if head.0 != 0 || strip(&head.1).to_lowercase() != ref_ || tags.0 != 0 {
        return Ok(None);
    }
    let mut releases = Vec::new();
    for t in splitlines(&tags.1) {
        if crate::naming::release_of(&J::from(t))?.is_some() {
            releases.push(t.to_string());
        }
    }
    if releases.len() != 1 {
        return Ok(None);
    }
    let tag = releases.remove(0);
    let peeled = sterile_git(&safe, &["rev-parse", "--verify", &format!("refs/tags/{tag}^{{commit}}")]);
    match peeled {
        Some((0, out)) if strip(&out).to_lowercase() == ref_ => Ok(Some(tag)),
        _ => Ok(None),
    }
}

/// Every entry (recursive, links not followed) is a regular file or a directory.
fn tree_is_plain(d: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(d) else { return true };
    for e in rd.flatten() {
        let Ok(md) = std::fs::symlink_metadata(e.path()) else { return false };
        let ft = md.file_type();
        if ft.is_dir() {
            if !tree_is_plain(&e.path()) {
                return false;
            }
        } else if !ft.is_file() {
            return false;
        }
    }
    true
}

fn copy_tree(src: &Path, dst: &Path) -> HR<()> {
    std::fs::create_dir_all(dst).map_err(|e| core::io_err(&e, dst))?;
    for e in std::fs::read_dir(src).map_err(|e| core::io_err(&e, src))?.flatten() {
        let p = e.path();
        let t = dst.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &t)?;
        } else {
            std::fs::copy(&p, &t).map_err(|er| core::io_err(&er, &p))?;
        }
    }
    Ok(())
}

/// `git -c protocol.allow=never --git-dir <safe> …` with the original's scrubbed environment (timeout 20 s · start failure = None).
fn sterile_git(safe: &Path, args: &[&str]) -> Option<(i32, String)> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("git");
    cmd.arg("-c").arg("protocol.allow=never").arg("--git-dir").arg(safe).args(args);
    cmd.env_clear();
    for (k, v) in [
        ("PATH", "/bin:/usr/bin"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_NO_REPLACE_OBJECTS", "1"),
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_ALLOW_PROTOCOL", ""),
        ("LC_ALL", "C"),
    ] {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > std::time::Duration::from_secs(20) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(_) => return None,
        }
    }
    let o = child.wait_with_output().ok()?;
    Some((o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()))
}

/// 측정 이미지 안의 패치 스크립트 · 컨텍스트 COPY 대상 파일 · 스크립트가 선언한 마커의 대상 파일을 꺼낸다
/// (`docker create`(start ✗) → `docker cp` → finally `docker rm -v`). 바이트는 메모리에만 둔다.
/// SIGNATURE: returns the typed `Probe` (the stub's `J` cannot carry bytes); memoised in `A2Memo.probe`.
pub fn _image_probe(c: &_Ctx, df_text: &str) -> HR<Probe> {
    if let Some(p) = probe_memo(c) {
        return Ok(p);
    }
    let res = image_probe_run(c, df_text)?;
    memo_set(c, "probe", res.to_memo_j());
    Ok(res)
}

fn image_probe_run(c: &_Ctx, df_text: &str) -> HR<Probe> {
    let (image, image_src) = _image_ref(c)?;
    let mut rec = Map::new();
    rec.insert("tier", "image-probe");
    rec.insert("image", J::from(image.clone()));
    rec.insert("image_source", image_src.clone());
    let mut res = Probe::default();
    let finish = |res: &mut Probe, rec: Map| {
        res.record = J::Dict(rec);
    };
    let (ok, why) = _probe_allowed(c)?;
    if !ok {
        rec.insert("result", why);
        finish(&mut res, rec);
        return Ok(res);
    }
    let Some(image) = image.filter(|s| !s.is_empty()) else {
        rec.insert("result", format!("skipped({image_src})"));
        finish(&mut res, rec);
        return Ok(res);
    };
    let (cls, _) = ctx_docker(c, &["image", "inspect", "--format", "{{.Id}}", &image], true)?;
    if cls != "ok" {
        rec.insert("result", format!("skipped(측정 이미지가 로컬에 없다 · {cls} — 태그로 대체하지 않는다)"));
        finish(&mut res, rec);
        return Ok(res);
    }
    let (cls, proc_) = ctx_docker(c, &["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", &image], false)?;
    let cid = created_cid(&proc_);
    if cls != "ok" || !rx_cid().is_full(&cid) {
        rec.insert("result", format!("failed(docker create {cls})"));
        finish(&mut res, rec);
        return Ok(res);
    }
    let tmp = core::TempDir::new("hint-probe-");
    let mut copies: Vec<J> = Vec::new();
    let body = probe_copy_all(c, df_text, &cid, &image, tmp.path(), &mut res, &mut rec, &mut copies);
    let (rcls, _) = ctx_docker(c, &["rm", "-v", &cid], false)?;
    drop(tmp);
    body?;
    let removed = rcls == "ok";
    rec.insert("copies", J::List(copies));
    rec.insert("container_removed", removed);
    rec.insert(
        "result",
        if removed { "done".to_string() } else { format!("done(⚠ 컨테이너 제거 실패 — `docker rm -v {}` 로 지운다)", core::head_chars(&cid, 12)) },
    );
    finish(&mut res, rec);
    Ok(res)
}

#[allow(clippy::too_many_arguments)]
fn probe_copy_all(c: &_Ctx, df_text: &str, cid: &str, image: &str, tmp: &Path, res: &mut Probe, rec: &mut Map, copies: &mut Vec<J>) -> HR<()> {
    let dests = _copy_dests(df_text)?;
    let dmap = dests.as_dict().cloned().unwrap_or_default();
    for (ph, d) in patch_dirs() {
        let Some(dst) = dmap.get(&d) else { continue };
        let cdir = _dest_path(&d, &dst.display(), true)?;
        let base = tmp.join(format!("dir-{ph}"));
        let got = _docker_cp(c, cid, &cdir, &base)?;
        copies.push(jd! {"src" => cdir.clone(), "result" => got.clone()});
        let dir = if got == "ok" && base.is_dir() {
            let mut items: Vec<PathBuf> = std::fs::read_dir(&base).map_err(|e| core::io_err(&e, &base))?.flatten().map(|e| e.path()).collect();
            items.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            let mut out = Vec::new();
            for p in items {
                let md = std::fs::symlink_metadata(&p).map_err(|e| core::io_err(&e, &p))?;
                if !md.file_type().is_file() {
                    continue;
                }
                use std::os::unix::fs::PermissionsExt;
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                out.push((name, std::fs::read(&p).map_err(|e| core::io_err(&e, &p))?, md.permissions().mode() & 0o777));
            }
            Some(out)
        } else {
            None
        };
        match res.scripts.iter_mut().find(|(p, _)| *p == ph) {
            Some(slot) => slot.1 = dir,
            None => res.scripts.push((ph.clone(), dir)),
        }
    }
    let mut targets: Vec<String> = Vec::new();
    let out = ctx_out(c);
    let pd_values: Vec<String> = patch_dirs().into_iter().map(|x| x.1).collect();
    let mut items: Vec<(String, String)> = dmap.iter().map(|(k, v)| (k.to_string(), v.display())).collect();
    items.sort();
    for (src, dst) in items {
        if pd_values.contains(&src) || !out.join(&src).is_file() {
            continue;
        }
        targets.push(_dest_path(&src, &dst, false)?);
    }
    let mut modules: BTreeSet<String> = BTreeSet::new();
    for (ph, d) in patch_dirs() {
        let baked: Vec<(String, Vec<u8>, u32)> = res.script_dir(&ph).cloned().unwrap_or_default();
        let pb = _patch_base(c)?;
        let mut names: BTreeSet<String> = baked.iter().map(|x| x.0.clone()).collect();
        for p in _list_patch_dir(&pb.join(&d))?.0 {
            names.insert(p.name());
        }
        for name in names.into_iter().filter(|n| rx_patch_name().is_full(n)) {
            let text = match baked.iter().find(|x| x.0 == name) {
                Some(b) => String::from_utf8_lossy(&b.1).into_owned(),
                None => {
                    let f = pb.join(&d).join(&name);
                    if f.is_file()? { read_lossy(&f)? } else { String::new() }
                }
            };
            let sh = _script_shape(&text)?;
            targets.extend(sh["markers"].items().iter().map(|m| m["path"].display()));
            modules.extend(sh["modules"].items().iter().map(J::display));
        }
    }
    let roots = if modules.is_empty() { vec![] } else { _python_roots(c, image)? };
    rec.insert("python_roots", roots.clone());
    for m in &modules {
        res.set_module(m, None);
        'roots: for root in &roots {
            let mp = m.replace('.', "/");
            for cand in [format!("{root}/{mp}.py"), format!("{root}/{mp}/__init__.py")] {
                let dest = tmp.join(format!("mod-{}", copies.len()));
                let got = _docker_cp(c, cid, &cand, &dest)?;
                copies.push(jd! {"src" => cand.clone(), "result" => got.clone()});
                if got == "ok" && dest.is_file() {
                    res.set_file(&cand, Some(std::fs::read(&dest).map_err(|e| core::io_err(&e, &dest))?));
                    res.set_module(m, Some(cand.clone()));
                    break;
                }
            }
            if res.module(m).is_some() {
                break 'roots;
            }
        }
    }
    let tset: BTreeSet<String> = targets.into_iter().collect();
    for t in tset {
        let dest = tmp.join(format!("file-{}", copies.len()));
        let got = _docker_cp(c, cid, &t, &dest)?;
        copies.push(jd! {"src" => t.clone(), "result" => got.clone()});
        let data = if got == "ok" && dest.is_file() { Some(std::fs::read(&dest).map_err(|e| core::io_err(&e, &dest))?) } else { None };
        res.set_file(&t, data);
    }
    res.ran = true;
    Ok(())
}

/// 원장 경로의 스크립트 바이트 정체(원장 `script_sha256` × 작업트리 파일 대조 · 판정 ✗).
pub fn _ledger_script_identity(c: &_Ctx, row: &J) -> HR<J> {
    let want = row.get_str("script_sha256").filter(|s| !s.is_empty()).map(str::to_string);
    let ph = row.get_str("phase").unwrap_or("");
    let (Some(want), true) = (want, PATCH_DIRS().get(ph).is_some() && row["phase"].is_str()) else { return Ok(row.clone()) };
    let p = _patch_base(c)?.join(&patch_dir(ph)).join(&row.get("file").cloned().unwrap_or(J::Null).display());
    let ident = if !p.is_file()? {
        "build-ledger(원장 script_sha256 기록 · 작업트리에 파일 없음 — 실린 바이트 대조 불가)"
    } else if _sha256_of(&p)? == want {
        "build-ledger(원장 script_sha256 = 작업트리 바이트 — 실린 바이트 = 빌드가 실행한 바이트)"
    } else {
        "build-ledger(원장 script_sha256 ≠ 작업트리 바이트 — 빌드 뒤 바뀐 파일)"
    };
    let mut m = row.as_dict().cloned().unwrap_or_default();
    m.insert("script_identity", ident);
    m.insert("ledger_script_sha256", want);
    Ok(J::Dict(m))
}

/// ` · 작업트리 mtime … ≤/> 이미지 Created …` 꼬리(둘 다 관측일 때만).
pub fn _mtime_vs_created(c: &_Ctx, p: Option<&Path>) -> HR<String> {
    let mt = match p {
        Some(p) => _mtime_utc(p)?,
        None => None,
    };
    let created = _image_created(c)?;
    let (Some(mt), Some(created)) = (mt.filter(|s| !s.is_empty()), created.filter(|s| !s.is_empty())) else { return Ok(String::new()) };
    Ok(if mt > created {
        format!(" · 작업트리 mtime {mt} > 이미지 Created {created} — 빌드 뒤 추가·수정")
    } else {
        format!(" · 작업트리 mtime {mt} ≤ 이미지 Created {created} — 빌드 뒤 추가 아님")
    })
}

/// 원장 패치 행에 없는 작업트리 패치의 **관측된** 사유(① 원장 context_inventory ② mtime × 이미지 Created).
pub fn _not_in_ledger_reason(c: &_Ctx, ledger: &J, ph: &str, p: &Path) -> HR<String> {
    not_in_ledger_reason(c, ledger, ph, &PatchPath::Fs(p.to_path_buf()))
}

fn not_in_ledger_reason(c: &_Ctx, ledger: &J, ph: &str, p: &PatchPath) -> HR<String> {
    let slot = format!("build_patch_{ph}");
    let inv = if ledger.is_dict() { ledger.get("context_inventory").cloned() } else { None };
    let when = _mtime_vs_created(c, p.fs())?;
    if let Some(J::Dict(inv)) = &inv {
        if let Some(J::List(l)) = inv.get(&slot) {
            let listed: Vec<String> = l.iter().map(J::display).collect();
            if !listed.contains(&p.name()) {
                let shown = if listed.is_empty() { "[]".to_string() } else { str_list_repr(&listed) };
                return Ok(format!("원장 context_inventory.{slot} = {shown} — 빌드 때 빌드 컨텍스트에 들지 않았다{when}"));
            }
            return Ok(format!("원장 context_inventory.{slot} 에는 있으나 패치 행이 없다(실행 기록 없음){when}"));
        }
    }
    if !when.is_empty() {
        return Ok(format!("원장 context_inventory 없음(옛 원장){when}"));
    }
    Ok("원장 context_inventory 없음(옛 원장) · 이미지 Created 또는 mtime 미관측 — 빌드 뒤 추가 여부 미관측".to_string())
}

fn excluded_row(ph: &str, file: &str, why: String) -> J {
    jd! {"phase" => ph, "file" => file, "why" => why}
}

/// (applied_set, build selection, 메인 원장, 노드별 원장). `probe=false` = 패치 이미지 탐침을 부르지 않는다.
pub fn _applied(c: &_Ctx, probe: bool) -> HR<(J, J, Option<J>, J)> {
    if let Some(v) = applied_memo(c, probe) {
        return Ok(v);
    }
    let (ledger, ledger_source, per_node, mut probes) = _observe_ledger(c)?;
    let sel = _build_selection(c, ledger.as_ref().unwrap_or(&J::Null))?;
    let out = ctx_out(c);
    let dockerfile = sel["dockerfile"].display();
    if !out.join(&dockerfile).is_file() {
        let topo = ctx_topo(c);
        return core::fail(
            "HINT_DOCKERFILE_ABSENT",
            format!("쓰인 Dockerfile 이 없다: output/{topo}/{dockerfile}"),
            Some(&format!("`render_dockerfile.py` 로 output/{topo}/ 를 다시 렌더하거나 셀 env BUILD_DOCKERFILE 을 확인한다.")),
        );
    }
    let dsel = _select_dockerfile(c, &dockerfile)?;
    let df_text = dsel["text"].display();
    let origin = dsel.get("origin_path").filter(|x| x.truthy()).map(J::display);
    if let (Some(origin), true) = (origin, dsel.get("commit").map_or(false, J::truthy)) {
        let parent = pp_parent(&origin);
        if parent != core::rel(&ctx_repo(c), &out)? {
            memo_set(c, "patch_base", jd! {"__gitpath__" => jl![dsel["commit"].display(), parent.clone()]});
        }
    }
    let copies = _dockerfile_copies(&df_text)?;
    let in_recipe: BTreeSet<String> = patch_dirs().into_iter().filter(|(_, d)| copies.contains(d)).map(|(p, _)| p).collect();
    let mut excluded: Vec<J> = Vec::new();
    let mut listing: BTreeMap<String, Vec<PatchPath>> = BTreeMap::new();
    for (ph, d) in patch_dirs() {
        let (patches, known, odd) = _list_patch_dir(&_patch_base(c)?.join(&d))?;
        for name in known {
            excluded.push(excluded_row(&ph, &name, "known-non-patch(패치가 아님 — 명시적 건너뜀)".into()));
        }
        for name in odd {
            excluded.push(excluded_row(&ph, &name, "nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)".into()));
        }
        if !in_recipe.contains(&ph) {
            for p in &patches {
                if copies.contains(&format!("{d}/{}", p.name())) {
                    continue;
                }
                excluded.push(excluded_row(&ph, &p.name(), format!(
                    "excluded_by_recipe({dockerfile} 이 {d}/{} 를 COPY 하지 않는다 — 실행된 적 없음)", p.name())));
            }
        }
        listing.insert(ph, patches);
    }
    let mut applied: Map;
    if let Some(ledger) = &ledger {
        let mut rows = Vec::new();
        for r in _ledger_rows(ledger)? {
            rows.push(_ledger_script_identity(c, &_post_label(&r)?)?);
        }
        let in_ledger: BTreeSet<(String, String)> =
            rows.iter().map(|r| (r["phase"].display(), r["file"].display())).collect();
        let asym = _node_asymmetry(c, ledger_source.as_deref(), &per_node)?;
        let mut others: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        if let Some(a) = &asym {
            let rep = a.get("representative").cloned().unwrap_or(J::Null);
            let mut nodes: Vec<(String, J)> = a["nodes"].as_dict().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()).unwrap_or_default();
            nodes.sort_by(|x, y| x.0.cmp(&y.0));
            for (n, nd) in nodes {
                if rep.as_str() == Some(n.as_str()) {
                    continue;
                }
                for r in nd["patches"].items() {
                    others.entry((r["phase"].display(), r["file"].display())).or_default().push(format!("{n} 원장 {}", r["result"].display()));
                }
            }
        }
        for ph in &in_recipe {
            for p in &listing[ph] {
                if in_ledger.contains(&(ph.clone(), p.name())) {
                    continue;
                }
                let why = match others.get(&(ph.clone(), p.name())) {
                    Some(seen) => format!(
                        "not-in-ledger(대표 {} 원장에 없음 · {} — 노드 비대칭 · 대표 원장 밖이라 싣지 않음)",
                        asym.as_ref().map(|a| a["representative"].display()).unwrap_or_default(),
                        seen.join(" · ")
                    ),
                    None => format!("not-in-ledger(원장에 없음 — {} · 싣지 않음)", not_in_ledger_reason(c, ledger, ph, p)?),
                };
                excluded.push(excluded_row(ph, &p.name(), why));
            }
        }
        applied = Map::new();
        applied.insert("status", "observed");
        applied.insert("source", J::from(ledger_source.clone()));
        applied.insert("patches", J::List(rows));
        applied.insert("probes", J::List(probes.clone()));
        if let Some(pn) = per_node.as_dict().filter(|m| m.len() > 1) {
            let mut items: Vec<(&str, &J)> = pn.iter().collect();
            items.sort_by(|a, b| a.0.cmp(b.0));
            let mut m = Map::new();
            for (n, doc) in items {
                let mut v: Vec<String> = _ledger_rows(doc)?
                    .iter()
                    .map(|r| format!("{}/{}={}", r["phase"].display(), r["file"].display(), r["result"].display()))
                    .collect();
                v.sort();
                m.insert(n, v);
            }
            applied.insert("per_node_patchsets", J::Dict(m));
        }
        if let Some(a) = asym {
            applied.insert("node_asymmetry", a);
        }
    } else {
        let (args, args_src) = _history_build_args(c, &df_text)?;
        let (image, _) = _image_ref(c)?;
        let exists = match image.as_deref().filter(|s| !s.is_empty()) {
            Some(img) => ctx_docker(c, &["image", "inspect", "--format", "{{.Id}}", img], true)?.0 == "ok",
            None => false,
        };
        let pr: Option<Probe> = if probe { Some(_image_probe(c, &df_text)?) } else { None };
        let mut rows = Vec::new();
        for ph in ["pre", "post"] {
            if !in_recipe.contains(ph) {
                continue;
            }
            let loop_ = _patch_loop_failloud(c, &df_text, ph)?;
            let baked: Option<Vec<(String, Vec<u8>, u32)>> = pr.as_ref().filter(|p| p.ran).and_then(|p| p.script_dir(ph).cloned());
            let wt: Vec<(String, PatchPath)> = listing[ph].iter().map(|p| (p.name(), p.clone())).collect();
            let mut names: BTreeSet<String> = wt.iter().map(|x| x.0.clone()).collect();
            if let Some(b) = &baked {
                names.extend(b.iter().map(|x| x.0.clone()).filter(|n| rx_patch_name().is_full(n)));
            }
            for name in names {
                let wp = wt.iter().find(|x| x.0 == name).map(|x| x.1.clone());
                let bb = baked.as_ref().and_then(|b| b.iter().find(|x| x.0 == name).cloned());
                if baked.is_some() && bb.is_none() {
                    let tail = _mtime_vs_created(c, wp.as_ref().and_then(|w| w.fs()))?;
                    excluded.push(excluded_row(ph, &name, format!(
                        "not-in-image(측정 이미지의 패치 디렉터리에 없다 — 빌드 때 빌드 컨텍스트에 들지 않았다{tail} · 싣지 않음)")));
                    continue;
                }
                let (text, ident) = if let Some((_, bytes, mode)) = &bb {
                    let text = String::from_utf8_lossy(bytes).into_owned();
                    let ident = match &wp {
                        None => {
                            push_ship(c, ph, &name, bytes, *mode);
                            "image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다)"
                        }
                        Some(w) if w.read_bytes()? == *bytes => "image-probe(sha256 일치 — 실린 바이트 = 이미지에 구워진 바이트)",
                        Some(_) => {
                            push_ship(c, ph, &name, bytes, *mode);
                            "image-probe(작업트리와 다름 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다)"
                        }
                    };
                    (text, ident.to_string())
                } else {
                    let text = match &wp {
                        Some(w) => read_lossy(w)?,
                        None => String::new(),
                    };
                    let r = pr.as_ref().map(|p| p.record["result"].clone()).filter(J::truthy).map(|x| x.display());
                    (text, format!("unobserved({}) — 작업트리 바이트가 빌드 당시와 같다는 보장 없음", r.unwrap_or_else(|| "이미지 탐침 안 함".into())))
                };
                let env = RecEnv { loop_: Some(loop_.clone()), image_exists: exists, probe: pr.as_ref(), identity_ok: bb.is_some() };
                let mut row = reconstruct(&name, &text, ph, &args, &env)?;
                row.set("script_identity", ident);
                if let Some((_, bytes, _)) = &bb {
                    row.set("image_script_sha256", core::sha256_bytes(bytes));
                }
                rows.push(_post_label(&row)?);
            }
        }
        probes.push(jd! {"tier" => "reconstruction", "result" => if args.truthy() { "done" } else { "build-args-unobserved" }, "source" => args_src.clone()});
        if let Some(p) = &pr {
            probes.push(p.record.clone());
        }
        applied = Map::new();
        applied.insert("status", "unobservable");
        applied.insert("source", "no-build-ledger");
        applied.insert("patches", J::List(rows));
        applied.insert("probes", J::List(probes.clone()));
        applied.insert("reconstruction", jd! {
            "source" => "docker-history+gate-rule · fail-loud-rule · image-probe",
            "image" => J::from(_image_ref(c)?.0),
            "build_args" => args,
            "build_args_source" => args_src,
            "caveat" => "원장 부재 — 결과는 **라벨된 재구성**이다(관측된 원장이 아니다). skipped = history 빌드 \
                         인자가 자기게이트를 끈다 · applied = 이미지에 구워진 스크립트 바이트(탐침)가 skip 경로 \
                         없음 × fail-loud 루프 × 측정 이미지 실재(reconstructed(fail-loud+built)) 또는 이미지에 \
                         구워진 스크립트 바이트 × 선언 마커 전부 실재(image-probe(script-sha+marker) — 패치 후 상태 \
                         관측) · 그 밖 = 미관측(X9) — 탐침이 없으면 applied 는 나오지 않는다. skipped 는 탐침이 \
                         없을 때 작업트리 바이트로 판정한다(script_identity 가 image-probe 가 아니면 빌드 당시 \
                         바이트와 같다는 보장이 없다).",
        });
    }
    excluded.sort_by(|a, b| (a["phase"].display(), a["file"].display()).cmp(&(b["phase"].display(), b["file"].display())));
    applied.insert("excluded", J::List(excluded));
    let result = (J::Dict(applied), sel, ledger, per_node);
    let r = &result;
    c.memo_set(&jl!["applied", probe], jl![r.0.clone(), r.1.clone(), r.2.clone().unwrap_or(J::Null), r.3.clone()]);
    Ok(result)
}

fn push_ship(c: &_Ctx, ph: &str, name: &str, bytes: &[u8], mode: u32) {
    let mut m = c.memo.borrow_mut();
    let slot = m.setdefault("ship_patch", J::dict());
    slot.set(format!("{ph}{SEP}{name}"), jd! {"__bytes__" => hex_encode(bytes), "mode" => mode as i64});
}

pub const NODE_ASYMMETRY_NOTE: &str = "노드별 빌드 원장의 적용 집합이 다르다 — 이 측정은 cluster(노드 전부)가 서빙했다. 대표 원장 하나로 cluster 의 적용 집합을 말하지 않는다(합집합 ✗ · 한 노드 선택 ✗). 실린 패치 파일 · 슬롯 판정의 기준은 대표 원장이다";

/// {representative, nodes{role: {source, patches[{phase,file,result}]}}, note} | None — cluster 셀 · 원장 둘 이상 · 집합이 다를 때만.
pub fn _node_asymmetry(c: &_Ctx, ledger_source: Option<&str>, per_node: &J) -> HR<Option<J>> {
    node_asymmetry(&ctx_node(c), ledger_source, per_node)
}

fn node_asymmetry(node: &str, ledger_source: Option<&str>, per_node: &J) -> HR<Option<J>> {
    let src = ledger_source.unwrap_or("");
    if node != "cluster" || per_node.len() < 2 || !src.starts_with("attestation:") {
        return Ok(None);
    }
    let (base, rep_role) = match src.rfind('#') {
        Some(i) => (&src[..i], &src[i + 1..]),
        None => ("", src),
    };
    let mut items: Vec<(&str, &J)> = per_node.as_dict().map(|m| m.iter().collect()).unwrap_or_default();
    items.sort_by(|a, b| a.0.cmp(b.0));
    let mut nodes = Map::new();
    let mut sets: BTreeSet<BTreeSet<(String, String, String)>> = BTreeSet::new();
    for (n, doc) in items {
        let rows = _ledger_rows(doc)?;
        let patches: Vec<J> = rows.iter().map(|r| jd! {"phase" => r["phase"].clone(), "file" => r["file"].clone(), "result" => r["result"].clone()}).collect();
        sets.insert(rows.iter().map(|r| (r["phase"].display(), r["file"].display(), r["result"].display())).collect());
        nodes.insert(n, jd! {"source" => format!("{base}#{n}"), "patches" => patches});
    }
    if sets.len() < 2 {
        return Ok(None);
    }
    Ok(Some(jd! {"representative" => rep_role, "nodes" => J::Dict(nodes), "note" => NODE_ASYMMETRY_NOTE}))
}

/// 적용 집합(SPEC §5.6): ① attestation 원장 ② 메인 로컬 이미지 원장 ③ X9 재구성 ④ 미관측.
pub fn applied_set(repo: &Path, ev: &J, runner: &J, forward_module: &J) -> HR<J> {
    let c = _ctx(repo, ev, runner, forward_module, &J::Null)?;
    Ok(_applied(&c, true)?.0)
}

// ── render_dockerfile / slave_forward owner boundary (env 형상) ───────────────────────────────────
// `render_module` J: `J::Null` = the real `render_dockerfile` (env_tier through its native launcher `--env-tier KEY`,
// tables as OWNER-MIRROR below). A dict = an injected fake module (tests): attribute name → value, with
// `"env_tier"` = {key: [tier, detail]} (a missing key = KeyError) and `"_env_tier_probe_env"` = the probe env dict.

// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/render_dockerfile.py::NCCL_TRANSPORTS
fn real_rd_table(name: &str) -> Option<J> {
    match name {
        "NCCL_TRANSPORTS" => Some(jd! {
            "socket" => jd! {"NCCL_IB_DISABLE" => "1", "NCCL_NET" => "Socket"},
            "rdma" => jd! {"NCCL_IB_DISABLE" => "0", "NCCL_NET" => "IB"},
        }),
        _ => None,
    }
}
// OWNER-MIRROR: render_dockerfile.py::_ENV_TIER_PROBES · _env_tier_probe_env (only the ① keys whose value carries a probe
// sentinel verbatim: NCCL_IB_HCA `=<hca>` · NCCL_IB_GID_INDEX · the 5 `_IFACE_ENV_KEYS` = the socket_iface value).
fn real_rd_probes() -> J {
    jd! {
        "__probe_hca__" => "interconnect.hca_devices", "__probe_gid__" => "interconnect.gid_index",
        "__probe_iface__" => "interconnect.socket_iface", "__probe_main_host__" => "nodes[main].host",
        "__probe_sub_host__" => "nodes[sub].host", "__probe_main_ssh__" => "nodes[main].ssh_user",
        "__probe_sub_ssh__" => "nodes[sub].ssh_user",
    }
}
fn real_rd_probe_env() -> J {
    let mut m = Map::new();
    m.insert("NCCL_IB_HCA", "=__probe_hca__");
    m.insert("NCCL_IB_GID_INDEX", "__probe_gid__");
    for k in ["NCCL_SOCKET_IFNAME", "GLOO_SOCKET_IFNAME", "TP_SOCKET_IFNAME", "UCX_NET_DEVICES", "OMPI_MCA_btl_tcp_if_include"] {
        m.insert(k, "__probe_iface__");
    }
    J::Dict(m)
}

/// A tier judge: `(tier, detail)` per key (`Err` = the owner raised KeyError/ValueError → text).
pub struct TierFn<'a> {
    rd: &'a J,
    repo: PathBuf,
}

impl TierFn<'_> {
    pub fn tier(&self, key: &str) -> HR<Result<J, String>> {
        if let Some(t) = self.rd.get("env_tier") {
            return Ok(match t.get(key) {
                Some(v) => Ok(v.clone()),
                None => Err(py_repr_str(key)),
            });
        }
        if is_fake_rd(self.rd) {
            return Ok(Ok(derived_tier(self.rd, key)));
        }
        let p = core::run_owner(&self.repo, core::REL_RENDER_DOCKERFILE, &["--env-tier".to_string(), key.to_string()], None, &[], false)?;
        if p.returncode != 0 {
            let e = strip(&p.stderr).to_string();
            return Ok(Err(e.split_once(": ").map(|x| x.1.to_string()).unwrap_or(e)));
        }
        match crate::value::loads(strip(&p.stdout)) {
            Ok(v) => Ok(Ok(jl![v["tier"].clone(), v["detail"].clone()])),
            Err(e) => core::fail("HINT_ENV_TIER_UNKNOWN", format!("render_dockerfile --env-tier {key} 출력 해석 실패: {e}"), None),
        }
    }
}

fn py_repr_str(s: &str) -> String {
    crate::value::py_repr(s)
}

/// The derived 3-tier judge of `_tier_fn` (fake module without `env_tier`).
fn derived_tier(rd: &J, key: &str) -> J {
    let inv = rd["NCCL_INVARIANTS"].get(key).is_some() || rd["CLUSTER_INVARIANTS"].get(key).is_some();
    if inv {
        return jl!["invariant", J::Null];
    }
    let pre = ["NCCL_PRESETS", "CLUSTER_PRESETS"]
        .iter()
        .any(|t| rd[*t].as_dict().map_or(false, |m| m.values().any(|v| v.get(key).is_some())));
    if pre {
        return jl!["preset", J::Null];
    }
    jl!["env", J::Null]
}

/// An injected fake module (a dict that is not a1's `{"__render_module__": ..}` revision handle).
fn is_fake_rd(rd: &J) -> bool {
    rd.is_dict() && rd.get("__render_module__").is_none()
}

fn rd_attr(rd: &J, name: &str) -> Option<J> {
    if is_fake_rd(rd) {
        rd.get(name).cloned()
    } else {
        real_rd_table(name)
    }
}

/// 한 manifest 필드에서 파생되는 여러 env 키의 **실효값 표**(렌더러가 가진 것만 읽는다 · 손목록 ✗).
pub fn _derived_values(rd: &J, field_name: &str, keys: &[String]) -> HR<J> {
    let mut ph = Map::new();
    for k in keys {
        ph.insert(k.clone(), format!("<derived:{field_name}→{k}>"));
    }
    let mut out = Map::new();
    out.insert("field", field_name);
    out.insert("keys", keys.to_vec());
    out.insert("placeholders", J::Dict(ph));
    out.insert("effective_values", J::Null);
    out.insert("value_rule", J::Null);
    out.insert("source", J::Null);
    let last = field_name.rsplit('.').next().unwrap_or(field_name).to_uppercase();
    let table = rd_attr(rd, &format!("{last}S"));
    if let Some(J::Dict(t)) = &table {
        if !t.is_empty() && t.values().all(|v| v.is_dict() && keys.iter().all(|k| v.get(k).is_some())) {
            let mut items: Vec<(&str, &J)> = t.iter().collect();
            items.sort_by(|a, b| a.0.cmp(b.0));
            let mut ev = Map::new();
            let mut rule = Vec::new();
            for (opt, v) in items {
                let mut kv = Map::new();
                for k in keys {
                    kv.insert(k.clone(), v[k.as_str()].display());
                }
                rule.push(format!("{opt} → {}", kv.iter().map(|(k, v)| format!("{k}={}", v.display())).collect::<Vec<_>>().join(" · ")));
                ev.insert(opt, J::Dict(kv));
            }
            out.insert("effective_values", J::Dict(ev));
            out.insert("source", format!("render_dockerfile.{last}S(선택지 → 키 값)"));
            out.insert("rule_text", rule.join(" | "));
            return Ok(J::Dict(out));
        }
    }
    let (penv, sentinels) = if is_fake_rd(rd) {
        (rd.get("_env_tier_probe_env").cloned(), rd.get("_ENV_TIER_PROBES").cloned())
    } else {
        (Some(real_rd_probe_env()), Some(real_rd_probes()))
    };
    if let (Some(penv), Some(J::Dict(sent))) = (penv, sentinels) {
        let s = sent.iter().find(|(_, f)| f.as_str() == Some(field_name)).map(|(sv, _)| sv.to_string());
        if let (true, Some(s)) = (penv.is_dict(), s.filter(|x| !x.is_empty())) {
            if keys.iter().all(|k| penv.get(k).map(J::display).unwrap_or_default().contains(&s)) {
                let mut rule = Map::new();
                for k in keys {
                    rule.insert(k.clone(), penv[k.as_str()].display().replace(&s, &format!("{{{field_name}}}")));
                }
                let text = rule.iter().map(|(k, v)| format!("{k}={}", v.display())).collect::<Vec<_>>().join(" · ");
                out.insert("value_rule", J::Dict(rule));
                out.insert("source", "render_dockerfile.env_tier 탐침 manifest(탐침 값 → {field} 규칙)");
                out.insert("rule_text", text);
                return Ok(J::Dict(out));
            }
        }
    }
    out.insert("source", "미관측(렌더러에 선택지 표·탐침 규칙이 없다 — 지어내지 않는다)");
    out.insert("rule_text", "미관측");
    Ok(J::Dict(out))
}

/// (층 판정 함수, 규칙 이름). Real module = `render_dockerfile.env_tier`; a fake module without `env_tier` = the derived
/// rule from its 3-tier constants (fail-closed `HINT_ENV_TIER_OWNER_MISSING` when those are absent).
/// SIGNATURE: returns the typed judge (`TierFn`) and takes `repo` (the real judge runs the owner's launcher).
pub fn _tier_fn(rd: &J) -> HR<(J, String)> {
    let (_, rule) = tier_fn(rd, Path::new("."))?;
    Ok((J::Null, rule))
}

fn tier_fn<'a>(rd: &'a J, repo: &Path) -> HR<(TierFn<'a>, String)> {
    let f = TierFn { rd, repo: repo.to_path_buf() };
    if !is_fake_rd(rd) || rd.get("env_tier").is_some() {
        return Ok((f, "render_dockerfile.env_tier".to_string()));
    }
    let need = ["NCCL_PRESETS", "NCCL_INVARIANTS", "CLUSTER_PRESETS", "CLUSTER_INVARIANTS"];
    if !need.iter().all(|n| rd.get(n).map_or(false, J::is_dict)) {
        return core::fail(
            "HINT_ENV_TIER_OWNER_MISSING",
            "render_dockerfile.env_tier(key) 도 3층 상수(NCCL_/CLUSTER_ PRESETS·INVARIANTS)도 없다.",
            Some("upstream-version-watch render_dockerfile.py 에 공개 env_tier(key) 를 둔다(SPEC §6.2)."),
        );
    }
    Ok((f, "derived(render_dockerfile 3층 상수 · env_tier 부재)".to_string()))
}

rx!(rx_env_line, r"([A-Za-z_][A-Za-z0-9_]*)=(.*)");
/// slave_forward `read_env_first`: first `KEY=` line wins · raw text after the first `=`.
// OWNER-MIRROR: .claude/skills/upstream-version-watch/scripts/slave_forward.py::read_env_first
fn read_env_first(path: &Path) -> Map {
    let mut out = Map::new();
    if !path.is_file() {
        return out;
    }
    let text = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    for raw in splitlines(&text) {
        if let Some(m) = rx_env_line().fullmatch(raw) {
            let k = grp(&m, 1).unwrap_or("");
            if !out.contains_key(k) {
                out.insert(k, grp(&m, 2).unwrap_or(""));
            }
        }
    }
    out
}

/// env 형상 텍스트와 규칙 이름(키는 항상 남긴다 · 측정 후 재생성이면 리터럴 값 미실림 · 배포 강도 PII 백스톱).
/// SIGNATURE: `derived_out: Option<&mut Vec<J>>` (an out-list; the stub's `&J` could not be appended to).
#[allow(clippy::too_many_arguments)]
pub fn _env_shape(
    c_repo: &Path,
    path: &Path,
    kind: &str,
    render_module: &J,
    sf: &J,
    regen: &J,
    measured: Option<&str>,
    derived_out: &J,
    rerender: &J,
) -> HR<(String, String)> {
    let _ = derived_out; // a `&J` cannot be appended to — callers that need the derived rows call `_env_shape_d`
    _env_shape_d(c_repo, path, kind, render_module, sf, regen, measured, None, rerender)
}

/// `_env_shape` with the `derived_out` out-list (Python appends the derived-key rows to the caller's list).
#[allow(clippy::too_many_arguments)]
pub fn _env_shape_d(
    c_repo: &Path,
    path: &Path,
    kind: &str,
    render_module: &J,
    sf: &J,
    regen: &J,
    measured: Option<&str>,
    mut derived_out: Option<&mut Vec<J>>,
    rerender: &J,
) -> HR<(String, String)> {
    let _ = sf;
    let env = read_env_first(path);
    let renderer = match kind {
        "cluster" => Some("--cluster-envfile --topology multi --manifest output/<t>/manifest.yaml"),
        "interconnect" => Some("--nccl-envfile --manifest output/<t>/manifest.yaml"),
        _ => None,
    };
    let mut lines: Vec<String> = vec![
        "# 형상 템플릿 — 발행 시점 형상이다(운영자 실값 ✗). 키는 항상 남는다: 키를 지우면 수신자는 그 변수의".into(),
        "# 존재 자체를 모른다(2026-09-06 · 멀티 클러스터 5변수가 그렇게 빠졌다).".into(),
    ];
    let regen_on = regen.truthy();
    if regen_on {
        lines.push(format!(
            "# ⚠ 측정 후 재생성: 이 파일은 측정({} · {}) 뒤 {}(mtime 관측)에 다시 쓰였다.",
            regen["measured_utc"].display(), regen["measured_source"].display(), regen["mtime_utc"].display()
        ));
        lines.push(format!("#   측정 당시 값은 관측되지 않았다 — 리터럴 값은 싣지 않고 `{REGENERATED_VALUE}` 로 둔다."));
        lines.push("#   키 목록도 재생성 시점의 형상이다(측정 당시 키 집합과 같다는 보장 없음) · `<manifest.…>` 표지는 파생 규칙이라 남긴다.".into());
    }
    let keep = if rerender.truthy() {
        let r = rerender;
        lines.push(format!("# ⚠ 측정 당시 판본 재렌더 — generated_by: {} · verification: generated-unverified(실행 바이트 아님)", r["generated_by"].display()));
        lines.push(format!(
            "#   작업트리 envs/{} 는 측정({} · {}) 뒤 {}(mtime 관측)에 다시 쓰였다 — 그 바이트는 싣지 않는다.",
            r["file"].display(), r["measured_utc"].display(), r["measured_source"].display(), r.get("mtime_utc").cloned().unwrap_or(J::Null).display()
        ));
        lines.push(format!("#   이 형상 = {}.", r["method"].display()));
        lines.push(format!("#   입력: {}.", r["manifest_basis"].display()));
        lines.push(format!("#   측정 실행이 실제로 읽은 바이트와 같다는 관측은 없다 — 측정 당시 env 관측값 = {ENV_OBSERVED_POINTER}."));
        format!("값 유지 — 측정 당시 판본 렌더러 {} 산출(실행 바이트 아님)", r["rev12"].display())
    } else if regen_on {
        "리터럴 값 미실림 — 측정 후 재생성".to_string()
    } else if let Some(m) = measured.filter(|s| !s.is_empty()) {
        format!("값 유지 — 재현 사실(mtime ≤ 측정 {m} 확인)")
    } else {
        "값 유지 — 측정 시각 미관측이라 측정 당시 값인지 대조하지 못했다".to_string()
    };
    let fname = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let rule: String;
    let regen_mask = |shaped: String| -> String {
        if regen_on && !rx_placeholder_value().is_match(&shaped) { REGENERATED_VALUE.to_string() } else { shaped }
    };
    if kind == "cluster" || kind == "interconnect" {
        let (tier, r) = tier_fn(render_module, c_repo)?;
        rule = r;
        let rev = if rerender.truthy() {
            format!("(측정 당시 판본 = git show {}:{}).", rerender["rev12"].display(), core::REL_RENDER_DOCKERFILE)
        } else {
            ".".to_string()
        };
        lines.push(format!("# 값의 정본 = 자기 manifest → `render_dockerfile.py {}` 로 다시 파생한다{rev}", renderer.unwrap_or("None")));
        lines.push(format!("# 층 규칙: {rule} (①env=치환 · ②preset·③invariant={keep})"));
        let mut tiers: Vec<(String, String, J)> = Vec::new();
        for (key, _) in env.iter() {
            let got = match tier.tier(key)? {
                Ok(g) => g,
                Err(e) => {
                    return core::fail(
                        "HINT_ENV_TIER_UNKNOWN",
                        format!("{fname}:{key} 를 env_tier 가 판정하지 못했다: {e}"),
                        Some("render_dockerfile 3층(NCCL_/CLUSTER_ PRESETS·INVARIANTS)에 없는 키다 — 파일이 낡았는지 본다."),
                    )
                }
            };
            let ok = got.as_list().map_or(false, |l| l.len() == 2 && l[0].as_str().map_or(false, |t| ENV_TIERS.contains(&t)));
            if !ok {
                return core::fail(
                    "HINT_ENV_TIER_UNKNOWN",
                    format!("{fname}:{key} 의 env tier 가 규약 밖이다: {}", got.repr()),
                    Some(&format!("env_tier 는 {} 중 하나와 세부(또는 None)를 돌려준다.", J::List(ENV_TIERS.iter().map(|s| J::from(*s)).collect()).repr().replacen('[', "(", 1).replacen(']', ")", 1))),
                );
            }
            tiers.push((key.to_string(), got.at(0).display(), got.at(1).clone()));
        }
        let mut by_field: Vec<(String, Vec<String>)> = Vec::new();
        for (key, t, detail) in &tiers {
            if t == "env" && detail.truthy() {
                let f = _field_of(detail)?;
                match by_field.iter_mut().find(|(x, _)| *x == f) {
                    Some(slot) => slot.1.push(key.clone()),
                    None => by_field.push((f, vec![key.clone()])),
                }
            }
        }
        let mut multi: Vec<(String, Vec<String>)> = by_field.into_iter().filter(|(_, ks)| ks.len() > 1).collect();
        multi.sort();
        for (field_name, keys) in &multi {
            let drow = _derived_values(render_module, field_name, keys)?;
            lines.push(format!(
                "# 파생 키 {field_name} → {} — 키마다 자기 자리표시 `<derived:{field_name}→<KEY>>` · 실효값 규칙: {}",
                keys.join(" · "), drow["rule_text"].display()
            ));
            if let Some(d) = derived_out.as_deref_mut() {
                let mut m = drow.as_dict().cloned().unwrap_or_default();
                m.remove("rule_text");
                d.push(J::Dict(m));
            }
        }
        for ((key, value), (_, t, detail)) in env.iter().zip(tiers.iter()) {
            let value = value.display();
            let df = if detail.truthy() { Some(_field_of(detail)?) } else { None };
            let shaped = if t == "env" && df.as_ref().map_or(false, |f| multi.iter().any(|(x, _)| x == f)) {
                format!("<derived:{}→{key}>", df.unwrap())
            } else if t == "env" {
                let d = detail.display();
                if detail.truthy() && d.starts_with("manifest") {
                    format!("<{d}>")
                } else {
                    _manifest_ph(&if detail.truthy() { d } else { format!("{kind}.{}", key.to_lowercase()) })?
                }
            } else if t == "unknown" {
                _shape_key_axis(key, strip(&value))?
            } else {
                strip(&value).to_string()
            };
            let shaped = regen_mask(shaped);
            lines.push(format!("{key}={shaped}    # tier={t}{}", if t == "unknown" { "(키-축 규칙)" } else { "" }));
        }
    } else {
        rule = "key-axis(IDENTITY_KEY_AXIS · TUNING_KEYS · 2026-09-06)".to_string();
        lines.push(format!("# 층 규칙: {rule} — 값의 정본은 manifest 의 동명 필드다 · 튜닝·그 밖 값 = {keep}."));
        for (key, value) in env.iter() {
            let shaped = regen_mask(_shape_key_axis(key, strip(&value.display()))?);
            lines.push(format!("{key}={shaped}"));
        }
    }
    let text = lines.join("\n") + "\n";
    let terms = crate::pii::require_terms(c_repo)?;
    let hits = crate::pii::scan_text(&text, Some(&terms), "deploy", &[], false, false, None)?;
    if !hits.is_empty() {
        return core::fail(
            "HINT_ENV_SHAPE_PII",
            format!("{fname} 형상화 뒤에도 배포 금지 패턴이 남았다(덧칠 ✗):\n  {}", crate::pii::render_hits(&hits, 10)?),
            Some("해당 키를 IDENTITY_KEY_AXIS 에 편입하거나(render 3층 파일이면 env_tier 를) 고친다."),
        );
    }
    Ok((text, rule))
}

/// `.env.cluster`/`.env.interconnect` = render 3층 치환 · 그 밖 = 키-축 규칙. `measured_utc` 뒤 mtime = 측정 후 재생성.
pub fn env_shape(repo: &Path, path: &Path, kind: &str, render_module: &J, forward_module: &J, measured_utc: Option<&str>) -> HR<String> {
    let measured = match measured_utc.filter(|s| !s.is_empty()) {
        Some(m) => _utc_z(Some(m))?,
        None => None,
    };
    let mt = _mtime_utc(path)?;
    let regen = match (&measured, &mt) {
        (Some(ms), Some(m)) if !ms.is_empty() && !m.is_empty() && m > ms => {
            jd! {"mtime_utc" => m.clone(), "measured_utc" => ms.clone(), "measured_source" => "호출부 measured_utc"}
        }
        _ => J::Null,
    };
    let r = core::resolve(repo);
    Ok(_env_shape_d(&r, path, kind, render_module, forward_module, &regen, measured.as_deref(), None, &J::Null)?.0)
}

// ── 서빙 러너 파생 (ctx) ─────────────────────────────────────────────────────────────────────
/// `_select_tracked(...)["bytes"]` as bytes (a J str, or `{"__bytes__": hex}`).
fn sel_bytes(sel: &J) -> Vec<u8> {
    _j_bytes(&sel["bytes"]).unwrap_or_default()
}

/// (서빙 경로가 부르는 러너 스크립트 이름, 모델 러너 참조) — compose command 와 러너의 `source` 에서 파생.
pub fn _runner_names(c: &_Ctx) -> HR<(Vec<String>, Vec<String>)> {
    if ctx_compose_path(c).is_none() {
        return Ok((vec![], vec![]));
    }
    let text = _compose_text(c)?;
    let cell_sh = format!("{}.sh", ctx_cell(c));
    let mut names: Vec<String> = Vec::new();
    let mut model_refs: Vec<String> = Vec::new();
    for (_, block) in _serving_blocks(&text)? {
        for body in _command_texts(&block)? {
            for r in rx_configs_ref().findall(&body) {
                if r.contains("${") || r == cell_sh {
                    model_refs.push(r);
                } else if !names.contains(&r) {
                    names.push(r);
                }
            }
        }
    }
    let asset = ctx_shared_asset_dir(c)?;
    let scan = |names: &mut Vec<String>, text: &str| -> Vec<String> {
        let new: Vec<String> = rx_sourced().findall(text).into_iter().filter(|s| !names.contains(s) && *s != cell_sh).collect();
        for s in &new {
            if !names.contains(s) {
                names.push(s.clone());
            }
        }
        new
    };
    let mut queue: Vec<String> = names.clone();
    if let Some(sh) = ctx_triplet(c, "sh").filter(|p| p.is_file()) {
        let t = _read(&sh)?;
        queue.extend(scan(&mut names, &t));
    }
    while !queue.is_empty() {
        let n = queue.remove(0);
        let f = asset.join(&n);
        if f.is_file() {
            let sel = _select_tracked(c, &f)?;
            let t = String::from_utf8_lossy(&sel_bytes(&sel)).into_owned();
            queue.extend(scan(&mut names, &t));
        }
    }
    let refs: BTreeSet<String> = model_refs.into_iter().collect();
    Ok((names, refs.into_iter().collect()))
}

rx!(rx_arm_key_line, r#"^\s*_cfg="([^"\n]+)"\s*$"#, pyre::M);
rx!(rx_arm_key_expr, r"\$\{([A-Za-z_][A-Za-z0-9_]*):-(.*)\}");
rx!(rx_arm_echo, r#"echo\s+"(\[[A-Za-z_]+\][^"$]*?)\s*\$\{_patch\}"#);
rx!(rx_ansi, "\x1b\\[[0-9;]*[A-Za-z]");

/// `str(rev)[:12]`.
fn rev12(rev: &J) -> String {
    core::head_chars(&rev.display(), 12)
}

/// {chain, default, source} — 측정 당시 개정 armer 의 `_cfg="${A:-${B:-lit}}"` 줄(해석 실패 = K11 규칙 · fail-loud 폴백).
pub fn _arm_key_chain(c: &_Ctx) -> HR<J> {
    if let Some(v) = memo_get(c, "arm_key_chain") {
        return Ok(v);
    }
    let mut out = jd! {"chain" => jl!["CONFIG_FILE"], "default" => "default",
                       "source" => format!("{RUNTIME_PATCH_ARMER} 부재 — K11 규칙(CONFIG_FILE)으로 판정")};
    let asset = ctx_shared_asset_dir(c)?.join(RUNTIME_PATCH_ARMER);
    if asset.is_file() {
        let sel = _select_tracked(c, &asset)?;
        let rev = sel.get("commit").filter(|x| x.truthy()).cloned().unwrap_or_else(|| sel.get("shipped").cloned().unwrap_or(J::Null));
        let text = String::from_utf8_lossy(&sel_bytes(&sel)).into_owned();
        let mut expr: Option<String> = rx_arm_key_line().search(&text).and_then(|m| grp(&m, 1).map(str::to_string));
        let mut chain: Vec<String> = Vec::new();
        while let Some(e) = expr.clone() {
            let Some(em) = rx_arm_key_expr().fullmatch(&e) else { break };
            chain.push(grp(&em, 1).unwrap_or("").to_string());
            expr = Some(grp(&em, 2).unwrap_or("").to_string());
        }
        out = match expr {
            Some(e) if !chain.is_empty() && !e.contains('$') => jd! {
                "chain" => chain.clone(), "default" => e.clone(),
                "source" => format!("{RUNTIME_PATCH_ARMER} 측정 당시 개정({}) `_cfg=` 키 체인 {} > '{e}'", rev12(&rev), chain.join(" > ")),
            },
            _ => jd! {
                "chain" => jl!["CONFIG_FILE"], "default" => "default",
                "source" => format!("{RUNTIME_PATCH_ARMER} 측정 당시 개정({})의 `_cfg=` 줄을 해석하지 못했다 — \
                                     K11 규칙(CONFIG_FILE)으로 판정(⚠ 러너 모양 변경 확인)", rev12(&rev)),
            },
        };
    }
    memo_set(c, "arm_key_chain", out.clone());
    Ok(out)
}

/// slave_forward.derive(측정 당시 compose 개정) — 스모크와 같은 함수. 거부는 원인을 번역해 차단한다. Rows = `[{key, value}]`.
pub fn _slave_forward_rows(c: &_Ctx) -> HR<J> {
    let compose = _compose_file(c)?;
    let out = ctx_out(c);
    match ctx_sf_derive(c, ctx_triplet(c, "env").as_deref(), &out.join(".env"), compose.as_deref(), &out.join("envs").join(".env.cluster")) {
        Ok(rows) => Ok(J::List(rows.into_iter().map(|(k, v)| jd! {"key" => k, "value" => v}).collect())),
        Err(e) if e.code == "py:SystemExit" => core::fail(
            "HINT_SUB_RECIPE_DERIVE_FAILED",
            format!("slave_forward.derive 가 거부했다(SystemExit {}).", e.message),
            Some("stderr 의 [slave-forward] FAIL 사유를 고친다(값에 공백·따옴표 등 — 원격 shell prefix 안전 문자 밖)."),
        ),
        Err(e) if e.is_py() => core::fail(
            "HINT_SUB_RECIPE_DERIVE_FAILED",
            match e.message.split_once(SEP) {
                Some((code, msg)) => format!("slave_forward.derive 거부: {code}: {msg}"),
                None => format!("slave_forward.derive 거부: {}: {}", e.code.trim_start_matches("py:"), e.message),
            },
            Some("사유 코드의 자리(셀 env 값 · compose 모양 · render_dockerfile.env_tier)를 고친다 — 전달 목록을 손으로 적지 않는다."),
        ),
        Err(e) => Err(e),
    }
}

/// 역할별 런타임 패치 무장 판정 {chain, master:{stem,key,armed,patch}, slave:{…}|None}. `forwards` = `[{key, value}]` | None.
pub fn _arm_roles(c: &_Ctx, forwards: &J) -> HR<J> {
    let chain = _arm_key_chain(c)?;
    let mut menv = match ctx_triplet(c, "env") {
        Some(p) => _read_env(ctx_sf(c, "read_env_first")?, Some(&p))?,
        None => _read_env(ctx_sf(c, "read_env_first")?, None)?,
    };
    if let J::Dict(m) = &mut menv {
        m.setdefault("CONFIG_FILE", J::from(ctx_cell(c)));
    }
    let out_dir = ctx_out(c);
    let judge = |values: &J| -> HR<J> {
        let (stem, k) = _arm_stem(&chain, values)?;
        let patch = out_dir.join("configs").join(format!("{stem}_patch.py"));
        Ok(jd! {"stem" => stem.clone(), "key" => k, "armed" => patch.is_file(), "patch" => format!("{stem}_patch.py")})
    };
    let mut out = jd! {"chain" => chain.clone(), "master" => judge(&menv)?, "slave" => J::Null};
    if ctx_topo(c) == "multi" && ctx_compose_path(c).is_some() {
        let rows = if forwards.is_null() { _slave_forward_rows(c)? } else { forwards.clone() };
        let mut senv = Map::new();
        for f in rows.items() {
            let v = f.get("value").cloned().unwrap_or(J::Null);
            senv.insert(f.get("key").cloned().unwrap_or(J::from("")).display(), if v.truthy() { v.display() } else { String::new() });
        }
        let compose = _compose_file(c)?;
        for k in ctx_sf_never_forward(c) {
            let d = ctx_sf_compose_default(c, compose.as_deref(), &k)?.unwrap_or_default();
            senv.insert(k, d);
        }
        out.set("slave", judge(&J::Dict(senv))?);
    }
    Ok(out)
}

/// Python `re.escape` (3.7+: only the special characters).
fn re_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\u{0b}\u{0c}".contains(ch) {
            o.push('\\');
        }
        o.push(ch);
    }
    o
}

/// {signature, measured[{path,lines,patch}], failed[{path,lines,patch,role}], source} — 무장 서명 줄 관측(읽기 전용).
pub fn _arm_log_observations(c: &_Ctx) -> HR<J> {
    if let Some(v) = memo_get(c, "arm_log_obs") {
        return Ok(v);
    }
    let mut out = jd! {"signature" => J::Null, "measured" => jl![], "failed" => jl![], "source" => J::Null};
    let asset = ctx_shared_asset_dir(c)?.join(RUNTIME_PATCH_ARMER);
    let mut sig: Option<String> = None;
    if asset.is_file() {
        let t = String::from_utf8_lossy(&sel_bytes(&_select_tracked(c, &asset)?)).into_owned();
        sig = rx_arm_echo().search(&t).and_then(|m| grp(&m, 1).map(|s| strip(s).to_string()));
    }
    let Some(sig) = sig.filter(|s| !s.is_empty()) else {
        out.set("source", format!("{RUNTIME_PATCH_ARMER} 의 무장 echo 서명을 해석하지 못했다 — 로그 관측 안 함"));
        memo_set(c, "arm_log_obs", out.clone());
        return Ok(out);
    };
    out.set("signature", sig.clone());
    let pat = Re::new(&format!(r"{}\s*(/\S+?_patch\.py)", re_escape(&sig)), 0);
    let scan = |f: &Path| -> HR<(Vec<i64>, Vec<String>)> {
        let b = std::fs::read(f).map_err(|e| core::io_err(&e, f))?;
        let text = String::from_utf8_lossy(&b).into_owned();
        let (mut lines, mut patches) = (Vec::new(), Vec::new());
        for (i, ln) in text.split('\n').enumerate() {
            let clean = rx_ansi().sub_literal(ln, "");
            if let Some(mm) = pat.search(&clean) {
                lines.push(i as i64 + 1);
                let p = grp(&mm, 1).unwrap_or("").to_string();
                if !patches.contains(&p) {
                    patches.push(p);
                }
            }
        }
        Ok((lines, patches))
    };
    let repo = ctx_repo(c);
    let cell = ctx_cell(c);
    let sweep = _get(ctx_ev(c), "sweep", &J::Null)?;
    let sdir = if sweep.is_dict() { sweep.get_str("dir").map(str::to_string) } else { None };
    let mut measured = Vec::new();
    if let Some(sdir) = sdir.filter(|d| repo.join(d).is_dir()) {
        for name in [format!("lite_engine_head_{cell}.log"), format!("lite_engine_{cell}.log")] {
            let f = repo.join(&sdir).join(&name);
            if f.is_file() {
                let (lines, patches) = scan(&f)?;
                if !lines.is_empty() {
                    measured.push(jd! {"path" => core::rel(&repo, &f)?, "lines" => J::List(lines.into_iter().map(J::Int).collect()), "patch" => patches});
                }
            }
        }
    }
    let mut failed = Vec::new();
    let fdir = ctx_out(c).join("benchlog").join(format!("serve_fail_{cell}"));
    if fdir.is_dir() {
        let mut logs: Vec<PathBuf> = std::fs::read_dir(&fdir)
            .map_err(|e| core::io_err(&e, &fdir))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "log") && p.file_name().map_or(false, |n| n.len() > 4))
            .collect();
        logs.sort();
        for f in logs {
            let (lines, patches) = scan(&f)?;
            if lines.is_empty() {
                continue;
            }
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let head = name.split('_').next().unwrap_or("");
            let role = if head == "master" || head == "slave" { head.to_string() } else { "<역할 미상>".to_string() };
            failed.push(jd! {"path" => core::rel(&repo, &f)?, "lines" => J::List(lines.into_iter().map(J::Int).collect()), "patch" => patches, "role" => role});
        }
    }
    out.set("measured", J::List(measured));
    out.set("failed", J::List(failed));
    out.set("source", format!("{RUNTIME_PATCH_ARMER} 무장 echo 서명 `{sig}` × 측정 기동 로그(스윕 lite 캡처) · 실패 기동 로그(serve_fail_{cell}/)"));
    memo_set(c, "arm_log_obs", out.clone());
    Ok(out)
}

/// 이 셀이 런타임 패치를 **실제로** 무장하는가(키 체인 = 측정 당시 armer 개정).
pub fn _runtime_patch_armed(c: &_Ctx) -> HR<(bool, String)> {
    let r = _arm_roles(c, &J::Null)?;
    let (m, s) = (&r["master"], &r["slave"]);
    if m["armed"].truthy() || (s.truthy() && s["armed"].truthy()) {
        let parts: Vec<String> = [("master", m), ("slave", s)]
            .iter()
            .filter(|(_, x)| x.truthy() && x["armed"].truthy())
            .map(|(role, x)| format!("{role} {}({})", x["patch"].display(), x["key"].display()))
            .collect();
        return Ok((true, format!("armed({})", parts.join(" · "))));
    }
    let slave = if s.truthy() { format!(" · slave {}({}) 없음", s["patch"].display(), s["key"].display()) } else { String::new() };
    Ok((false, format!(
        "output/{}/configs/{} 없음({}){slave} — arm_patch.sh 는 no-op · {}",
        ctx_topo(c), m["patch"].display(), m["key"].display(), r["chain"]["source"].display()
    )))
}
