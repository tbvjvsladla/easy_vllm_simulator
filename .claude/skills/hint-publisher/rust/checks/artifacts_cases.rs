//! `artifacts` checks (owner: the artifacts port).

// ── a1: probe-only docker runner (the rule decisions, with an injected real call — no docker needed) ──
mod a1 {
    use std::cell::RefCell;
    use std::rc::Rc;

    use hintlib::artifacts::{self as a, DockerRunner, RunFail};
    use hintlib::core::Proc;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn runner() -> (Rc<a::_ProbeOnlyRunner>, Rc<RefCell<Vec<Vec<String>>>>) {
        let calls: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let log = calls.clone();
        let real: a::RealRun = Rc::new(move |argv: &[String], _t: u64| -> Result<Proc, RunFail> {
            log.borrow_mut().push(argv.to_vec());
            let out = if argv.get(1).map(String::as_str) == Some("create") { format!("noise\n{}\n", "c".repeat(12) + &"0".repeat(52)) } else { String::new() };
            Ok(Proc { returncode: 0, stdout: out, stderr: String::new() })
        });
        (a::_probe_only_runner(Some(real)).unwrap(), calls)
    }

    #[test]
    fn probe_only_rule() {
        let (r, calls) = runner();
        assert!(r.image_probe());
        let cid = "c".repeat(12) + &"0".repeat(52);
        // refused without calling: run/start/exec/build/pull/compose · create without both flags · cp/rm of foreign containers
        for argv in [
            s(&["docker", "run", "x"]),
            s(&["docker", "start", "x"]),
            s(&["docker", "exec", "x"]),
            s(&["docker", "build", "."]),
            s(&["docker", "pull", "x"]),
            s(&["docker", "compose", "up"]),
            s(&["docker"]),
            s(&["docker", "create", "--pull", "never", "img"]),
            s(&["docker", "create", "--network", "none", "img"]),
            s(&["docker", "cp", &format!("{cid}:/x"), "/tmp/y"]),
            s(&["docker", "rm", "-v", &cid]),
        ] {
            assert_eq!(r.run(&argv, 60).unwrap().returncode, 125, "{argv:?}");
        }
        assert!(calls.borrow().is_empty());
        // allowed: inspect · history · create with both flags → its container may be cp'd and rm'd
        for argv in [s(&["docker", "image", "inspect", "x"]), s(&["docker", "history", "x"])] {
            assert_eq!(r.run(&argv, 60).unwrap().returncode, 0);
        }
        assert_eq!(r.run(&s(&["docker", "create", "--pull=never", "--network", "none", "img"]), 60).unwrap().returncode, 0);
        assert_eq!(r.run(&s(&["docker", "cp", "-L", &format!("{cid}:/x"), "/tmp/y"]), 60).unwrap().returncode, 0);
        assert_eq!(r.run(&s(&["docker", "rm", "-v", &cid]), 60).unwrap().returncode, 0);
        // removed → no longer ours
        assert_eq!(r.run(&s(&["docker", "rm", "-v", &cid]), 60).unwrap().returncode, 125);
        assert_eq!(calls.borrow().len(), 5);
    }

    #[test]
    fn closures_do_not_declare_probe_and_read_only_refuses_run() {
        let f = |_: &[String], _: u64| -> Result<Proc, RunFail> { Err(RunFail::Timeout) };
        assert!(!f.image_probe());
        let ro = a::_DockerReadOnly { real: Some(Rc::new(|_: &[String], _: u64| Ok(Proc::default()))) };
        assert!(!ro.image_probe());
        let p = ro.run(&s(&["docker", "run", "x"]), 60).unwrap();
        assert_eq!((p.returncode, p.stderr.as_str()), (125, "hint.sh --docker-read-only: 이미지 실행(원장 탐침) 거부"));
        assert_eq!(ro.run(&s(&["docker", "history", "x"]), 60).unwrap().returncode, 0);
    }

