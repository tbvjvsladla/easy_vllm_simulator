//! Command line of `native_wheelhouse.py` (the `build` and `verify` parts). `--self-test` stays in the Python original and ends here with
//! status 2. `build-dir` is the same build over a local directory standing in for an image root (`DirSource` of the original's self-test);
//! it is the entry the parity checks use and takes the image facts from a JSON file.

use crate::wheelhouse_build::*;
use crate::wheelhouse_source::*;
use crate::wheelhouse_util::*;
use crate::wheelhouse_verify::{run_verify, VerifyArgs};
use std::path::{Path, PathBuf};

const PROG: &str = "native_wheelhouse.py";

fn err(text: &str) {
    eprintln!("{text}");
}

fn usage_error(msg: &str) -> i32 {
    err(&format!("usage: {PROG} [-h] [--self-test] {{build,verify}} ..."));
    err(&format!("{PROG}: error: {msg}"));
    2
}

struct BuildArgs {
    image: Option<String>,
    root: Option<String>,
    meta: Option<String>,
    out: Option<String>,
    generated_utc: Option<String>,
    sites: Vec<String>,
    cuda_link: String,
    accept_mismatch: Vec<String>,
    accept_file: Option<String>,
    jobs: usize,
    keep_staging: bool,
}

fn parse_build(args: &[String], dir_mode: bool) -> Result<BuildArgs, i32> {
    let mut a = BuildArgs {
        image: None,
        root: None,
        meta: None,
        out: None,
        generated_utc: None,
        sites: vec![],
        cuda_link: CUDA_LINK_DEFAULT.to_string(),
        accept_mismatch: vec![],
        accept_file: None,
        jobs: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16),
        keep_staging: false,
    };
    let mut i = 0;
    while i < args.len() {
        let (key, inline) = match args[i].split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (args[i].clone(), None),
        };
        let val = |i: &mut usize| -> Result<String, i32> {
            if let Some(v) = &inline {
                return Ok(v.clone());
            }
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| usage_error(&format!("argument {key}: expected one argument")))
        };
        match key.as_str() {
            "--image" if !dir_mode => a.image = Some(val(&mut i)?),
            "--root" if dir_mode => a.root = Some(val(&mut i)?),
            "--meta-json" if dir_mode => a.meta = Some(val(&mut i)?),
            "--out" => a.out = Some(val(&mut i)?),
            "--generated-utc" => a.generated_utc = Some(val(&mut i)?),
            "--site-packages" => a.sites.push(val(&mut i)?),
            "--cuda-link" => a.cuda_link = val(&mut i)?,
            "--accept-mismatch" => a.accept_mismatch.push(val(&mut i)?),
            "--accept-file" => a.accept_file = Some(val(&mut i)?),
            "--jobs" => {
                let v = val(&mut i)?;
                a.jobs = v.parse().map_err(|_| usage_error(&format!("argument --jobs: invalid int value: {v:?}")))?;
            }
            "--keep-staging" => a.keep_staging = true,
            other => return Err(usage_error(&format!("unrecognized arguments: {other}"))),
        }
        i += 1;
    }
    let need = if dir_mode { [("--root", a.root.is_some()), ("--meta-json", a.meta.is_some())] } else { [("--image", a.image.is_some()), ("--image", true)] };
    let mut missing: Vec<&str> = need.iter().filter(|(_, ok)| !ok).map(|(k, _)| *k).collect();
    if a.out.is_none() {
        missing.push("--out");
    }
    if a.generated_utc.is_none() {
        missing.push("--generated-utc");
    }
    if !missing.is_empty() {
        return Err(usage_error(&format!("the following arguments are required: {}", missing.join(", "))));
    }
    Ok(a)
}

fn parse_verify(args: &[String]) -> Result<VerifyArgs, i32> {
    let mut a = VerifyArgs { wheelhouse_dir: None, venv: None, generated_utc: None, accept_file: None, diagnostic: false, out_json: None };
    let mut i = 0;
    while i < args.len() {
        let (key, inline) = match args[i].split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_string(), Some(v.to_string())),
            _ => (args[i].clone(), None),
        };
        let val = |i: &mut usize| -> Result<String, i32> {
            if let Some(v) = &inline {
                return Ok(v.clone());
            }
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| usage_error(&format!("argument {key}: expected one argument")))
        };
        match key.as_str() {
            "--wheelhouse-dir" => a.wheelhouse_dir = Some(val(&mut i)?),
            "--venv" => a.venv = Some(val(&mut i)?),
            "--generated-utc" => a.generated_utc = Some(val(&mut i)?),
            "--accept-file" => a.accept_file = Some(val(&mut i)?),
            "--out-json" => a.out_json = Some(val(&mut i)?),
            "--diagnostic" => a.diagnostic = true,
            other => return Err(usage_error(&format!("unrecognized arguments: {other}"))),
        }
        i += 1;
    }
    let mut missing: Vec<&str> = Vec::new();
    for (k, ok) in [("--wheelhouse-dir", a.wheelhouse_dir.is_some()), ("--venv", a.venv.is_some()), ("--generated-utc", a.generated_utc.is_some())] {
        if !ok {
            missing.push(k);
        }
    }
    if !missing.is_empty() {
        return Err(usage_error(&format!("the following arguments are required: {}", missing.join(", "))));
    }
    Ok(a)
}

