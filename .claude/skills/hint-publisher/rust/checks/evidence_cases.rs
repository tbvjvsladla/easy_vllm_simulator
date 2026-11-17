//! `evidence` checks (owner: the evidence port).

// ── e1 (CellEvidence shape · completion_gate mirror · gate wrapper · docker probe hooks) ──
mod e1 {
    use hintlib::core::Proc;
    use hintlib::evidence as e;
    use hintlib::jd;
    use hintlib::value::J;
    use std::path::Path;

    #[test]
    fn e1_cell_evidence_shape() {
        let ev = e::new_cell_evidence("campaign", Some("camp"), "c1", "main", "single");
        let keys: Vec<&str> = ev.as_dict().unwrap().keys().collect();
        assert_eq!(keys, e::CELL_EVIDENCE_FIELDS.to_vec());
        assert_eq!(ev["triplet"], J::dict());
        assert_eq!(ev["missing"], J::list());
        assert_eq!(ev["sweep"], J::Null);
        assert_eq!(e::new_cell_evidence("publication-replay", None, "c", "n", "t")["campaign_id"], J::Null);
    }

    #[test]
    fn e1_flat_certificate_mirror() {
        let g = e::CGate { path: Default::default() };
        let (f, ok) = g.parse_flat_certificate("# c\nmodel: m\nnote: \"a \\\"b\\\"\"\n\nempty:\n");
        assert!(ok);
        assert_eq!(f, jd! {"model" => "m", "note" => "a \"b\"", "empty" => ""});
        assert!(!g.parse_flat_certificate("a: 1\na: 2\n").1);
        assert!(!g.parse_flat_certificate("  a: 1\n").1);
        assert!(!g.parse_flat_certificate("a: 1 \n").1);
        assert!(!g.parse_flat_certificate("bad line\n").1);
        let cert = jd! {"model" => "m", "gpu_model" => "g", "vllm_version" => "v", "quantization" => "q", "topology" => "t",
                        "tensor_parallel_size" => " 1 ", "measured_utc" => "u"};
        assert_eq!(g.certificate_run_key(&cert).unwrap(), vec!["m", "g", "v", "q", "t", "1", "u"]);
        let mut c2 = cert.clone();
        c2.set("measured_utc", " ");
        assert!(g.certificate_run_key(&c2).is_none());
        let (st, parts) = g.lexical_components(Path::new("/r/docs"), Path::new("/r"), "../x/./y.yaml");
        assert_eq!((st.as_str(), parts.unwrap()), ("ok", vec!["x".to_string(), "y.yaml".to_string()]));
        assert_eq!(g.lexical_components(Path::new("/r"), Path::new("/r"), "../../z").0, "repo_escape");
        assert!(g.certificate_waived("full_benchmark", &J::from("PASS"), &J::from("explore")));
        assert!(!g.certificate_waived("full_benchmark", &J::from("PASS"), &J::from("explicit")));
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let base = std::env::var_os("TMPDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "/tmp".into());
        let d = base.join(format!("hp-e1-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".claude/policies/runtime")).unwrap();
        d
    }

    #[test]
    fn e1_authorize_gate_wrapper() {
        let d = scratch("auth");
        // gate absent = owner module missing (the original's _cgate_path)
        let r = e::authorize(&d, &J::from("m.json"), "x").unwrap_err();
        assert_eq!(r.code, "HINT_OWNER_MODULE_MISSING");
        // a launcher that is not the gate (its stdout is not the gate's JSON) = unreadable gate output (exit 2). The old
        // "source present, launcher absent" case cannot occur any more: the owner path is the launcher (plan_26111717).
        std::fs::write(d.join(".claude/policies/runtime/completion_gate.sh"), "x = 1\n").unwrap();
        let r = e::authorize(&d, &J::from("m.json"), "x").unwrap_err();
        assert_eq!((r.code.as_str(), r.exit_code), ("HINT_GATE_OUTPUT_UNREADABLE", 2));
        assert!(r.message.starts_with("completion_gate.sh authorize (exit="), "{}", r.message);
        // the API presence check of cgate(): a present launcher is judged by the Rust gate source this checkout builds
        // (not by the launcher's own text), so the API is present
        let g = e::cgate(&d).expect("launcher present: the API comes from the built gate source");
        assert!(g.path.ends_with(".claude/policies/runtime/completion_gate.sh"), "{:?}", g.path);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn e1_host_facts_observed_beats_declared() {
        let mut ev = e::new_cell_evidence("campaign", Some("c"), "c1", "cluster", "multi");
        ev.set("attestation", jd! {"_path" => "a.json", "config" => "c1",
            "parity" => jd!{"driver" => jd!{"main" => "580.178", "sub" => "580.178", "verdict" => "equal"}}});
        ev.set("manifest", jd! {"driver_version" => "580.173", "os" => "ubuntu 24.04"});
        ev.set("build_identity", jd! {"track" => "wheel"});
        e::_host_facts(&mut ev, None).unwrap();
        let b = &ev["build_identity"];
        let keys: Vec<&str> = b.as_dict().unwrap().keys().collect();
        assert_eq!(keys, vec!["track", "source", "driver", "driver_by_node", "driver_conflict", "driver_check", "os"]);
        assert_eq!(b["driver"], J::from("580.178"));
        assert_eq!(b["driver_by_node"], jd! {"main" => "580.178", "sub" => "580.178"});
        assert_eq!(b["driver_conflict"]["declared"], J::from("580.173"));
        assert_eq!(b["driver_check"]["verdict"], J::from("conflict"));
        assert_eq!(b["os"], J::from("ubuntu 24.04"));
        let src = b["source"]["driver"].as_str().unwrap();
        assert!(src.starts_with("attestation parity.driver(a.json · 노드별 관측 · verdict=equal) — main=sub 일치"), "{src}");
        assert!(src.ends_with(" · ⚠ 선언 580.173 과 다르다(driver_conflict)"), "{src}");
        // other run (attestation of another config): declared value wins, the observation is scoped in the source
        ev.set("attestation", jd! {"_path" => "a.json", "config" => "other",
            "parity" => jd!{"driver" => jd!{"main" => "580.178", "verdict" => "x"}}});
        e::_host_facts(&mut ev, None).unwrap();
        let b = &ev["build_identity"];
        assert_eq!(b["driver"], J::from("580.173"));
        assert!(b.get("driver_by_node").is_none() && b.get("driver_conflict").is_none());
        assert_eq!(b["driver_check"]["verdict"], J::from("unpaired"));
        assert!(b["source"]["driver"].as_str().unwrap().contains("다른 run(other) attestation 관측 580.178(main=580.178 · 이 run 아님"));
    }

    #[test]
    fn e1_docker_probe_hooks() {
        let seen = std::cell::RefCell::new(Vec::new());
        let fake = |argv: &[String]| -> Result<Proc, String> {
            seen.borrow_mut().push(argv.join(" "));
            if argv[1] == "history" {
                return Err("timeout".into());
            }
            Ok(Proc { returncode: 0, stdout: "[{\"Id\": \"sha256:x\", \"Config\": {\"Env\": [\"CUDA_VERSION=13\"]}}]".into(), stderr: String::new() })
        };
        let f = e::_image_facts_r("img:1", Some(&fake)).unwrap().unwrap();
        assert_eq!(f["id"], J::from("sha256:x"));
        assert_eq!(f["env"], jd! {"CUDA_VERSION" => "13"});
        assert_eq!(f["history_observed"], J::Bool(false));
        assert_eq!(
            *seen.borrow(),
            vec!["docker image inspect img:1".to_string(), "docker history --no-trunc --format {{.CreatedBy}} img:1".to_string()]
        );
        let r = e::_docker_r(Some(&fake), &["rm".into(), "x".into()]).unwrap_err();
        assert_eq!(r.code, "HINT_DOCKER_WRITE_REFUSED");
        assert!(r.message.ends_with("('rm', 'x')"), "{}", r.message);
        let nz = |_: &[String]| -> Result<Proc, String> { Ok(Proc { returncode: 1, stdout: "x".into(), stderr: String::new() }) };
        assert_eq!(e::_docker_r(Some(&nz), &["history".into()]).unwrap(), None);
        // _local_image_digest: probe-only; an image that cannot be inspected is recorded with id None
        let mut ev = e::new_cell_evidence("campaign", Some("c"), "c1", "main", "single");
        ev.set("build_identity", jd! {"image_tag" => "img:1", "image_digest_local" => "stale"});
        e::_local_image_digest_r(Path::new("."), &mut ev, Some(&nz)).unwrap();
        assert_eq!(ev["build_identity"]["image_digest_local"]["id"], J::Null);
        assert!(ev["build_identity"]["image_digest_local"]["source"].as_str().unwrap().contains("관측 불가"));
        // a measured digest = nothing to probe (the stale local value is dropped)
        ev.set("build_identity", jd! {"image_tag" => "img:1", "image_digest" => "sha256:m", "image_digest_local" => "stale"});
        e::_local_image_digest_r(Path::new("."), &mut ev, Some(&nz)).unwrap();
        assert!(ev["build_identity"].get("image_digest_local").is_none());
    }
}

// ── e4 (qualification · identity · facts · tails) ──
mod e4 {
    use hintlib::evidence as e;
    use hintlib::value::{Map, J};
    use hintlib::{jd, jl};
    use std::path::Path;

    #[test]
    fn e4_topology_label() {
        assert_eq!(e::topology_label(&jd! {"topology" => "multi", "tp" => 2}).unwrap(), "multi TP=2(Ray)");
        assert_eq!(e::topology_label(&jd! {"topology" => "single", "tp" => 1}).unwrap(), "single TP=1");
        assert_eq!(e::topology_label(&jd! {"topology" => "single", "tp" => true}).unwrap_err().code, "HINT_TOPOLOGY_UNDERIVABLE");
        assert_eq!(e::topology_label(&J::Null).unwrap_err().code, "HINT_TOPOLOGY_UNDERIVABLE");
    }

    #[test]
    fn e4_flatteners() {
        let sh = e::_flat_sh("A=1\nrun --a=1 --b 2 --c --d -- e\nrun --g \"open\n").unwrap();
        assert_eq!(sh["A"], J::from("1"));
        assert_eq!(sh["--a"], J::from("1"));
        assert_eq!(sh["--b"], J::from("2"));
        assert_eq!(sh["--c"], J::from("true"));
        assert!(sh.get("--g").is_none()); // unclosed quote = shlex ValueError → line skipped
        let env = e::_flat_env("A='x y'\n# c\nexport B=\"q\"\n").unwrap();
        assert_eq!(env, jd! {"A" => "x y", "B" => "q"});
        let mut out = Map::new();
        e::_flatten("", &jd! {"s" => "{\"n\": 2}", "l" => jl![1, jd! {"b" => true}]}, &mut out).unwrap();
        assert_eq!(J::Dict(out), jd! {"s" => "{\"n\": 2}", "s.n" => "2", "l" => "[1,{\"b\":true}]", "l.0" => "1", "l.1.b" => "true"});
    }

    #[test]
    fn e4_same_and_serve_proof() {
        assert!(e::_same(&J::from("0.9"), &J::Float(0.90)).unwrap());
        assert!(e::_same(&J::from("true"), &J::Bool(true)).unwrap());
        assert!(!e::_same(&J::from("x"), &J::from("y")).unwrap());
        let sp = jd! {"config" => "c1", "health_http" => 200, "inference_verdict" => "pass", "evidence" => "v1.completions",
                      "completion_text_len" => "5", "provenance" => "measured(x)"};
        assert!(e::_serve_proof_ok(&sp, "c1", None).unwrap().0);
        assert!(!e::_serve_proof_ok(&sp, "c2", None).unwrap().0);
    }

    #[test]
    fn e4_fail_closed_publisher_modes() {
        let ev = e::new_cell_evidence("publication-replay", None, "c1", "main", "single");
        let d = Path::new("/nonexistent-draft");
        let utc = J::from("2026-01-02T03:04:05Z");
        assert_eq!(e::drive_publisher(Path::new("."), &ev, &J::from("t"), &utc, d).unwrap_err().code, "HINT_REPLAY_READ_ONLY");
        let r = e::set_promotion_target(Path::new("."), &J::from("t"), &J::from("hint/x"), &J::from("single"), &J::from("a"), &utc, None);
        assert_eq!(r.unwrap_err().code, "HINT_PUBLISHER_INPUTS_ABSENT");
        assert_eq!(e::output_manifest(Path::new("."), "dual").unwrap_err().code, "HINT_TOPOLOGY_UNDERIVABLE");
    }
}

// ── e3 (banners · node axis) ──
mod e3 {
    use hintlib::evidence as e;
    use hintlib::jl;
    use hintlib::value::J;

    #[test]
    fn e3_mmdd_and_window() {
        assert_eq!(e::_mmdd(Some("2026-01-02T03:04:05Z")).unwrap().as_deref(), Some("01-02 03:04:05"));
        assert_eq!(e::_mmdd(None).unwrap(), None);
        let w = |s, a, b| e::_banner_in_window(Some(s), a, b).unwrap();
        assert_eq!(w("01-02 03:00:00", "2026-01-01T00:00:00Z", "2026-01-03T00:00:00Z").as_deref(), Some("2026-01-02T03:00:00Z"));
        assert_eq!(w("01-02 03:00:00", "2025-01-01T00:00:00Z", "2026-12-31T00:00:00Z"), None);
        assert_eq!(w("02-30 03:00:00", "2026-01-01T00:00:00Z", "2026-12-31T00:00:00Z"), None);
    }

    #[test]
    fn e3_pick_banner_conflict_and_nearest() {
        let c = |u: &str, v: &str, s: &str| jl![u, v, s, J::List(vec![jl![s, "engine_log"]])];
        let one = e::_pick_banner(&J::List(vec![c("2026-01-02T01:00:00Z", "0.9.0", "a"), c("2026-01-02T03:00:00Z", "0.9.0", "b")]),
            Some("2026-01-02T03:04:05Z"), "x", "w").unwrap();
        assert_eq!(one["basis"], J::from("x"));
        assert!(one["source"].as_str().unwrap().starts_with("b — w"));
        let two = e::_pick_banner(&J::List(vec![c("2026-01-02T01:00:00Z", "0.9.0", "a"), c("2026-01-02T03:00:00Z", "0.9.1", "b")]),
            None, "x", "w").unwrap();
        assert_eq!(two["basis"], J::from("conflict"));
        assert_eq!(two["value"], J::Null);
    }

    #[test]
    fn e3_resolve_node_fail_closed() {
        let code = |r: Result<(String, String), hintlib::core::HintError>| r.err().map(|e| e.code);
        assert_eq!(code(e::_resolve_node(&J::Null, &J::Null, "single", None, &[], "c", &J::Null)).as_deref(), Some("HINT_NODE_AXIS_UNDERIVABLE"));
        assert_eq!(code(e::_resolve_node(&J::from("cluster"), &J::Null, "single", None, &[], "c", &J::Null)).as_deref(),
            Some("HINT_ARCH_NODE_AXIS_CERT_MISMATCH"));
        assert_eq!(e::_resolve_node(&J::Null, &J::from(" main "), "single", None, &[], "c", &J::Null).unwrap().0, "main");
    }
}

// ── e2 (ledger windows · timeline helpers) ──
mod e2 {
    use hintlib::evidence as e;
    use hintlib::value::J;
    use hintlib::{jd, jl};

    #[test]
    fn e2_dur_and_labels() {
        let a = J::from("2026-09-01T00:00:00Z");
        assert_eq!(e::_dur(&a, &J::from("2026-09-01T01:02:03Z")).unwrap().as_deref(), Some("1h02m03s"));
        assert_eq!(e::_dur(&J::from("2026-09-01T00:00:59Z"), &a).unwrap().as_deref(), Some("-0m59s"));
        assert_eq!(e::_dur(&a, &J::from("2026-02-30T00:00:00Z")).unwrap(), None);
        assert!(e::_label_is_cell(&J::from("smoke-c1"), "c1").unwrap());
        assert!(!e::_label_is_cell(&J::from("smoke-c1-kv8"), "c1").unwrap());
        assert!(e::_session_is_cell(&J::from("serve-a.b-20260901T000000Z"), "a.b").unwrap());
        assert!(!e::_session_is_cell(&J::from("serve-axb"), "a.b").unwrap());
    }

    #[test]
    fn e2_windows_cross_stream_cut() {
        let rows = jl![
            jl!["2026-01-01T00:00:00Z", 1, jd! {"kind" => "budget_honored", "label" => "A"}],
            jl!["2026-01-01T00:05:00Z", 2, jd! {"kind" => "watchdog_highrate_hold"}]
        ];
        let opens = jl![jl!["2026-01-01T00:00:00Z", "A"], jl!["2026-01-01T00:04:00Z", "B"]];
        let w = e::_event_windows(&rows, &opens).unwrap();
        assert_eq!(w[0]["rows"], jl![0]);
        assert_eq!(w[0]["end_ts"], J::Null);
        assert_eq!(e::_event_windows(&rows, &J::Null).unwrap()[0]["rows"], jl![0, 1]);
    }

    #[test]
    fn e2_fields_and_scope() {
        let f = e::_event_fields(&jd! {"kind" => "budget_declare", "existed" => true, "model" => "1.2.3.4:80", "target" => "a/b"}).unwrap();
        assert_eq!(f, "existed=true");
        let bnd = jd! {"utc" => "2026-01-01T00:00:03Z"};
        let rows = jl![jd! {"utc" => "2026-01-01T00:00:01Z", "detail" => "d"}, jd! {"utc" => J::Null}];
        let out = e::_scope_rows(&rows, &bnd, &J::Null).unwrap();
        assert_eq!(out[0]["campaign_scope"], J::from(e::CAMPAIGN_SCOPE_PRIOR));
        assert!(out[1].get("campaign_scope").is_none());
    }
}

// ── selftest ──
mod selftest {
    use hintlib::evidence as e;

    #[test]
    fn evidence_selftest_is_empty() {
        let bad = e::selftest();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