    #[test]
    fn bytes_codec_and_memo_keys() {
        let b = a::_bytes_j(&[0, 255, 16]);
        assert_eq!(a::_j_bytes(&b), Some(vec![0, 255, 16]));
        assert_eq!(a::_Ctx::memo_key(&hintlib::jl!["applied", true]), "applied\u{1f}True");
        assert_eq!(a::_Ctx::memo_key(&hintlib::value::J::from("layers")), "layers");
    }
}

// ── a2: static analysis of build patch scripts + labelled reconstruction (Python selftest F12 fixtures) ──
mod a2 {
    use hintlib::artifacts as a;
    use hintlib::value::J;
    use hintlib::{jd, jl};

    const LOUD60: &str = "#!/bin/bash\nset -euo pipefail\nTAG=\"[60]\"\nDST=\"/workspace/vllm-src\"\n[ -d \"$DST/vllm\" ] || { echo \"$TAG FAIL\" >&2; exit 1; }\npython3 - \"$DST\" <<'PY'\nimport sys\np = sys.argv[1] + \"/vllm/a.py\"\ns = open(p).read()\nif \"anchor\" not in s:\n    raise SystemExit(1)\nopen(p, \"w\").write(s.replace(\"anchor\", \"anchor  # [port:60-fx]\"))\nPY\ngrep -q 'port:60-fx' \"$DST/vllm/a.py\" || { echo \"$TAG FAIL: x\" >&2; exit 1; }\nif grep -q 'already-upstream' \"$DST/vllm/a.py\"; then echo \"$TAG FAIL\" >&2; exit 1; fi\n";
    const GATE50: &str = "#!/bin/bash\nset -euo pipefail\n: \"${SM12X_PORT:=0}\"\nif [ \"${SM12X_PORT}\" != \"1\" ]; then\n    echo \"[50] skip\"\n    exit 0\nfi\necho applying\n";
    const TWO56: &str = "#!/bin/bash\nset -euo pipefail\n: \"${FX_OPEN:=0}\"\n: \"${FX_SHUT:=0}\"\nif [ \"${FX_OPEN}\" != \"1\" ]; then\n    exit 0\nfi\nif [ \"${FX_SHUT}\" != \"1\" ]; then\n    exit 0\nfi\necho go\n";

    fn env_ok(probe: J) -> J {
        jd! {"loop" => jl![true, "fx 루프"], "image_exists" => true, "identity_ok" => true, "probe" => probe}
    }
    fn res(r: &J) -> (String, String) {
        (r["result"].display(), r["result_source"].display())
    }

    #[test]
    fn script_shape_decisions() {
        let sh = a::_script_shape(LOUD60).unwrap();
        // a grep inside `if` is a tripwire, not an applied marker; the heredoc body is not shell code
        assert_eq!(sh["markers"].len(), 1);
        assert_eq!(sh["markers"][0]["path"].display(), "/workspace/vllm-src/vllm/a.py");
        assert!(sh["errexit"].truthy() && sh["exits"].items().is_empty());
        let ev = a::_script_shape("#!/bin/bash\nset -e\neval 'exit 0'\n[ -f /x ] || exit 1\n").unwrap();
        assert_eq!(ev["exits"].items().iter().map(|e| e["code"].display()).collect::<Vec<_>>(), vec!["0"]);
        let sw = a::_script_shape("#!/bin/bash\nset -e\nrm -rf /t || true\nV=\"$(grep x /f || true)\"\nfoo || true\n").unwrap();
        assert_eq!(sw["swallow"], jl![5]);
        assert!(!a::_script_shape("#!/bin/bash\nset -e\nset +e\n").unwrap()["errexit"].truthy());
    }

