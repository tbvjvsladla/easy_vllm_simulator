//! `native_wheelhouse.py --self-test` in Rust: a fake image root (dist-info trees, an editable install, a Debian dpkg place, a CUDA tree) is
//! repacked by the real `build` and checked, including the negative controls (RECORD mismatch, human accept, accept file, wrong image digest,
//! parity verify with a declared and an undeclared conflict) and an offline `pip install` into a real venv. No Python of its own is run; the venv
//! step runs `python3.12 -m venv` and the venv's pip, the same commands the original runs.

use crate::wheelhouse_build::*;
use crate::wheelhouse_source::*;
use crate::wheelhouse_util::*;
use crate::wheelhouse_verify::verify;
use crate::zip_archive::ZipArchive;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

type R = Result<(), String>;
type Files = Vec<(String, Vec<u8>)>;

struct Dist<'a> {
    name: &'a str,
    ver: &'a str,
    files: Vec<(&'a str, &'a [u8])>,
    extra_rows: Vec<Vec<String>>,
    ep: &'a str,
    direct: Option<&'a str>,
    tamper: Vec<(&'a str, &'a [u8])>,
    wheel_tag: Option<&'a str>,
    requires: &'a str,
}

impl<'a> Dist<'a> {
    fn new(name: &'a str, ver: &'a str, files: Vec<(&'a str, &'a [u8])>) -> Self {
        Dist { name, ver, files, extra_rows: vec![], ep: "", direct: None, tamper: vec![], wheel_tag: Some("py3-none-any"), requires: "" }
    }
}

fn io<T>(r: std::io::Result<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

fn rec_line(rel: &str, data: Option<&[u8]>) -> Vec<String> {
    match data {
        None => vec![rel.into(), String::new(), String::new()],
        Some(d) => {
            let dg = ring::digest::digest(&ring::digest::SHA256, d);
            vec![rel.into(), format!("sha256={}", b64url(dg.as_ref())), d.len().to_string()]
        }
    }
}

fn fake_dist(site: &Path, d: Dist) -> R {
    let di = format!("{}-{}.dist-info", d.name, d.ver);
    io(std::fs::create_dir_all(site.join(&di)))?;
    let mut meta = format!("Metadata-Version: 2.1\nName: {}\nVersion: {}\n", d.name, d.ver);
    if !d.requires.is_empty() {
        meta.push_str(&format!("Requires-Dist: {}\n", d.requires));
    }
    let mut files: Files = d.files.iter().map(|(k, v)| (k.to_string(), v.to_vec())).collect();
    files.push((format!("{di}/METADATA"), meta.into_bytes()));
    files.push((format!("{di}/INSTALLER"), b"pip\n".to_vec()));
    if let Some(t) = d.wheel_tag {
        files.push((format!("{di}/WHEEL"), format!("Wheel-Version: 1.0\nGenerator: t\nRoot-Is-Purelib: true\nTag: {t}\n").into_bytes()));
    }
    if !d.ep.is_empty() {
        files.push((format!("{di}/entry_points.txt"), d.ep.as_bytes().to_vec()));
    }
    if let Some(dj) = d.direct {
        files.push((format!("{di}/direct_url.json"), dj.as_bytes().to_vec()));
    }
    let mut rows: Vec<Vec<String>> = vec![];
    for (rel, data) in &files {
        let p = PathBuf::from(normpath(&format!("{}/{}", site.display(), rel)));
        io(std::fs::create_dir_all(p.parent().unwrap()))?;
        let bytes = d.tamper.iter().find(|(k, _)| k == rel).map(|(_, v)| v.to_vec()).unwrap_or_else(|| data.clone());
        io(std::fs::write(&p, bytes))?;
        if rel.starts_with("../../../bin/") {
            io(std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)))?;
        }
        rows.push(rec_line(rel, Some(data)));
    }
    rows.extend(d.extra_rows);
    rows.push(rec_line(&format!("{di}/RECORD"), None));
    io(std::fs::write(site.join(&di).join("RECORD"), csv_write(&rows)))
}