fn finish(c: &J) -> i32 {
    let mut summary: Vec<(&str, J)> = Vec::new();
    for k in ["status", "counts", "sizes", "elapsed_s_monotonic"] {
        summary.push((k, c.get(k).cloned().unwrap_or(J::Null)));
    }
    summary.push(("failures", c.get("failures").cloned().unwrap_or(J::Null)));
    summary.push(("unresolved", c.get("runtime_lib").and_then(|r| r.get("unresolved")).cloned().unwrap_or(J::Null)));
    println!("{}", dumps(&obj(summary)));
    if c.get("status").and_then(|s| s.str()) == Some("PASS") {
        0
    } else {
        3
    }
}

fn run_build(a: &BuildArgs, dir_mode: bool) -> Result<i32, Err> {
    let out = PathBuf::from(a.out.as_ref().unwrap());
    let utc = a.generated_utc.as_ref().unwrap();
    if !out.is_absolute() || !is_utc(utc) {
        err("--out 은 절대경로, --generated-utc 는 YYYY-MM-DDTHH:MM:SSZ");
        return Ok(2);
    }
    if out.exists() && std::fs::read_dir(&out).map(|mut r| r.next().is_some()).unwrap_or(false) {
        err(&format!("--out 이 비어 있지 않다(덮어쓰지 않는다): {}", out.display()));
        return Ok(2);
    }
    std::fs::create_dir_all(&out)?;
    let staging = out.join(".staging");
    let sites: Vec<String> = if a.sites.is_empty() { SITES_DEFAULT.iter().map(|s| s.to_string()).collect() } else { a.sites.clone() };
    let mut src = if dir_mode {
        let meta_text = std::fs::read(a.meta.as_ref().unwrap())?;
        let mj = loads(&meta_text).map_err(|_| Err::Crash("JSONDecodeError: --meta-json".into()))?;
        let root = std::fs::canonicalize(a.root.as_ref().unwrap())?;
        Source::dir(&root, Meta::from_json(&mj)?)
    } else {
        Source::docker(a.image.as_ref().unwrap(), &staging)?
    };
    let result = (|| -> Result<J, Err> {
        src.start()?;
        let mut acc = parse_accept(&a.accept_mismatch)?;
        let mut ameta = None;
        if let Some(f) = &a.accept_file {
            let (fa, m) = load_accept_file(Path::new(f))?;
            for (k, v) in acc.iter() {
                let _ = (k, v);
            }
            let mut merged = fa;
            merged.extend(acc);
            acc = merged;
            ameta = Some(m);
        }
        build(&mut src, &out, &sites, &a.cuda_link, utc, &acc, a.jobs, Some(&staging), ameta.as_ref())
    })();
    let closed = src.close();
    if !a.keep_staging {
        let _ = std::fs::remove_dir_all(&staging);
    }
    let c = result?;
    closed?;
    Ok(finish(&c))
}

pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--self-test") {
        err("native_wheelhouse: --self-test 는 Rust 로 이식되지 않았다(원본 native_wheelhouse.py 를 쓴다)");
        return 2;
    }
    let Some(cmd) = args.first() else {
        err(&format!("usage: {PROG} [-h] [--self-test] {{build,verify}} ..."));
        return 2;
    };
    let result = match cmd.as_str() {
        "build" | "build-dir" => {
            let dir_mode = cmd == "build-dir";
            match parse_build(&args[1..], dir_mode) {
                Ok(a) => run_build(&a, dir_mode),
                Err(code) => return code,
            }
        }
        "verify" => match parse_verify(&args[1..]) {
            Ok(a) => run_verify(&a),
            Err(code) => return code,
        },
        "-h" | "--help" => {
            println!("usage: {PROG} [-h] [--self-test] {{build,verify}} ...");
            return 0;
        }
        other => return usage_error(&format!("argument cmd: invalid choice: {other:?} (choose from 'build', 'verify')")),
    };
    match result {
        Ok(code) => code,
        Err(Err::Wh(t)) => {
            err(&format!("native_wheelhouse: {t}"));
            2
        }
        Err(Err::Crash(t)) => {
            err(&format!("Traceback (most recent call last):\n{t}"));
            1
        }
    }
}