    #[test]
    fn self_gate_and_reconstruct() {
        let g = a::_self_gate(GATE50).unwrap().unwrap();
        assert_eq!(g["guards"], jl!["SM12X_PORT"]);
        assert!(a::_self_gate("#!/bin/bash\nrm -rf /x\n: \"${A:=0}\"\nif [ \"$A\" != \"1\" ]; then\n exit 0\nfi\n").unwrap().is_none());
        let none = env_ok(J::Null);
        assert_eq!(res(&a::_reconstruct("50", GATE50, "pre", &jd! {"SM12X_PORT" => "0"}, &none).unwrap()).0, "skipped");
        // `:=` fills an empty arg with the default (0 → skip); a missing arg = unobserved
        assert_eq!(res(&a::_reconstruct("50", GATE50, "pre", &jd! {"SM12X_PORT" => ""}, &none).unwrap()).0, "skipped");
        assert_eq!(res(&a::_reconstruct("50", GATE50, "pre", &J::dict(), &none).unwrap()).0, "unobserved");
        let r56 = a::_reconstruct("56", TWO56, "pre", &jd! {"FX_OPEN" => "1", "FX_SHUT" => "0"}, &none).unwrap();
        assert!(r56["reason"].display().contains("전부 열리지는"));
        let probe = jd! {"ran" => true, "files" => jd! {"/workspace/vllm-src/vllm/a.py" => "anchor  # [port:60-fx]\n"}, "modules" => J::dict()};
        assert_eq!(
            res(&a::_reconstruct("60", LOUD60, "pre", &J::dict(), &env_ok(probe)).unwrap()),
            ("applied".to_string(), "reconstructed(fail-loud+built)".to_string())
        );
        let missing = jd! {"ran" => true, "files" => jd! {"/workspace/vllm-src/vllm/a.py" => "anchor\n"}, "modules" => J::dict()};
        let r = a::_reconstruct("60", LOUD60, "pre", &J::dict(), &env_ok(missing)).unwrap();
        assert!(r["result"].display() == "unobserved" && r["reason"].display().contains("선언 마커 부재"));
        let noid = jd! {"loop" => jl![true, "l"], "image_exists" => true, "identity_ok" => false};
        let r = a::_reconstruct("60", LOUD60, "pre", &J::dict(), &noid).unwrap();
        assert!(r["result"].display() == "unobserved" && r["reason"].display().contains("--docker-read-only"));
        let trap = a::_reconstruct("71", "#!/bin/bash\nset -e\ntrap 'exit 0' ERR\npython3 /x.py\n", "pre", &J::dict(), &env_ok(J::Null)).unwrap();
        assert!(trap["result"].display() == "unobserved" && trap["reason"].display().contains("trap"));
    }

    #[test]
    fn ledger_rows_and_labels() {
        let rows = a::_ledger_rows(&jd! {"patches" => jl![
            jd! {"phase" => "post", "file" => "b.sh", "result" => "applied"},
            jd! {"phase" => "pre", "file" => "a.sh", "result" => "odd"}]})
        .unwrap();
        assert_eq!(rows[0]["phase"].display(), "pre");
        assert_eq!(rows[0]["result"].display(), "unobserved");
        assert!(a::_post_label(&rows[1]).unwrap()["shared_image_input"].truthy());
        assert_eq!(
            a::_docker_triplet_env_shape("QUANT_MODEL_PATH=/q\n").unwrap(),
            ("QUANT_MODEL_PATH=<manifest.quant_model_path>\n".to_string(), true)
        );
    }
}

// ── selftest ──
#[test]
fn artifacts_selftest_is_clean() {
    let bad = hintlib::artifacts::selftest();
    for b in &bad {
        eprintln!("{b}");
    }
    assert_eq!(bad, Vec::<String>::new());
}

// ── image_git_release ──
// Port of `scripts/hintlib/selftest_image_git_release.py` (unittest; run by verify_distribution `hint_image_git_release_selftest`).
// Translations: `mock.patch(artifacts._ctx)` → `bind_image_git_release_with(.., Some(runner))` · `mock.patch(evidence._self_node)`
// → `_self_node_cache_put` · `_attestation_ref`/`_image_ref` mocks → the real functions (the fixture digest is the measured
// `image_digest`) · `subprocess.run` argv/env spy → not observable in Rust (the hostile-helper marker is the observed fact) ·
// `hint.cmd_name` mock graph → static fact: `cmd_name` does not reference `bind_image_git_release`.
mod image_git_release {
    use std::cell::{Cell, RefCell};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::rc::Rc;

    use hintlib::artifacts::{self as a, DockerRunner, RunFail};
    use hintlib::core::{self, Proc};
    use hintlib::value::J;
    use hintlib::{evidence, jd, jl};