fn make_fake_root(root: &Path, with_bad: bool) -> Result<J, String> {
    let site = root.join(SITE_DEFAULT.trim_start_matches('/'));
    io(std::fs::create_dir_all(&site))?;
    let mut alpha = Dist::new(
        "alpha",
        "1.0",
        vec![
            ("alpha/__init__.py", b"VALUE = 42\n"),
            ("../../../bin/alpha", b"#!/usr/bin/python3\nimport alpha\n"),
            ("../../../bin/alpha-tool", b"#!/usr/bin/python3\nprint('tool')\n"),
            ("../../../share/alpha/data.txt", b"data\n"),
        ],
    );
    alpha.extra_rows = vec![vec!["alpha/__pycache__/__init__.cpython-312.pyc".into(), String::new(), String::new()]];
    alpha.ep = "[console_scripts]\nalpha = alpha:main\n";
    fake_dist(&site, alpha)?;
    if with_bad {
        let mut beta = Dist::new("beta", "2.0", vec![("beta/__init__.py", b"ORIG = 1\n")]);
        beta.tamper = vec![("beta/__init__.py", b"PATCHED = 1\n")];
        beta.requires = "zeta>=9";
        fake_dist(&site, beta)?;
    }
    let mut delta = Dist::new("delta", "1.0", vec![("ns/__init__.py", b""), ("ns/d.py", b"D = 1\n")]);
    delta.tamper = vec![("ns/__init__.py", b"# epsilon\n")];
    fake_dist(&site, delta)?;
    fake_dist(&site, Dist::new("epsilon", "1.0", vec![("ns/__init__.py", b"# epsilon\n"), ("ns/e.py", b"E = 1\n")]))?;
    let src_pkg = root.join("workspace/gamma-src/gamma");
    io(std::fs::create_dir_all(src_pkg.join("__pycache__")))?;
    io(std::fs::write(src_pkg.join("__init__.py"), b"from gamma.sub import X\n"))?;
    io(std::fs::write(src_pkg.join("sub.py"), b"X = 'editable-ok'\n"))?;
    io(std::fs::write(src_pkg.join("_C.abi3.so"), b"not-really-elf"))?;
    io(std::fs::write(src_pkg.join("__pycache__/sub.cpython-312.pyc"), b"junk"))?;
    let finder = "MAPPING: dict[str, str] = {'gamma': '/workspace/gamma-src/gamma'}\nNAMESPACES = {}\n";
    let mut gamma = Dist::new(
        "gamma",
        "3.0.dev0+g1",
        vec![
            ("__editable__.gamma-3.0.dev0+g1.pth", b"import __editable___gamma_3_0_dev0_g1_finder\n"),
            ("__editable___gamma_3_0_dev0_g1_finder.py", finder.as_bytes()),
            ("../../../bin/gamma", b"#!/usr/bin/python3\nimport gamma\n"),
        ],
    );
    gamma.ep = "[console_scripts]\ngamma = gamma:main\n";
    gamma.direct = Some("{\"dir_info\": {\"editable\": true}, \"url\": \"file:///workspace/gamma-src\"}");
    gamma.wheel_tag = Some("cp312-cp312-linux_aarch64");
    fake_dist(&site, gamma)?;
    // Debian place: zeta without RECORD (dpkg manifest) and a shadowed alpha
    let deb = root.join("usr/lib/python3/dist-packages");
    io(std::fs::create_dir_all(deb.join("zeta-0.5.dist-info")))?;
    io(std::fs::create_dir_all(deb.join("zeta")))?;
    let zf: Vec<(&str, &[u8])> = vec![
        ("zeta/__init__.py", b"Z = 5\n"),
        ("zeta-0.5.dist-info/METADATA", b"Metadata-Version: 2.1\nName: zeta\nVersion: 0.5\n"),
        ("zeta-0.5.dist-info/WHEEL", b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n"),
    ];
    for (rel, data) in &zf {
        io(std::fs::write(deb.join(rel), data))?;
    }
    io(std::fs::create_dir_all(deb.join("alpha-0.1.dist-info")))?;
    io(std::fs::write(deb.join("alpha-0.1.dist-info/METADATA"), b"Metadata-Version: 2.1\nName: alpha\nVersion: 0.1\n"))?;
    let info = root.join("var/lib/dpkg/info");
    io(std::fs::create_dir_all(&info))?;
    let dsite = "/usr/lib/python3/dist-packages";
    let mut list = vec![dsite.to_string(), format!("{dsite}/zeta-0.5.dist-info")];
    list.extend(zf.iter().map(|(r, _)| format!("{dsite}/{r}")));
    io(std::fs::write(info.join("python3-zeta.list"), list.join("\n") + "\n"))?;
    let md5s: String = zf.iter().map(|(r, v)| format!("{}  {}/{}\n", hex(&md5(v)), dsite.trim_start_matches('/'), r)).collect();
    io(std::fs::write(info.join("python3-zeta.md5sums"), md5s))?;
    let cuda = root.join("usr/local/cuda-13.3");
    io(std::fs::create_dir_all(cuda.join("compat/lib.real")))?;
    io(std::fs::write(cuda.join("compat/lib.real/libcuda.so.1"), b"fake"))?;
    io(std::fs::create_dir_all(cuda.join("bin")))?;
    io(std::os::unix::fs::symlink("/usr/local/cuda-13.3", root.join("usr/local/cuda")))?;
    io(std::fs::create_dir_all(root.join("etc")))?;
    io(std::fs::write(root.join("etc/ld.so.conf"), "/usr/local/cuda/lib64\n"))?;
    Ok(obj(vec![
        ("tag", J::s("fake:selftest")),
        ("id", J::s(&format!("sha256:{}", "0".repeat(64)))),
        ("repo_digests", J::Arr(vec![])),
        ("architecture", J::s("arm64")),
        ("os", J::s("linux")),
        (
            "env",
            J::strs([
                "LD_LIBRARY_PATH=/usr/local/lib/python3.12/dist-packages/torch/lib:/usr/local/cuda/compat/lib:/usr/local/nvidia/lib",
                "TRITON_PTXAS_PATH=/usr/local/cuda/bin/ptxas",
                "CUDA_HOME=/usr/local/cuda",
            ]),
        ),
    ]))
}

struct Ck {
    checks: Vec<(String, bool, String)>,
}

impl Ck {
    fn ck(&mut self, name: &str, cond: bool, detail: &str) {
        self.checks.push((name.to_string(), cond, detail.to_string()));
    }
}

fn path_of<'a>(j: &'a J, keys: &[&str]) -> Option<&'a J> {
    let mut cur = j;
    for k in keys {
        cur = cur.get(k)?;
    }
    Some(cur)
}

