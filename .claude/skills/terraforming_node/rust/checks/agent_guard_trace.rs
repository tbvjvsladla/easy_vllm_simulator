//! The agent guard's run loop driven with the synthetic scenarios the ORIGINAL `agent_guard._main` was traced with
//! (`checks/data/selftest/agent_guard.trace.json`: MemAvailable sequence, a monotonic clock that only `sleep` advances, a
//! fixed wall clock, a fake `subprocess.run` that records argv). Every level transition, debounce, action, event record,
//! stop-file, recorded `sudo`/`docker` argv (with timeout) and exit code must be the same. No process is ever started
//! and nothing is killed; each scenario runs in its own temporary directory. No Python is executed here.

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use terraforming_runtime::agent_guard::{self, Hooks};
use terraforming_runtime::py;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..").canonicalize().unwrap()
}

#[derive(Clone)]
struct Buf(Rc<RefCell<Vec<u8>>>);
impl std::io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn reduce(s: &str) -> String {
    if !s.contains("Traceback (most recent call last):") {
        return s.to_string();
    }
    format!("{}\n", s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(""))
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, String>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.insert(
                p.strip_prefix(base).unwrap().display().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            );
        }
    }
}

#[test]
fn agent_guard_trace_matches_the_original() {
    let golden = repo().join(".claude/skills/terraforming_node/rust/checks/data/selftest/agent_guard.trace.json");
    let PyValue::List(cases) = pyjson::loads(&std::fs::read_to_string(golden).unwrap()).unwrap() else {
        panic!("trace golden")
    };
    let scratch = std::env::temp_dir().join(format!("tf-agent-guard-trace-{}", std::process::id()));
    let mut bad = vec![];
    for c in &cases {
        let name = py::get_str(c, "name").unwrap();
        let dir = scratch.join(&name);
        std::fs::create_dir_all(&dir).unwrap();
        let mut setup = vec![];
        if let Some(PyValue::Dict(d)) = py::get(c, "setup") {
            for (k, v) in d.iter() {
                let rel = py::text(k);
                let p = dir.join(&rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, py::display(v)).unwrap();
                setup.push(rel);
            }
        }
        std::env::set_current_dir(&dir).unwrap();
        let argv: Vec<String> = match py::get(c, "argv") {
            Some(PyValue::List(l)) => l.iter().map(py::display).collect(),
            _ => vec![],
        };
        let mems: Vec<Option<i64>> = match py::get(c, "mem") {
            Some(PyValue::List(l)) => l
                .iter()
                .map(|v| match v {
                    PyValue::Int(n) => Some(i64::try_from(n.clone()).unwrap()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        let wall = match py::get(c, "wall") {
            Some(PyValue::Int(n)) => i64::try_from(n.clone()).unwrap(),
            _ => 1000,
        };
        let runs = py::get(c, "runs").cloned().unwrap_or(PyValue::None);
        let clock = Rc::new(Cell::new(1000.0f64));
        let calls: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(vec![]));
        let (out, err) = (Buf(Rc::new(RefCell::new(vec![]))), Buf(Rc::new(RefCell::new(vec![]))));
        let rc = {
            let mut it = mems.into_iter();
            let (c1, c2) = (clock.clone(), clock.clone());
            let calls2 = calls.clone();
            let mut h = Hooks {
                mem: Box::new(move || it.next()),
                monotonic: Box::new(move || c1.get()),
                sleep: Box::new(move |s| c2.set(c2.get() + s)),
                wall: Box::new(move || wall),
                run: Box::new(move |argv: &[String], t: u64| {
                    calls2.borrow_mut().push(format!("{argv:?} {t}"));
                    match py::get(&runs, &argv[0]) {
                        Some(PyValue::Str(s)) if s.equals("oserror") => {
                            Err(format!("[Errno 2] No such file or directory: '{}'", argv[0]))
                        }
                        Some(PyValue::Str(s)) if s.equals("timeout") => Err(agent_guard::timeout_text(argv, t)),
                        Some(PyValue::List(l)) => Ok((
                            i32::try_from(match &l[0] {
                                PyValue::Int(n) => i64::try_from(n.clone()).unwrap(),
                                _ => 0,
                            })
                            .unwrap(),
                            py::display(&l[1]),
                            py::display(&l[2]),
                        )),
                        _ => panic!("unexpected run {argv:?}"),
                    }
                }),
                out: Box::new(out.clone()),
                err: Box::new(err.clone()),
            };
            agent_guard::main_with(&argv, &mut h)
        };
        std::env::set_current_dir(&scratch).unwrap();
        let want_rc = match py::get(c, "rc") {
            Some(PyValue::Int(n)) => Some(i32::try_from(n.clone()).unwrap()),
            _ => None,
        };
        if rc != want_rc {
            bad.push(format!("{name}: rc golden={want_rc:?} actual={rc:?}"));
        }
        let so = String::from_utf8_lossy(&out.0.borrow()).into_owned();
        let se = String::from_utf8_lossy(&err.0.borrow()).into_owned();
        if py::get_str(c, "stdout").unwrap_or_default() != so {
            bad.push(format!("{name}: stdout\n--- golden\n{}--- actual\n{so}", py::get_str(c, "stdout").unwrap_or_default()));
        }
        if reduce(&py::get_str(c, "stderr").unwrap_or_default()) != reduce(&se) {
            bad.push(format!("{name}: stderr\n--- golden\n{}--- actual\n{se}", py::get_str(c, "stderr").unwrap_or_default()));
        }
        let mut files = BTreeMap::new();
        walk(&dir, &dir, &mut files);
        for s in &setup {
            files.remove(s);
        }
        let want: BTreeMap<String, String> = match py::get(c, "files") {
            Some(PyValue::Dict(d)) => d.iter().map(|(k, v)| (py::text(k), py::display(v))).collect(),
            _ => BTreeMap::new(),
        };
        if want != files {
            bad.push(format!("{name}: files\n--- golden\n{want:?}\n--- actual\n{files:?}"));
        }
        let want_calls: Vec<String> = match py::get(c, "calls") {
            Some(PyValue::List(l)) => l
                .iter()
                .map(|x| match x {
                    PyValue::List(v) => {
                        let (a, t) = v.split_at(v.len() - 1);
                        format!("{:?} {}", a.iter().map(py::display).collect::<Vec<_>>(), py::display(&t[0]))
                    }
                    _ => String::new(),
                })
                .collect(),
            _ => vec![],
        };
        if want_calls != *calls.borrow() {
            bad.push(format!("{name}: calls golden={want_calls:?} actual={:?}", calls.borrow()));
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(bad.is_empty(), "trace mismatches:\n{}", bad.join("\n"));
}