    fn git(dir: &Path, args: &[&str]) -> std::process::Output {
        let o = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        o
    }

    type Calls = Rc<RefCell<Vec<Vec<String>>>>;
    type Hook = Rc<dyn Fn(&[String]) -> Option<Proc>>;

    /// `ImageGitRelease.docker` + `runner.image_probe` (shared flag).
    struct Fake {
        repo: Rc<RefCell<PathBuf>>,
        calls: Calls,
        probe: Rc<Cell<bool>>,
        hook: Option<Hook>,
    }
    impl DockerRunner for Fake {
        fn run(&self, argv: &[String], _t: u64) -> Result<Proc, RunFail> {
            let args = &argv[1..];
            self.calls.borrow_mut().push(args.to_vec());
            if let Some(h) = &self.hook {
                if let Some(p) = h(args) {
                    return Ok(p);
                }
            }
            let ok = |out: &str| Ok(Proc { returncode: 0, stdout: out.to_string(), stderr: String::new() });
            match args[0].as_str() {
                "create" => ok(&"a".repeat(64)),
                "cp" => {
                    let src = self.repo.borrow().join(".git");
                    let st = Command::new("cp").arg("-a").arg(&src).arg(args.last().unwrap()).status().unwrap();
                    assert!(st.success());
                    ok("")
                }
                "rm" => ok(""),
                other => panic!("unexpected docker {other}"),
            }
        }
        fn image_probe(&self) -> bool {
            self.probe.get()
        }
    }

    struct T {
        _tmp: core::TempDir,
        root: PathBuf,
        repo: Rc<RefCell<PathBuf>>,
        sha: String,
        ev: J,
        calls: Calls,
        probe: Rc<Cell<bool>>,
        hook: Option<Hook>,
    }

    fn setup() -> T {
        let tmp = core::TempDir::new("hint-igr-");
        let root = core::resolve(tmp.path());
        // Python mocks `_ctx`; the Rust context checks the owner module is present, so stage it (never executed here).
        let own = core::skill_dir().join("../../..");
        let sf = root.join(core::REL_SLAVE_FORWARD);
        std::fs::create_dir_all(sf.parent().unwrap()).unwrap();
        std::fs::copy(own.join(core::REL_SLAVE_FORWARD), &sf).unwrap();
        let repo = root.join("upstream");
        let o = Command::new("git").args(["init", "-q"]).arg(&repo).status().unwrap();
        assert!(o.success());
        git(&repo, &["commit", "--allow-empty", "-qm", "fixture"]);
        git(&repo, &["tag", "v1.2.3"]);
        let sha = String::from_utf8(git(&repo, &["rev-parse", "HEAD"]).stdout).unwrap().trim().to_string();
        let ev = jd! {"build_identity" => jd! {"vllm_ref" => sha.as_str(), "vllm_sha" => sha.as_str(),
            "vllm_repo" => "https://github.com/vllm-project/vllm.git", "image_digest" => format!("sha256:{}", "a".repeat(64)),
            "track" => "source-build", "source" => J::dict()},
            // `cell`: the mocked Python `_ctx` skipped cell validation — the Rust context needs one
            "cell" => "fixture", "attestation" => J::Null, "node" => "sub", "topology" => "single",
            "manifest" => jd! {"topology" => "single", "self_role" => "sub"}};
        T { _tmp: tmp, root, repo: Rc::new(RefCell::new(repo)), sha, ev, calls: Rc::default(), probe: Rc::new(Cell::new(true)), hook: None }
    }