fn sget<'a>(j: &'a J, keys: &[&str]) -> &'a str {
    path_of(j, keys).and_then(|x| x.str()).unwrap_or("")
}

fn dist<'a>(c: &'a J, name: &str) -> Option<&'a J> {
    c.get("distributions")?.arr().iter().find(|d| d.get("name").and_then(|x| x.str()) == Some(name))
}

fn wheel_names(p: &Path) -> Result<Vec<String>, String> {
    let data = io(std::fs::read(p))?;
    let z = ZipArchive::open(&data).map_err(|_| format!("zip open failed: {}", p.display()))?;
    Ok(z.namelist().iter().map(|n| n.to_utf8().unwrap_or_default()).collect())
}

fn wheel_read(p: &Path, name: &str) -> Result<Vec<u8>, String> {
    let data = io(std::fs::read(p))?;
    let z = ZipArchive::open(&data).map_err(|_| "zip open failed".to_string())?;
    let idx = z.namelist().iter().position(|n| n.to_utf8().as_deref() == Some(name)).ok_or_else(|| format!("no member {name}"))?;
    z.read_entry(idx).map_err(|_| "zip read failed".to_string())
}

fn tail(t: &str, n: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

fn run_build(src_root: &Path, meta: &J, out: &Path, accept: &Accept, staging: &Path, am: Option<&AcceptMeta>) -> Result<J, Err> {
    let mut src = Source::dir(src_root, Meta::from_json(meta)?);
    src.start()?;
    let sites: Vec<String> = SITES_DEFAULT.iter().map(|s| s.to_string()).collect();
    let r = build(&mut src, out, &sites, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z", accept, 4, Some(staging), am);
    let _ = src.close();
    r
}

fn body(tdp: &Path, c: &mut Ck) -> R {
    let e = |x: Err| format!("{x:?}");
    let utc = "2026-01-01T00:00:00Z";
    // (1) negative control: RECORD mismatch -> FAIL
    let r1 = tdp.join("root_bad");
    let m1 = make_fake_root(&r1, true)?;
    let c1 = run_build(&r1, &m1, &tdp.join("out_bad"), &Accept::new(), &tdp.join("st1"), None).map_err(e)?;
    c.ck("mismatch→status FAIL", sget(&c1, &["status"]) == "FAIL", sget(&c1, &["status"]));
    let fails = c1.get("failures").map(|f| f.arr().to_vec()).unwrap_or_default();
    c.ck(
        "mismatch 목록에 beta/__init__.py",
        fails.iter().any(|f| sget(f, &["dist"]) == "beta" && sget(f, &["path"]) == "beta/__init__.py" && sget(f, &["kind"]) == "record-hash-mismatch"),
        "",
    );
    c.ck("다른 분포는 불일치 아님", fails.iter().all(|f| sget(f, &["dist"]) == "beta"), &dumps(&J::Arr(fails.clone())));
    // (1b) human accept
    let acc_b = parse_accept(&["beta:beta/__init__.py=selftest".to_string()]).map_err(e)?;
    let c1b = run_build(&r1, &m1, &tdp.join("out_acc"), &acc_b, &tdp.join("st1b"), None).map_err(e)?;
    let bb = dist(&c1b, "beta");
    c.ck(
        "수용된 불일치는 ACCEPTED+provenance",
        sget(&c1b, &["status"]) == "PASS"
            && bb.map(|b| sget(b, &["record_verify", "result"]) == "ACCEPTED").unwrap_or(false)
            && bb.and_then(|b| path_of(b, &["record_verify", "accepted"])).and_then(|a| a.arr().first()).map(|a| sget(a, &["provenance"]).starts_with("human")).unwrap_or(false),
        "",
    );
    // (1c) tracked accept file
    let mut acc_n = 0;
    let mut accept_doc = |conflicts: &[&str], image_id: &str| -> Result<PathBuf, String> {
        acc_n += 1;
        let p = tdp.join(format!("accept-{}.json", acc_n - 1));
        let doc = obj(vec![
            ("schema_version", J::Int(1)),
            ("approved_by", J::s("selftest-human")),
            ("approved_utc", J::s(utc)),
            ("image", obj(vec![("tag", J::s("fake:selftest")), ("id", J::s(image_id))])),
            (
                "accepted_mismatches",
                J::Arr(vec![
                    obj(vec![("dist", J::s("beta")), ("path", J::s("beta/__init__.py")), ("reason", J::s("t"))]),
                    obj(vec![("dist", J::s("beta")), ("path", J::s("beta/never.py")), ("reason", J::s("stale"))]),
                ]),
            ),
            ("image_inherent_conflicts", J::Arr(conflicts.iter().map(|x| obj(vec![("pip_check_line", J::s(x))])).collect())),
        ]);
        io(std::fs::write(&p, dumps(&doc)))?;
        Ok(p)
    };
    let conflict = "beta 2.0 has requirement zeta>=9, but you have zeta 0.5.";
    let m1_id = sget(&m1, &["id"]).to_string();
    let af = accept_doc(&[conflict], &m1_id)?;
    let (acc, ameta) = load_accept_file(&af).map_err(e)?;
    let c1c = run_build(&r1, &m1, &tdp.join("out_accfile"), &acc, &tdp.join("st1c"), Some(&ameta)).map_err(e)?;
    let bc = dist(&c1c, "beta");
    c.ck(
        "승인 파일 적용 → PASS · provenance accepted-mismatch(human: …)",
        sget(&c1c, &["status"]) == "PASS"
            && bc.and_then(|b| path_of(b, &["record_verify", "accepted"])).and_then(|a| a.arr().first()).map(|a| sget(a, &["provenance"]) == "accepted-mismatch(human: selftest-human)").unwrap_or(false),
        "",
    );
    let stale: Vec<&str> = path_of(&c1c, &["accept", "stale"]).map(|s| s.arr().iter().filter_map(|x| x.str()).collect()).unwrap_or_default();
    c.ck(
        "일어나지 않은 승인 = stale-accept 경고",
        stale == ["beta:beta/never.py"] && c1c.get("warnings").map(|w| w.arr().iter().any(|x| sget(x, &["kind"]) == "stale-accept")).unwrap_or(false),
        "",
    );
    let wrong = AcceptMeta { image_id: format!("sha256:{}", "1".repeat(64)), image_ids: vec![format!("sha256:{}", "1".repeat(64)), format!("sha256:{}", "2".repeat(64))], ..ameta.clone() };
    let wr = run_build(&r1, &m1, &tdp.join("out_wrongimg"), &acc, &tdp.join("st1d"), Some(&wrong));
    c.ck("다른 이미지 digest 의 승인 파일 거부", matches!(wr, Err(Err::Wh(_))), "");
    let vp = verify(&tdp.join("out_accfile"), &tdp.join("venv_parity"), utc, false, Some(&af)).map_err(e)?;
    c.ck("parity: gate_definition 표기", sget(&vp, &["gate_definition"]).starts_with("image-parity("), sget(&vp, &["gate_definition"]));
    let pi_argv: Vec<&str> = path_of(&vp, &["gates", "pip_install", "argv"]).map(|a| a.arr().iter().filter_map(|x| x.str()).collect()).unwrap_or_default();
    c.ck("parity: --no-deps 설치 PASS", sget(&vp, &["gates", "pip_install", "result"]) == "PASS" && pi_argv.contains(&"--no-deps"), &tail(sget(&vp, &["gates", "pip_install", "stderr_tail"]), 400));
    c.ck("parity: 설치 집합 == 핀", sget(&vp, &["gates", "installed_set", "result"]) == "PASS", &path_of(&vp, &["gates", "installed_set"]).map(dumps).unwrap_or_default());
    let obs: Vec<&str> = path_of(&vp, &["gates", "pip_check", "observed_conflicts"]).map(|a| a.arr().iter().filter_map(|x| x.str()).collect()).unwrap_or_default();
    c.ck("parity: 선언된 충돌은 PASS", sget(&vp, &["gates", "pip_check", "result"]) == "PASS" && obs == [conflict], &path_of(&vp, &["gates", "pip_check"]).map(dumps).unwrap_or_default());
    let af2 = accept_doc(&[], &m1_id)?;
    let vn = verify(&tdp.join("out_accfile"), &tdp.join("venv_parity_neg"), utc, false, Some(&af2)).map_err(e)?;
    let newc: Vec<&str> = path_of(&vn, &["gates", "pip_check", "new_conflicts"]).map(|a| a.arr().iter().filter_map(|x| x.str()).collect()).unwrap_or_default();
    c.ck("parity: 선언 밖 충돌 → FAIL", sget(&vn, &["gates", "pip_check", "result"]) == "FAIL" && newc == [conflict] && sget(&vn, &["verdict"]) == "FAIL", "");

    // (2) clean tree
    let r2 = tdp.join("root_ok");
    let m2 = make_fake_root(&r2, false)?;
    let m2_id = sget(&m2, &["id"]).to_string();
    let o2 = tdp.join("out_ok");
    let c2 = run_build(&r2, &m2, &o2, &Accept::new(), &tdp.join("st2"), None).map_err(e)?;
    c.ck("정상 트리 PASS", sget(&c2, &["status"]) == "PASS", &c2.get("failures").map(dumps).unwrap_or_default());
    let g = |n: &str| dist(&c2, n);
    let all_d = c2.get("distributions").map(|d| d.arr().to_vec()).unwrap_or_default();
    c.ck("출처 필드 전 항목", all_d.iter().all(|d| d.get("source").map(|x| x.truthy()).unwrap_or(false) && d.get("wheel").map(|x| x.truthy()).unwrap_or(false) && !sget(d, &["record_verify", "result"]).is_empty()), "");
    c.ck("alpha = repacked(image@…)", g("alpha").map(|d| sget(d, &["source"]) == format!("repacked(image@{m2_id})")).unwrap_or(false), "");
    c.ck("gamma = editable-repacked(image@…)", g("gamma").map(|d| sget(d, &["source"]) == format!("editable-repacked(image@{m2_id})")).unwrap_or(false), "");
    c.ck("closure 최상위 필드", ["image", "generated_utc", "provenance", "runtime_lib", "excluded"].iter().all(|k| c2.get(k).is_some()), "");
    c.ck("compat 표식", sget(&c2, &["runtime_lib", "cuda_tree", "compat_dir"]) == "{OUT}/runtime-lib/cuda/compat/lib.real", "");
    c.ck("env 템플릿 재사상", sget(&c2, &["runtime_lib", "env_template", "TRITON_PTXAS_PATH"]) == "{OUT}/runtime-lib/cuda/bin/ptxas", &path_of(&c2, &["runtime_lib", "env_template"]).map(dumps).unwrap_or_default());
    let whl = |n: &str| o2.join("wheelhouse").join(g(n).map(|d| sget(d, &["wheel"])).unwrap_or(""));
    let an = wheel_names(&whl("alpha"))?;
    let regen: Vec<&str> = g("alpha").and_then(|d| path_of(d, &["scripts", "regenerated_entry_points"])).map(|a| a.arr().iter().filter_map(|x| x.str()).collect()).unwrap_or_default();
    c.ck("alpha: 엔트리포인트 스크립트는 싣지 않음(pip 재생성)", !an.iter().any(|n| n.ends_with("scripts/alpha")) && regen == ["alpha"], "");
    c.ck("alpha: 비-엔트리 스크립트 #!python", wheel_read(&whl("alpha"), "alpha-1.0.data/scripts/alpha-tool")?.starts_with(b"#!python\n"), "");
    c.ck("alpha: data 파일", an.iter().any(|n| n == "alpha-1.0.data/data/share/alpha/data.txt"), "");
    c.ck("alpha: pyc·INSTALLER 없음", !an.iter().any(|n| n.ends_with(".pyc") || n.ends_with("/INSTALLER")), "");
    let gn = wheel_names(&whl("gamma"))?;
    c.ck("gamma: 소스 트리 + .so", ["gamma/__init__.py", "gamma/sub.py", "gamma/_C.abi3.so"].iter().all(|x| gn.iter().any(|n| n == x)), &format!("{gn:?}"));
    c.ck("gamma: editable shim·direct_url 없음", !gn.iter().any(|n| n.contains("__editable__") || n.ends_with("direct_url.json")), "");
    let gw = g("gamma").map(|d| sget(d, &["wheel"])).unwrap_or("");
    c.ck("gamma: 플랫폼 태그", gw.ends_with("-cp312-cp312-linux_aarch64.whl"), gw);
    let dn = wheel_names(&whl("delta"))?;
    c.ck(
        "공유 경로: delta 는 ns/__init__.py 생략(소유=epsilon)",
        !dn.iter().any(|n| n == "ns/__init__.py")
            && g("delta").and_then(|d| path_of(d, &["record_verify", "shared_path_omitted"])).and_then(|a| a.arr().first()).map(|a| sget(a, &["owner"]).ends_with("/epsilon-1.0.dist-info")).unwrap_or(false),
        "",
    );
    let z = g("zeta");
    c.ck(
        "dpkg 매니페스트로 RECORD 대체(zeta)",
        z.map(|d| sget(d, &["record_source"]) == "dpkg:python3-zeta.list+md5sums" && sget(d, &["record_verify", "result"]) == "PASS" && matches!(path_of(d, &["record_verify", "verified"]), Some(J::Int(3)))).unwrap_or(false),
        &z.and_then(|d| d.get("record_verify")).map(dumps).unwrap_or_default(),
    );
    c.ck("뒤 자리의 같은 이름은 shadowed 로 제외", c2.get("excluded").map(|x| x.arr().iter().any(|e| sget(e, &["name"]) == "alpha" && sget(e, &["reason"]).starts_with("shadowed"))).unwrap_or(false), "");
    let req = io(std::fs::read_to_string(o2.join("requirements-closure.txt")))?;
    c.ck("requirements 정확 핀", req.contains("gamma==3.0.dev0+g1") && req.contains("alpha==1.0"), "");

    // (3) real offline pip install into a temp venv
    let venv = tdp.join("venv");
    let rv = Command::new("python3.12").args(["-m", "venv", &venv.to_string_lossy()]).env_remove("PYTHONPATH").output();
    match rv {
        Ok(o) if o.status.success() => {
            let ri = Command::new(venv.join("bin/python"))
                .args(["-m", "pip", "install", "--no-index", "--find-links", &o2.join("wheelhouse").to_string_lossy(), "-r", &o2.join("requirements-closure.txt").to_string_lossy()])
                .env_remove("PYTHONPATH")
                .output()
                .map_err(|x| x.to_string())?;
            let txt = format!("{}{}", String::from_utf8_lossy(&ri.stdout), String::from_utf8_lossy(&ri.stderr));
            c.ck("pip install --no-index rc 0", ri.status.success(), &tail(&txt, 800));
            c.ck("venv/bin/gamma 재생성", venv.join("bin/gamma").exists(), "");
            let rp = Command::new(venv.join("bin/python"))
                .args(["-c", "import gamma,alpha,ns.d,ns.e,zeta;print(gamma.X)"])
                .env_remove("PYTHONPATH")
                .current_dir(tdp)
                .output()
                .map_err(|x| x.to_string())?;
            c.ck("설치본 import", String::from_utf8_lossy(&rp.stdout).trim() == "editable-ok", &tail(&String::from_utf8_lossy(&rp.stderr), 400));
        }
        Ok(o) => c.ck("venv 생성", false, &tail(&String::from_utf8_lossy(&o.stderr), 400)),
        Err(x) => c.ck("venv 생성", false, &x.to_string()),
    }
    Ok(())
}

pub fn self_test() -> i32 {
    let tdp = std::env::temp_dir().join(format!("nwh-selftest-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let mut c = Ck { checks: vec![] };
    let res = std::fs::create_dir_all(&tdp).map_err(|e| e.to_string()).and_then(|_| body(&tdp, &mut c));
    let _ = std::fs::remove_dir_all(&tdp);
    if let Err(m) = res {
        c.ck(&format!("self-test aborted: {m}"), false, &m);
    }
    let ok = c.checks.iter().all(|x| x.1);
    let items: Vec<J> = c
        .checks
        .iter()
        .map(|(n, o, d)| {
            let mut v = vec![("name", J::s(n)), ("ok", J::Bool(*o))];
            if !d.is_empty() && !*o {
                v.push(("detail", J::s(d)));
            }
            obj(v)
        })
        .collect();
    println!("{}", dumps(&obj(vec![("self_test", J::s(if ok { "PASS" } else { "FAIL" })), ("checks", J::Arr(items))])));
    if ok {
        0
    } else {
        3
    }
}
