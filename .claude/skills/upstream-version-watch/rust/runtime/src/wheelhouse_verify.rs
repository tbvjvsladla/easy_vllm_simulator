//! `native_wheelhouse.py verify`: a clean venv, `pip install --no-index`, `pip check`, the installed-set comparison, `ldd` over the vLLM
//! extension modules and the torch/vllm import probe, as one JSON verdict. The commands it runs (`python3.12 -m venv`, the venv's own pip and
//! interpreter, `ldd`) are the thing under test; this program itself runs no Python of its own.

use crate::wheelhouse_build::{is_utc, load_accept_file, TOOL};
use crate::wheelhouse_source::canon;
use crate::wheelhouse_util::*;
use std::collections::BTreeSet;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const IMPORT_PROBE: &str = r#"
import json, sys
import torch, vllm
ok = bool(torch.cuda.is_available())
print(json.dumps({"torch": torch.__version__, "torch_cuda": torch.version.cuda, "vllm": getattr(vllm, "__version__", None),
                  "torch_file": torch.__file__, "vllm_file": vllm.__file__, "cuda_available": ok,
                  "device_count": torch.cuda.device_count() if ok else 0}))
sys.exit(0 if ok else 7)
"#;
const GATE_RESOLVER: &str = "resolver(pip install --no-index 리졸버 · pip check 0)";
const GATE_PARITY: &str = "image-parity(accept-file: --no-deps 정확 핀 · 설치 집합 == closure 핀 · pip check ⊆ image_inherent_conflicts)";
const FIXED_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

type Env = Vec<(String, String)>;

fn set_env(e: &mut Env, k: &str, v: String) {
    match e.iter_mut().find(|(a, _)| a == k) {
        Some(slot) => slot.1 = v,
        None => e.push((k.to_string(), v)),
    }
}
fn env_get<'a>(e: &'a Env, k: &str) -> &'a str {
    e.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// `_clean_env(extra)`.
fn clean_env(extra: &Env) -> Env {
    let mut keep: Env = Vec::new();
    for k in ["HOME", "USER", "LANG", "LC_ALL", "TERM"] {
        if let Ok(v) = std::env::var(k) {
            keep.push((k.to_string(), v));
        }
    }
    set_env(&mut keep, "PATH", FIXED_PATH.to_string());
    for (k, v) in [("PIP_NO_INDEX", "1"), ("PIP_CONFIG_FILE", "/dev/null"), ("PIP_DISABLE_PIP_VERSION_CHECK", "1"), ("PIP_NO_INPUT", "1"), ("PYTHONNOUSERSITE", "1")] {
        set_env(&mut keep, k, v.to_string());
    }
    for (k, v) in extra {
        set_env(&mut keep, k, v.clone());
    }
    keep
}

fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() > n {
        c[c.len() - n..].iter().collect()
    } else {
        s.to_string()
    }
}

struct Out {
    rc: i64,
    stdout: String,
    stderr: String,
}

fn run(argv: &[String], env: &Env, cwd: Option<&Path>, timeout: Option<Duration>) -> Result<Out, Err> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).env_clear().envs(env.iter().map(|(k, v)| (k, v))).stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    let mut child = cmd.spawn().map_err(|e| Err::Crash(format!("{}: {e}: {:?}", if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" }, argv[0])))?;
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ho = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let he = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(s) => break s,
            None => {
                if timeout.map_or(false, |t| start.elapsed() > t) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Err::Crash(format!("subprocess.TimeoutExpired: Command '{}' timed out after {} seconds", argv[0], timeout.unwrap().as_secs())));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    let rc = match status.code() {
        Some(c) => c as i64,
        None => -(status.signal().unwrap_or(0) as i64),
    };
    let stdout = String::from_utf8_lossy(&ho.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&he.join().unwrap_or_default()).into_owned();
    Ok(Out { rc, stdout, stderr })
}

fn strv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn set(o: &mut J, k: &str, v: J) {
    if let J::Obj(items) = o {
        match items.iter_mut().find(|(a, _)| a == k) {
            Some(slot) => slot.1 = v,
            None => items.push((k.to_string(), v)),
        }
    }
}
fn get_mut<'a>(o: &'a mut J, k: &str) -> &'a mut J {
    match o {
        J::Obj(items) => &mut items.iter_mut().find(|(a, _)| a == k).expect("key").1,
        _ => panic!("not an object"),
    }
}
fn result_of(g: &J, name: &str) -> Option<String> {
    g.get(name).and_then(|x| x.get("result")).and_then(|x| x.str()).map(String::from)
}
fn pass(b: bool) -> J {
    J::s(if b { "PASS" } else { "FAIL" })
}
fn jb(b: bool) -> J {
    J::Bool(b)
}
fn seconds(t: Instant) -> J {
    J::Float((t.elapsed().as_secs_f64() * 10.0).round() / 10.0)
}
fn py_list_repr(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "))
}