    impl T {
        fn runner(&self) -> Rc<dyn DockerRunner> {
            Rc::new(Fake { repo: self.repo.clone(), calls: self.calls.clone(), probe: self.probe.clone(), hook: self.hook.clone() })
        }
        fn self_node(&self, node: Option<&str>) {
            evidence::_self_node_cache_put(&core::posix(&self.root), Some((node.map(str::to_string), "fixture".into())));
        }
        /// `probe()` — `_ctx` → fake runner · `_self_node` → ("sub", "fixture").
        fn probe(&mut self) -> hintlib::core::HR<()> {
            self.probe_as(Some("sub"))
        }
        fn probe_as(&mut self, node: Option<&str>) -> hintlib::core::HR<()> {
            self.self_node(node);
            let r = self.runner();
            a::bind_image_git_release_with(&self.root.clone(), &mut self.ev, Some(r))
        }
        fn tag(&self) -> Option<String> {
            self.ev["build_identity"].get("image_git_exact_tag").map(J::display)
        }
        fn pop_tag(&mut self) {
            self.ev.get_mut("build_identity").unwrap().as_dict_mut().unwrap().remove("image_git_exact_tag");
        }
        fn verbs(&self) -> Vec<String> {
            self.calls.borrow().iter().map(|c| c[0].clone()).collect()
        }
        fn bi_set(&mut self, k: &str, v: J) {
            self.ev.get_mut("build_identity").unwrap().set(k, v);
        }
        fn ctx(&self, runner: Rc<dyn DockerRunner>, node: &str) -> a::_Ctx {
            let mut c = a::_Ctx::new(&self.root, &self.ev, Some(runner), None, None, None, "", "", node);
            c.default_runner.set(false);
            c.out = self.root.join("output");
            c
        }
    }

    #[test]
    fn real_partial_clone_local_objects_and_missing_objects() {
        let mut t = setup();
        let up = t.repo.borrow().clone();
        git(&up, &["config", "uploadpack.allowFilter", "true"]);
        git(&up, &["tag", "-a", "v1.2.4", "-m", "annotated"]);
        git(&up, &["tag", "-d", "v1.2.3"]);
        let partial = t.root.join("partial");
        let url = format!("file://{}", core::posix(&up));
        let st = Command::new("git").args(["clone", "-q", "--filter=blob:none", "--no-checkout", &url]).arg(&partial).status().unwrap();
        assert!(st.success());
        let packs: Vec<_> = std::fs::read_dir(partial.join(".git/objects/pack")).unwrap().flatten().collect();
        assert!(packs.iter().any(|e| e.path().extension().is_some_and(|x| x == "promisor")));
        *t.repo.borrow_mut() = partial.clone();
        t.probe().unwrap();
        assert_eq!(t.tag().as_deref(), Some("v1.2.4"));
        t.pop_tag();
        // forge a missing tag object; copied hostile helpers/config must never execute
        let marker = t.root.join("helper-ran");
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(partial.join(".git/config")).unwrap();
            write!(f, "\n[credential]\n helper = !touch {}\n[remote \"origin\"]\n uploadpack = touch {}\n", core::posix(&marker), core::posix(&marker)).unwrap();
        }
        let packed = partial.join(".git/packed-refs");
        let original = std::fs::read_to_string(&packed).unwrap();
        let mut forged = Vec::new();
        let mut drop_peeled = false;
        for line in original.lines() {
            if line.ends_with(" refs/tags/v1.2.4") {
                forged.push(format!("{} refs/tags/v1.2.4", "f".repeat(40)));
                drop_peeled = true;
            } else if drop_peeled && line.starts_with('^') {
                drop_peeled = false;
            } else {
                forged.push(line.to_string());
                drop_peeled = false;
            }
        }
        std::fs::write(&packed, forged.join("\n") + "\n").unwrap();
        t.probe().unwrap();
        assert!(t.tag().is_none());
        assert!(!marker.exists());
        std::fs::write(&packed, &original).unwrap();
        for e in std::fs::read_dir(partial.join(".git/objects/pack")).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "pack" || x == "idx") {
                std::fs::remove_file(e.path()).unwrap();
            }
        }
        t.probe().unwrap();
        assert!(t.tag().is_none());
        assert!(!marker.exists());
        assert_eq!(t.verbs(), ["create", "cp", "rm"].repeat(3));
    }

    fn ledger_hook(sha: &str, digest: &str, with_image: bool) -> (Hook, J) {
        let ledger = jd! {"provenance" => "measured(build-time, in-image)",
            "vllm" => jd! {"git_sha" => sha, "describe" => "v1.2.3", "dist_version" => "1.2.4.dev0+gfixture"}, "patches" => J::list()};
        let txt = hintlib::value::dumps_default(&ledger);
        let digest = digest.to_string();
        let h: Hook = Rc::new(move |args: &[String]| -> Option<Proc> {
            let ok = |o: &str| Some(Proc { returncode: 0, stdout: o.to_string(), stderr: String::new() });
            match args[0].as_str() {
                "image" if with_image => ok(&digest),
                "image" => ok("measured"),
                "cp" if with_image => {
                    std::fs::write(args.last().unwrap(), &txt).unwrap();
                    ok("")
                }
                "create" | "cp" | "rm" if !with_image => panic!("read-only sub must not execute image"),
                _ => None,
            }
        });
        (h, ledger)
    }

    #[test]
    fn local_sub_ledger_is_bound_as_build_time_not_later_layer() {
        let mut t = setup();
        let digest = t.ev["build_identity"]["image_digest"].display();
        for (k, v) in [
            ("declaration", jd! {"nodes" => jl![jd! {"node_id" => "sub", "role" => "sub"}]}),
            ("image_ledger", J::Null),
            ("resolve", J::Null),
            ("sources", J::dict()),
            ("plane", J::from("docker")),
            ("pip_freeze_paths", J::dict()),
            ("pip_freeze_path", J::Null),
        ] {
            t.ev.set(k, v);
        }
        let (h, ledger) = ledger_hook(&t.sha, &digest, true);
        t.hook = Some(h);
        t.self_node(Some("sub"));
        let c = t.ctx(t.runner(), "sub");
        let (got, source, _, probes) = a::_observe_ledger(&c).unwrap();
        assert_eq!(got.as_ref(), Some(&ledger));
        let source = source.unwrap_or_default();
        assert!(source.contains("후속 이미지 층은 별도 관측"), "{source}");
        assert_eq!(probes.last().unwrap()["result"].display(), "observed");
        let v = t.verbs();
        assert_eq!(&v[v.len() - 3..], ["create", "cp", "rm"]);
        assert!(v.iter().all(|x| ["image", "create", "cp", "rm"].contains(&x.as_str())));
        evidence::bind_image_ledger_mut(&mut t.ev, &jd! {"vllm" => ledger["vllm"].clone(), "source" => source.as_str(), "provenance" => ledger["provenance"].clone()})
            .unwrap();
        let (version, where_) = evidence::_wheel_meta(&t.root, &t.ev).unwrap();
        assert_eq!(version.as_deref(), Some("1.2.4.dev0+gfixture"));
        assert!(where_.contains("빌드 시점"), "{where_}");
        assert!(!version.unwrap().contains("release"));
    }

    #[test]
    fn sub_ledger_remote_missing_digest_and_multi_sub_do_not_probe() {
        let mut t = setup();
        let digest = format!("sha256:{}", "a".repeat(64));
        for (local, topology, dg) in [("main", "single", Some(digest.as_str())), ("sub", "single", None), ("sub", "multi", Some(digest.as_str()))] {
            t.ev.set("topology", topology);
            t.bi_set("image_digest", J::from(dg));
            t.self_node(Some(local));
            let c = t.ctx(t.runner(), "sub");
            let (ledger, _, _, probes) = a::_observe_ledger(&c).unwrap();
            assert!(ledger.is_none());
            assert!(probes.last().unwrap()["result"].display().starts_with("skipped"), "{local} {topology}");
        }
        t.ev.set("node", "main");
        t.ev.set("topology", "multi");
        t.self_node(Some("sub"));
        let c = t.ctx(t.runner(), "main");
        let (ledger, _, _, probes) = a::_observe_ledger(&c).unwrap();
        assert!(ledger.is_none());
        assert!(probes.last().unwrap()["result"].display().starts_with("skipped"));
        assert!(t.calls.borrow().is_empty());
    }

    #[test]
    fn multi_sub_git_and_readonly_sub_ledger_never_run() {
        let mut t = setup();
        t.ev.set("topology", "multi");
        t.ev.set("manifest", jd! {"topology" => "multi", "self_role" => "sub"});
        t.probe().unwrap();
        assert!(t.calls.borrow().is_empty());
        t.ev.set("topology", "single");
        t.ev.set("manifest", jd! {"topology" => "single", "self_role" => "sub"});
        t.probe.set(false);
        let digest = t.ev["build_identity"]["image_digest"].display();
        let (h, _) = ledger_hook(&t.sha, &digest, false);
        t.hook = Some(h);
        t.self_node(Some("sub"));
        let c = t.ctx(t.runner(), "sub");
        let (ledger, _, _, probes) = a::_observe_ledger(&c).unwrap();
        assert!(ledger.is_none());
        assert!(t.verbs().iter().all(|x| x == "image"));
        assert!(probes.last().unwrap()["result"].display().contains("no image execution"));
    }

    #[test]
    fn public_name_path_does_not_probe() {
        // translation: the Python mocks the whole `hint.cmd_name` input graph; the Rust fact is that `cmd_name` never reaches
        // `bind_image_git_release` (static: its body references no probe).
        let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli/ops.rs")).unwrap();
        let start = src.find("pub fn cmd_name(").expect("cmd_name");
        let rest = &src[start + 1..];
        let end = rest.find("\npub fn ").map_or(rest.len(), |i| i);
        let body = &rest[..end];
        assert!(!body.contains("bind_image_git_release"));
    }

    #[test]
    fn exact_bound_tag_and_no_image_start() {
        let mut t = setup();
        t.probe().unwrap();
        assert_eq!(evidence::_prev_release(&t.ev).unwrap().as_deref(), Some("v1.2.3"));
        let digest = t.ev["build_identity"]["image_digest"].display();
        assert!(t.ev["build_identity"]["source"]["image_git_exact_tag"].display().contains(&digest));
        assert_eq!(t.verbs(), ["create", "cp", "rm"]);
        let mut facts = t.ev["build_identity"].clone();
        facts.set("prev_release", J::from(evidence::_prev_release(&t.ev).unwrap()));
        assert_eq!(hintlib::naming::vllm_segment(&facts).unwrap().value, format!("1.2.3-g{}", &t.sha[..12]));
    }

    #[test]
    fn wrong_sha_unknown_repository_or_digest_fails_closed() {
        let mut t = setup();
        for (field, val) in [
            ("vllm_sha", J::from("b".repeat(40))),
            ("vllm_repo", J::from("https://github.com/fork/vllm")),
            ("vllm_repo", J::Null),
            ("image_digest", J::Null),
        ] {
            let before = t.ev["build_identity"][field].clone();
            t.bi_set(field, val);
            t.probe().unwrap();
            assert!(t.tag().is_none(), "{field}");
            t.bi_set(field, before);
        }
        assert!(t.calls.borrow().is_empty());
    }

    #[test]
    fn mismatched_head_multiple_tags_and_missing_tag_fail_closed() {
        let mut t = setup();
        let up = t.repo.borrow().clone();
        git(&up, &["tag", "v1.2.4"]);
        t.probe().unwrap();
        assert!(t.tag().is_none());
        git(&up, &["tag", "-d", "v1.2.3", "v1.2.4"]);
        t.probe().unwrap();
        assert!(t.tag().is_none());
        git(&up, &["commit", "--allow-empty", "-qm", "later"]);
        t.probe().unwrap();
        assert!(t.tag().is_none());
        assert_eq!(t.verbs().iter().filter(|v| *v == "rm").count(), 3);
    }

    #[test]
    fn other_or_unknown_local_node_never_probes() {
        let mut t = setup();
        for local in [Some("main"), None] {
            t.probe_as(local).unwrap();
            assert!(t.calls.borrow().is_empty());
        }
    }

    #[test]
    fn cluster_probe_is_owned_by_the_unique_multi_main() {
        let mut t = setup();
        let main_sub = jd! {"nodes" => jl![jd! {"node_id" => "primary-7", "role" => "main"}, jd! {"node_id" => "sub", "role" => "sub"}]};
        t.ev.set("node", "cluster");
        t.ev.set("topology", "multi");
        t.ev.set("manifest", jd! {"topology" => "multi", "self_role" => "main"});
        t.ev.set("declaration", main_sub.clone());
        t.probe_as(Some("primary-7")).unwrap();
        assert_eq!(t.tag().as_deref(), Some("v1.2.3"));
        t.pop_tag();
        t.calls.borrow_mut().clear();
        for local in [Some("sub"), Some("cluster"), None] {
            t.probe_as(local).unwrap();
            assert!(t.calls.borrow().is_empty());
        }
        t.ev.set("topology", "single");
        t.probe_as(Some("primary-7")).unwrap();
        assert!(t.calls.borrow().is_empty());
        t.ev.set("topology", "multi");
        t.ev.set("declaration", jd! {"nodes" => jl![jd! {"node_id" => "primary-7", "role" => "sub"}]});
        t.probe_as(Some("primary-7")).unwrap();
        assert!(t.calls.borrow().is_empty());
        t.ev.set("declaration", jd! {"nodes" => jl![jd! {"node_id" => "sub", "role" => "main"}]});
        for manifest in [
            jd! {"topology" => "multi", "self_role" => "sub"},
            jd! {"topology" => "single", "self_role" => "main"},
            jd! {"topology" => "multi"},
            J::dict(),
            J::Null,
        ] {
            t.ev.set("manifest", manifest);
            t.probe_as(Some("sub")).unwrap();
            assert!(t.calls.borrow().is_empty());
        }
        t.ev.set("manifest", jd! {"topology" => "multi", "self_role" => "main"});
        t.ev.set("declaration", main_sub.clone());
        t.ev.set("node", "sub");
        t.probe_as(Some("primary-7")).unwrap();
        assert!(t.calls.borrow().is_empty());
        t.ev.set("node", "cluster");
        let mut d = main_sub;
        d.get_mut("nodes").unwrap().push(jd! {"node_id" => "other", "role" => "main"});
        t.ev.set("declaration", d);
        t.probe_as(Some("primary-7")).unwrap();
        assert!(t.calls.borrow().is_empty());
    }

    #[test]
    fn copied_root_symlink_is_rejected() {
        let mut t = setup();
        let target = t.repo.borrow().join(".git");
        t.hook = Some(Rc::new(move |args: &[String]| -> Option<Proc> {
            if args[0] == "cp" {
                std::os::unix::fs::symlink(&target, args.last().unwrap()).unwrap();
                return Some(Proc { returncode: 0, stdout: String::new(), stderr: String::new() });
            }
            None
        }));
        t.probe().unwrap();
        assert!(t.tag().is_none());
        assert_eq!(t.verbs().last().map(String::as_str), Some("rm"));
    }

    #[test]
    fn cleanup_failure_is_not_silently_accepted() {
        let mut t = setup();
        t.hook = Some(Rc::new(|args: &[String]| -> Option<Proc> {
            (args[0] == "rm").then(|| Proc { returncode: 1, stdout: String::new(), stderr: "fixture removal refused".into() })
        }));
        let e = t.probe().unwrap_err();
        assert_eq!(e.code, "HINT_IMAGE_PROBE_CLEANUP_FAILED");
        assert!(e.str().contains("fixture removal refused"), "{}", e.str());
        assert!(t.tag().is_none());
    }

    #[test]
    fn conflicting_ledger_describe_fails_closed() {
        let mut t = setup();
        t.probe().unwrap();
        t.ev.set("attestation", jd! {"build_ledger" => jd! {"vllm" => jd! {"describe" => "v9.9.9"}}});
        let e = evidence::_prev_release(&t.ev).unwrap_err();
        assert_eq!(e.code, "HINT_VLLM_INPUT_CONFLICT");
    }

    #[test]
    fn alternates_symlink_and_readonly_runner_fail_closed() {
        let mut t = setup();
        let up = t.repo.borrow().clone();
        let alt = up.join(".git/objects/info/alternates");
        std::fs::write(&alt, "/tmp/foreign\n").unwrap();
        t.probe().unwrap();
        assert!(t.tag().is_none());
        std::fs::remove_file(&alt).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", up.join(".git/objects/info/bad")).unwrap();
        t.probe().unwrap();
        assert!(t.tag().is_none());
        t.probe.set(false);
        let n = t.calls.borrow().len();
        t.probe().unwrap();
        assert_eq!(t.calls.borrow().len(), n);
    }
}