fn pip_names(out: &str) -> Option<Vec<(String, String)>> {
    let doc = loads(if out.is_empty() { b"[]" } else { out.as_bytes() }).ok()?;
    Some(
        doc.arr()
            .iter()
            .map(|x| (x.get("name").and_then(|n| n.str()).unwrap_or("").to_string(), x.get("version").and_then(|n| n.str()).unwrap_or("").to_string()))
            .collect(),
    )
}

fn nonempty_dir(p: &Path) -> bool {
    std::fs::read_dir(p).map(|mut r| r.next().is_some()).unwrap_or(false)
}

fn rglob_so(dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if e.file_name().to_string_lossy().ends_with(".so") {
            out.push(p.to_string_lossy().into_owned());
        }
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            rglob_so(&p, out);
        }
    }
}

/// `du` as the original computes it: `st_blocks * 512` of every non-directory entry `os.walk` lists (symlinks to directories are not listed as files).
fn du(p: &Path) -> i64 {
    let mut tot = 0i64;
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    for e in rd.flatten() {
        let path = e.path();
        let Ok(lm) = std::fs::symlink_metadata(&path) else { continue };
        if lm.file_type().is_symlink() {
            if std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
                continue;
            }
            tot += lm.blocks() as i64 * 512;
        } else if lm.is_dir() {
            tot += du(&path);
        } else {
            tot += lm.blocks() as i64 * 512;
        }
    }
    tot
}

struct TmpDir(PathBuf);
impl TmpDir {
    fn new() -> Result<TmpDir, Err> {
        let base = std::env::var("TMPDIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into());
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        for i in 0..100u32 {
            let p = PathBuf::from(&base).join(format!("tmp{:08x}", seed.wrapping_add(i).wrapping_mul(2654435761) ^ std::process::id()));
            if std::fs::create_dir(&p).is_ok() {
                return Ok(TmpDir(p));
            }
        }
        Err(Err::Crash("FileExistsError: No usable temporary directory name found".into()))
    }
}
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key_error(k: &str) -> Err {
    Err::Crash(format!("KeyError: '{k}'"))
}

pub fn verify(whdir: &Path, venv: &Path, generated_utc: &str, diagnostic: bool, accept_path: Option<&Path>) -> Result<J, Err> {
    let closure = loads(&std::fs::read(whdir.join("closure.json"))?).map_err(|_| Err::Crash("json.decoder.JSONDecodeError: closure.json".into()))?;
    let mut declared: Vec<String> = Vec::new();
    let mut ameta = None;
    if let Some(ap) = accept_path {
        let (_, meta) = load_accept_file(ap)?;
        let doc = loads(&std::fs::read(ap)?).map_err(|_| Err::Crash("JSONDecodeError".into()))?;
        let cid = closure.get("image").filter(|x| x.truthy()).and_then(|i| i.get("id")).and_then(|x| x.str()).map(String::from);
        let ids = if meta.image_ids.is_empty() { vec![meta.image_id.clone()] } else { meta.image_ids.clone() };
        if !cid.as_ref().map_or(false, |c| ids.contains(c)) {
            return wh(format!("승인 파일 이미지 {} ∌ closure 이미지 {}", py_list_repr(&ids), cid.unwrap_or_else(|| "None".into())));
        }
        declared = doc.get("image_inherent_conflicts").map(|a| a.arr()).unwrap_or(&[]).iter().map(|e| e.get("pip_check_line").and_then(|x| x.str()).unwrap_or("").trim().to_string()).collect();
        ameta = Some(meta);
    }
    let parity = ameta.is_some();
    let closure_status = closure.get("status").cloned().unwrap_or(J::Null);
    let status_pass = closure_status.str() == Some("PASS");
    let accept_j = match &ameta {
        Some(m) => obj(vec![("sha256", J::s(&m.sha256)), ("approved_by", J::s(&m.approved_by)), ("approved_utc", J::s(&m.approved_utc)), ("image_id", J::s(&m.image_id))]),
        None => J::Null,
    };
    let mut res = obj(vec![
        ("schema_version", J::Int(1)),
        ("kind", J::s("native_wheelhouse_verify")),
        ("tool", J::s(TOOL)),
        ("generated_utc", J::s(generated_utc)),
        ("wheelhouse_dir", J::s(&whdir.to_string_lossy())),
        ("venv", J::s(&venv.to_string_lossy())),
        ("closure_status", closure_status),
        ("gate_definition", J::s(if parity { GATE_PARITY } else { GATE_RESOLVER })),
        ("accept", accept_j),
        ("diagnostic_only", jb(diagnostic)),
        ("gates", obj(vec![])),
        ("verdict", J::s("FAIL")),
    ]);
    macro_rules! gate {
        ($k:expr, $v:expr) => {
            set(get_mut(&mut res, "gates"), $k, $v)
        };
    }
    if !status_pass && !diagnostic {
        gate!("closure", obj(vec![("result", J::s("FAIL")), ("detail", J::s("closure.json status≠PASS — --diagnostic 없이는 설치하지 않는다"))]));
        return Ok(res);
    }
    gate!("closure", obj(vec![("result", J::s(if status_pass { "PASS" } else { "FAIL(diagnostic 진행)" }))]));
    if venv.exists() && nonempty_dir(venv) {
        return wh(format!("venv 경로가 비어 있지 않다(덮어쓰지 않는다): {}", venv.display()));
    }
    let env = clean_env(&vec![]);
    let venv_s = venv.to_string_lossy().into_owned();
    let wh_s = whdir.to_string_lossy().into_owned();
    let r = run(&strv(&["python3.12", "-m", "venv", &venv_s]), &env, None, None)?;
    gate!("venv_create", obj(vec![("result", pass(r.rc == 0)), ("rc", J::Int(r.rc)), ("stderr", J::s(&tail(&r.stderr, 3000)))]));
    if r.rc != 0 {
        return Ok(res);
    }
    let py = format!("{venv_s}/bin/python");
    let rb = run(&strv(&[&py, "-m", "pip", "list", "--format", "json"]), &env, None, None)?;
    let baseline: Vec<String> = match pip_names(&rb.stdout) {
        Some(v) => {
            let mut b: Vec<String> = v.iter().map(|(n, _)| canon(n)).collect();
            b.sort();
            b
        }
        None => vec![],
    };
    let t = Instant::now();
    let mut cmd = strv(&[&py, "-m", "pip", "install", "--no-index"]);
    if parity {
        cmd.push("--no-deps".into());
    }
    cmd.extend(strv(&["--find-links", &format!("{wh_s}/wheelhouse"), "-r", &format!("{wh_s}/requirements-closure.txt")]));
    let r = run(&cmd, &env, None, None)?;
    gate!(
        "pip_install",
        obj(vec![
            ("result", pass(r.rc == 0)),
            ("rc", J::Int(r.rc)),
            ("argv", J::strs(&cmd[1..])),
            ("seconds_monotonic", seconds(t)),
            ("stdout_tail", J::s(&tail(&r.stdout, 1500))),
            ("stderr_tail", J::s(&tail(&r.stderr, 3000)))
        ])
    );
    if r.rc != 0 && diagnostic && !parity {
        let t = Instant::now();
        let mut cmd2: Vec<String> = cmd[..4].to_vec();
        cmd2.push("--no-deps".into());
        cmd2.extend_from_slice(&cmd[4..]);
        let r2 = run(&cmd2, &env, None, None)?;
        gate!(
            "pip_install_no_deps_diagnostic",
            obj(vec![
                ("result", pass(r2.rc == 0)),
                ("rc", J::Int(r2.rc)),
                ("argv", J::strs(&cmd2[1..])),
                ("seconds_monotonic", seconds(t)),
                ("stderr_tail", J::s(&tail(&r2.stderr, 3000))),
                ("note", J::s("진단 전용 — 판정에 산입하지 않는다"))
            ])
        );
    }
    let r = run(&strv(&[&py, "-m", "pip", "check"]), &env, None, None)?;
    gate!("pip_check", obj(vec![("result", pass(r.rc == 0)), ("rc", J::Int(r.rc)), ("output", J::s(&tail(&format!("{}{}", r.stdout, r.stderr), 6000)))]));
    let r = run(&strv(&[&py, "-m", "pip", "list", "--format", "json"]), &env, None, None)?;
    let inst: Vec<(String, String)> = pip_names(&r.stdout).map(|v| v.into_iter().map(|(n, ver)| (canon(&n), ver)).collect()).unwrap_or_default();
    let inst_get = |k: &str| inst.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let mut pins: Vec<(String, String)> = Vec::new();
    let req_text = String::from_utf8_lossy(&std::fs::read(whdir.join("requirements-closure.txt"))?).into_owned();
    for ln in splitlines(&req_text) {
        if !ln.is_empty() && !ln.starts_with('#') {
            if let Some((n, v)) = ln.split_once("==") {
                let k = canon(n);
                match pins.iter_mut().find(|(a, _)| *a == k) {
                    Some(slot) => slot.1 = v.to_string(),
                    None => pins.push((k, v.to_string())),
                }
            }
        }
    }
    let mut inst_keys: Vec<String> = Vec::new();
    for (n, _) in &inst {
        if !inst_keys.contains(n) {
            inst_keys.push(n.clone());
        }
    }
    let mut diff_missing: Vec<String> = pins.iter().filter(|(k, v)| inst_get(k).as_deref() != Some(v.as_str())).map(|(k, _)| k.clone()).collect();
    diff_missing.sort();
    let mut diff_extra: Vec<String> = inst_keys.iter().filter(|k| !pins.iter().any(|(p, _)| p == *k)).cloned().collect();
    diff_extra.sort();
    gate!(
        "installed_set",
        obj(vec![
            ("result", pass(diff_missing.is_empty())),
            ("pins", J::Int(pins.len() as i64)),
            ("installed", J::Int(inst_keys.len() as i64)),
            ("missing_or_other_version", J::strs(diff_missing.iter().take(50))),
            ("extra_in_venv", J::strs(&diff_extra)),
            ("note", J::s("extra = venv 부트스트랩(pip 등)"))
        ])
    );
    set(get_mut(get_mut(&mut res, "gates"), "pip_check"), "inherited_from_image", jb(!pins.is_empty() && diff_missing.is_empty()));
    if parity {
        let boot: BTreeSet<String> = baseline.iter().cloned().collect();
        let bad_extra: Vec<String> = diff_extra.iter().filter(|k| !boot.contains(*k)).cloned().collect();
        {
            let ig = get_mut(get_mut(&mut res, "gates"), "installed_set");
            set(ig, "result", pass(!pins.is_empty() && diff_missing.is_empty() && bad_extra.is_empty()));
            set(ig, "unexpected_extra", J::strs(&bad_extra));
            set(ig, "allowed_bootstrap_extra", J::strs(&boot));
        }
        let out_text = get_mut(get_mut(&mut res, "gates"), "pip_check").get("output").and_then(|x| x.str()).unwrap_or("").to_string();
        let lines: Vec<String> = splitlines(&out_text).iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && l != "No broken requirements found.").collect();
        let new: Vec<String> = lines.iter().filter(|l| !declared.contains(l)).cloned().collect();
        let dno: Vec<String> = declared.iter().filter(|d| !lines.contains(d)).cloned().collect();
        let pc = get_mut(get_mut(&mut res, "gates"), "pip_check");
        set(pc, "result", pass(new.is_empty()));
        set(pc, "observed_conflicts", J::strs(&lines));
        set(pc, "new_conflicts", J::strs(&new));
        set(pc, "declared_not_observed", J::strs(&dno));
        set(pc, "rule", J::s("관측 충돌 ⊆ image_inherent_conflicts(선언 밖 충돌 = FAIL)"));
    }
    let r = run(&strv(&[&py, "-c", "import sysconfig;print(sysconfig.get_paths()['purelib'])"]), &env, None, None)?;
    let vsite = r.stdout.trim().to_string();
    let rt = closure.get("runtime_lib").ok_or_else(|| key_error("runtime_lib"))?;
    let template = rt.get("ld_library_path_template").ok_or_else(|| key_error("ld_library_path_template"))?;
    let render = |with_compat: bool| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for ent in template.arr() {
            let Some(p) = ent.get("path").filter(|x| x.truthy()).and_then(|x| x.str()) else { continue };
            if ent.get("compat").map_or(false, |c| c.truthy()) && !with_compat {
                continue;
            }
            let p = p.replace("{VENV_SITE}", &vsite).replace("{OUT}", &wh_s);
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    };
    let mut sofiles: Vec<String> = Vec::new();
    if !vsite.is_empty() {
        rglob_so(&Path::new(&vsite).join("vllm"), &mut sofiles);
        sofiles.sort();
    }
    let cuda_tree = rt.get("cuda_tree").ok_or_else(|| key_error("cuda_tree"))?;
    let mut cands: Vec<J> = Vec::new();
    for (label, wc) in [("compat-first(이미지 env 순서)", true), ("host-driver(compat 제외)", false)] {
        if wc && !cuda_tree.get("compat_dir").map_or(false, |x| x.truthy()) {
            continue;
        }
        let ld = render(wc).join(":");
        let mut extra: Env = vec![("LD_LIBRARY_PATH".to_string(), ld.clone())];
        if let Some(J::Obj(items)) = rt.get("env_template") {
            for (k, v) in items {
                let v = v.str().unwrap_or("").replace("{OUT}", &wh_s);
                set_env(&mut extra, k, v);
            }
        }
        let mut pp: Vec<String> = rt.get("path_prepend_template").map(|a| a.arr().iter().filter_map(|x| x.str()).map(|p| p.replace("{OUT}", &wh_s)).collect()).unwrap_or_default();
        pp.push(format!("{venv_s}/bin"));
        pp.push(env_get(&env, "PATH").to_string());
        set_env(&mut extra, "PATH", pp.join(":"));
        let e2 = clean_env(&extra);
        let mut ldd: Vec<(String, i64, Vec<String>)> = Vec::new();
        for so in &sofiles {
            let lr = run(&strv(&["ldd", so]), &e2, None, None)?;
            let nf: BTreeSet<String> = splitlines(&lr.stdout).iter().filter(|l| l.contains("not found")).map(|l| l.split("=>").next().unwrap_or("").trim().to_string()).collect();
            ldd.push((relpath(so, &vsite), lr.rc, nf.into_iter().collect()));
        }
        let ldd_ok = !sofiles.is_empty() && ldd.iter().all(|(_, rc, nf)| *rc == 0 && nf.is_empty());
        let cwd = TmpDir::new()?;
        let ir = run(&strv(&[&py, "-c", IMPORT_PROBE]), &e2, Some(&cwd.0), Some(Duration::from_secs(900)))?;
        drop(cwd);
        let probe: Option<J> = {
            let s = ir.stdout.trim();
            if s.is_empty() {
                None
            } else {
                splitlines(s).last().and_then(|l| loads(l.as_bytes()).ok())
            }
        };
        let probe = probe.filter(|p| matches!(p, J::Obj(_)));
        let in_venv = probe.as_ref().map_or(false, |p| {
            let starts = |k: &str| match p.get(k) {
                None => venv_s.is_empty(),
                Some(J::Str(s)) => s.starts_with(&venv_s),
                Some(J::Null) => "None".starts_with(&venv_s),
                Some(_) => false,
            };
            probe_truthy(p) && starts("vllm_file") && starts("torch_file")
        });
        let cuda_av = probe.as_ref().and_then(|p| p.get("cuda_available")).map_or(false, |x| x.truthy());
        let imp_ok = ir.rc == 0 && probe.as_ref().map_or(false, |p| probe_truthy(p)) && cuda_av && in_venv;
        let not_found: Vec<(String, J)> = ldd.iter().filter(|(_, rc, nf)| !nf.is_empty() || *rc != 0).map(|(f, _, nf)| (f.clone(), J::strs(nf))).collect();
        cands.push(obj(vec![
            ("label", J::s(label)),
            ("with_compat", jb(wc)),
            ("ld_library_path", J::s(&ld)),
            ("env", J::Obj(extra.iter().map(|(k, v)| (k.clone(), J::s(v))).collect())),
            (
                "ldd",
                obj(vec![("result", pass(ldd_ok)), ("files", J::Int(ldd.len() as i64)), ("not_found", J::Obj(not_found))]),
            ),
            (
                "import_cuda",
                obj(vec![
                    ("result", pass(imp_ok)),
                    ("rc", J::Int(ir.rc)),
                    ("probe", probe.clone().unwrap_or(J::Null)),
                    ("imported_from_venv", jb(in_venv)),
                    ("stderr_tail", J::s(&tail(&ir.stderr, 2500)))
                ]),
            ),
        ]));
    }
    let both = |c: &J| c.get("ldd").and_then(|x| x.get("result")).and_then(|x| x.str()) == Some("PASS") && c.get("import_cuda").and_then(|x| x.get("result")).and_then(|x| x.str()) == Some("PASS");
    let sel: Option<J> = cands.iter().find(|c| both(c)).cloned();
    let pick = sel.clone().or_else(|| cands.first().cloned());
    let ldd_result = pick.as_ref().and_then(|c| c.get("ldd")).and_then(|x| x.get("result")).cloned().unwrap_or_else(|| J::s("FAIL"));
    let pick_label = pick.as_ref().and_then(|c| c.get("label")).cloned().unwrap_or(J::Null);
    gate!("runtime_candidates", J::Arr(cands.clone()));
    gate!("ldd", obj(vec![("result", ldd_result), ("for", pick_label)]));
    gate!("import_cuda", obj(vec![("result", pass(sel.is_some())), ("for", sel.as_ref().and_then(|c| c.get("label")).cloned().unwrap_or(J::Null))]));
    set(&mut res, "selected", sel.as_ref().and_then(|c| c.get("label")).cloned().unwrap_or(J::Null));
    set(&mut res, "ld_library_path", sel.as_ref().and_then(|c| c.get("ld_library_path")).cloned().unwrap_or(J::Null));
    set(&mut res, "env", sel.as_ref().and_then(|c| c.get("env")).cloned().unwrap_or(J::Null));
    let base_gates: Vec<&str> = if parity { vec!["venv_create", "pip_install", "installed_set", "pip_check"] } else { vec!["venv_create", "pip_install", "pip_check"] };
    let base_ok = base_gates.iter().all(|g| result_of(res.get("gates").unwrap(), g).as_deref() == Some("PASS"));
    let mut vg: Vec<&str> = base_gates.clone();
    vg.extend(["ldd", "import_cuda"]);
    set(&mut res, "verdict_gates", J::strs(&vg));
    set(&mut res, "verdict", pass(base_ok && sel.is_some() && status_pass));
    set(&mut res, "sizes", obj(vec![("venv_bytes", J::Int(du(venv))), ("wheelhouse_dir_bytes_total", J::Int(du(whdir)))]));
    Ok(res)
}

fn probe_truthy(p: &J) -> bool {
    p.truthy()
}

pub struct VerifyArgs {
    pub wheelhouse_dir: Option<String>,
    pub venv: Option<String>,
    pub generated_utc: Option<String>,
    pub accept_file: Option<String>,
    pub diagnostic: bool,
    pub out_json: Option<String>,
}

pub fn run_verify(a: &VerifyArgs) -> Result<i32, Err> {
    let wd = PathBuf::from(a.wheelhouse_dir.as_ref().unwrap());
    let ve = PathBuf::from(a.venv.as_ref().unwrap());
    let utc = a.generated_utc.as_ref().unwrap();
    if !wd.is_absolute() || !ve.is_absolute() || !is_utc(utc) {
        eprintln!("경로는 절대경로, --generated-utc 는 YYYY-MM-DDTHH:MM:SSZ");
        return Ok(2);
    }
    let res = verify(&wd, &ve, utc, a.diagnostic, a.accept_file.as_ref().map(Path::new))?;
    let txt = dumps(&res);
    let target = a.out_json.as_ref().map(PathBuf::from).unwrap_or_else(|| wd.join("verify.json"));
    std::fs::write(target, format!("{txt}\n"))?;
    println!("{txt}");
    Ok(if res.get("verdict").and_then(|v| v.str()) == Some("PASS") { 0 } else { 3 })
}
