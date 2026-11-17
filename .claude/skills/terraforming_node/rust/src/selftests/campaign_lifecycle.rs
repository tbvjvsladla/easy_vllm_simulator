//! Port of `selftest_campaign_lifecycle.py` — the campaign lifecycle states (suspend · resume · abandon · preempt) in a
//! fixture checkout, deterministically (plan_26101719 acceptance groups; plan_26101722 K1~K3, K5, K8).
//!
//!   state      — journey fold (old-instance defaults · orphan · unknown)
//!   transition — the single gate (loophole closed · cooperative refusal · --preempt · teardown failure fail-closed · two-step
//!                topology crossing)
//!   requery    — re-query triggers (a)(d) only · an answered transition is not asked again · N revolving reads ask nothing
//!   guard      — progressive writes refused · tidy writes allowed · no purge while suspended · abandoned shrink gate · resume
//!                gate
//!   guard_relay, propagate, assign — sub propagation, the relay control texts/echo judges and the assignment writer
//!
//! Live runs (a real sub over A2A, a real teardown, a real branch switch) are not this program's scope — the E2E testlog owns
//! them. The fixture can be narrower than the real thing (teardown · docker · git are replaced by injection points).
//!
//! How the original's injection points are met here (the campaign writer is the native `campaign_init` behind
//! `scripts/campaign_init.sh`, driven as a command line; the relay is this crate's library):
//! * `REPO_ROOT`/`CAMPAIGNS` → the launcher's `CAMPAIGN_READ_COMPAT_OWNER` names a program path inside the fixture root, so
//!   the writer's checkout root is the fixture;
//! * `_CHECKOUT_TOPOLOGY_FN` → the fixture has no four-way predicate, so the writer reads `output/<t>/manifest.yaml`; the
//!   fixture's topology is that manifest;
//! * `_SELF_ROLE_FN` → a `self_role: sub` line in that manifest;
//! * `_IMAGE_DIGEST_FN` → a stub `docker` first on `PATH` that answers `image inspect` from the fixture's image table;
//! * `_RELEASE_RUNNER` → stub teardown entry points (`single_serve_down.sh`, `multinode_serve_smoke.sh`) inside the fixture,
//!   recording their arguments and exiting with the fixture's status;
//! * in-process readers (`fold_lifecycle`, `lifecycle_line`, `purge_gate_reasons`, `resume_brief`, `emit_slice`) → the
//!   writer's read commands (`--lifecycle`, `--verify-purge-gate`, `--resume-brief`, `--emit-slice`);
//! * in-process writers the original called directly (`writer_set_phase`, `writer_set_cell`, `writer_add_revision`) → the
//!   writer's own command line; `writer_append_journey` / `record_sub_echo(rev=)` (no command line takes `--rev`) and
//!   `_write_json` → the fixture appends/writes the same JSON documents;
//! * `relay.supervise_decide = lambda ...` → the check-only `relay::dispatch::SUPERVISE_DECIDE_HOOK`.
//! The `_cell_lock_reason(c2) is None` check of a `<<FILL>>` cell is observed as the assignment writer allowing that
//! cell's removal (on c1, since c2 carries a hint approval that refuses removal for another reason).

use super::{dumps, j, read, scripts_dir, stub, Checks, TempDir};
use crate::agent_control::pyhelp::shlex_quote;
use crate::relay::core as relay;
use crate::relay::pv::{self, g, items, py_str, s, set, V};
use std::path::{Path, PathBuf};

const UTC: &str = "2026-10-17T11:00:00Z";
const OWNER_ENV: &str = "CAMPAIGN_READ_COMPAT_OWNER";

struct Fx {
    _tmp: TempDir,
    root: PathBuf,
    bin: PathBuf,
}

fn owner_of(root: &Path) -> PathBuf {
    root.join(".claude/skills/terraforming_node/scripts/campaign_init.sh")
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("copy dir");
    for e in std::fs::read_dir(src).expect("template").flatten() {
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            copy_tree(&s, &d);
        } else {
            std::fs::copy(&s, &d).expect("copy file");
        }
    }
}

/// Lays out one fixture checkout root (campaigns, docs, wiki-desk naming owner, stub teardown entry points).
fn layout(root: &Path, bin: &Path) {
    for d in ["campaigns", "docs/testlog", "docs/devlog", ".claude/skills/terraforming_node/scripts"] {
        std::fs::create_dir_all(root.join(d)).expect("fixture dir");
    }
    // The document naming owner (`doc_naming.ts`, self-contained) the writer asks for PARKED devlog names: a byte copy
    // (a symlink would fail its own "am I the main module" check and answer nothing).
    let wd = super::repo_root().join(".claude/skills/wiki-desk");
    for rel in ["package.json", "src/doc_naming.ts"] {
        let dst = root.join(".claude/skills/wiki-desk").join(rel);
        std::fs::create_dir_all(dst.parent().expect("parent")).expect("wiki-desk dir");
        std::fs::copy(wd.join(rel), &dst).expect("doc_naming copy");
    }
    // The tracked campaign skeleton (`campaigns/_template/**`, README) the writer materialises and validates against — the
    // original read it from the real checkout (its module-level `TEMPLATE`).
    copy_tree(&super::repo_root().join("campaigns/_template"), &root.join("campaigns/_template"));
    let _ = std::fs::copy(super::repo_root().join("campaigns/README.md"), root.join("campaigns/README.md"));
    // The declaration validator asks the schema engine's launcher to be present in the checkout (it validates in-process).
    let gate = ".claude/policies/runtime/completion_gate.sh";
    std::fs::create_dir_all(root.join(gate).parent().expect("parent")).expect("policies dir");
    std::fs::copy(super::repo_root().join(gate), root.join(gate)).expect("completion_gate.sh copy");
    let calls = shlex_quote(&bin.join("release.calls").display().to_string());
    let rc = shlex_quote(&bin.join("release.rc").display().to_string());
    for script in ["single_serve_down.sh", "multinode_serve_smoke.sh"] {
        stub(&root.join(".claude/skills/upstream-version-watch/scripts").join(script),
             &format!("printf '%s\\n' \"$0 $*\" >> {calls}\necho fixture teardown\nexit \"$(cat {rc} 2>/dev/null || echo 0)\"\n"));
    }
}

impl Fx {
    fn new() -> Fx {
        let tmp = TempDir::new("campaign-lifecycle-");
        let root = tmp.path().join("repo");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        layout(&root, &bin);
        let images = shlex_quote(&bin.join("images").display().to_string());
        stub(&bin.join("docker"), &format!(
            "if [ \"$1\" = image ] && [ \"$2\" = inspect ]; then tag=\"${{@: -1}}\"; d=$(awk -v t=\"$tag\" '$1==t{{print $2}}' {images} 2>/dev/null); [ -n \"$d\" ] && {{ echo \"$d\"; exit 0; }}; fi\nexit 1\n"));
        std::fs::write(bin.join("images"), "").expect("images");
        let fx = Fx { _tmp: tmp, root, bin };
        fx.topology("single", false);
        fx
    }
    fn campaigns(&self) -> PathBuf {
        self.root.join("campaigns")
    }
    /// The checkout topology the writer sees (`_CHECKOUT_TOPOLOGY_FN`) and whether this checkout is a sub (`_SELF_ROLE_FN`).
    fn topology(&self, t: &str, sub: bool) {
        let _ = std::fs::remove_dir_all(self.root.join("output"));
        let role = if sub { "self_role: sub\n" } else { "" };
        super::write(&self.root.join("output").join(t).join("manifest.yaml"), &format!("topology: {t}\n{role}"));
    }
    fn image(&self, tag: &str, digest: &str) {
        let path = self.bin.join("images");
        let mut lines: Vec<String> = read(&path).lines().filter(|l| !l.starts_with(&format!("{tag} "))).map(String::from).collect();
        lines.push(format!("{tag} {digest}"));
        std::fs::write(path, lines.join("\n") + "\n").expect("images");
    }
    fn release_rc(&self, rc: i32) {
        std::fs::write(self.bin.join("release.rc"), rc.to_string()).expect("rc");
    }
    fn calls(&self) -> Vec<String> {
        read(&self.bin.join("release.calls")).lines().map(String::from).collect()
    }
    fn clear_calls(&self) {
        let _ = std::fs::remove_file(self.bin.join("release.calls"));
    }

    /// `run(*argv)`: the writer's command line in this fixture → (status, stdout + stderr).
    fn run_in(&self, root: &Path, args: &[&str]) -> (i32, String) {
        let out = std::process::Command::new("bash")
            .arg(scripts_dir().join("campaign_init.sh"))
            .args(args)
            .current_dir(root)
            .env(OWNER_ENV, owner_of(root))
            .env("PATH", super::path_with(&self.bin))
            .stdin(std::process::Stdio::null())
            .output()
            .expect("campaign_init.sh");
        (out.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    }
    fn run(&self, args: &[&str]) -> (i32, String) {
        self.run_in(&self.root, args)
    }

    fn event(kind: &str, extra: &[(&str, V)]) -> V {
        let mut e = pv::obj(vec![("utc", s(UTC)), ("kind", s(kind)), ("lifecycle_v", V::int(1)), ("source", s("campaign_init"))]);
        for (k, v) in extra {
            set(&mut e, k, v.clone());
        }
        e
    }
    fn append(base: &Path, event: &V) {
        use std::io::Write;
        let _ = std::fs::create_dir_all(base);
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(base.join("journey.jsonl")).expect("journey");
        writeln!(f, "{}", dumps(event)).expect("journey line");
    }

    fn camp_in(&self, campaigns: &Path, cid: &str, topology: &str, active: bool, opened: bool, sub: bool) -> PathBuf {
        let base = campaigns.join(cid);
        for d in ["cells", "phases", "sweeps", "relay"] {
            std::fs::create_dir_all(base.join(d)).expect("camp dir");
        }
        let mut nodes = vec![pv::obj(vec![("node_id", s("main")), ("role", s("main")), ("topology", s(topology))])];
        if sub {
            nodes.push(pv::obj(vec![("node_id", s("sub1")), ("role", s("sub")), ("topology", s(topology))]));
        }
        let decl = pv::obj(vec![("schema_version", V::int(1)), ("id", s(cid)), ("plan_ref", s("docs/plan/p.md")), ("nodes", V::List(nodes)),
            ("assignments", j(r#"{"main": [{"cell": "c1"}]}"#)),
            ("control_variables", pv::obj(vec![("model", s("m")), ("vllm_version", s("0.31.0")), ("topology", s(topology))]))]);
        std::fs::write(base.join("campaign.yaml"), dumps(&decl)).expect("decl");
        std::fs::write(base.join("evidence_pointers.json"), dumps(&pv::obj(vec![("campaign_id", s(cid)), ("pointers", V::List(vec![]))]))).expect("ep");
        if opened {
            Fx::append(&base, &Fx::event("campaign_opened", &[("topology", s(topology)), ("approved_by", s("fixture"))]));
        }
        if active {
            std::fs::write(campaigns.join("ACTIVE"), format!("{cid}\n")).expect("ACTIVE");
        }
        base
    }
    fn camp(&self, cid: &str, active: bool, opened: bool, sub: bool) -> PathBuf {
        self.camp_in(&self.campaigns(), cid, "single", active, opened, sub)
    }
    fn set_active(&self, cid: &str) {
        std::fs::write(self.campaigns().join("ACTIVE"), format!("{cid}\n")).expect("ACTIVE");
    }
    fn active(&self) -> String {
        let t = read(&self.campaigns().join("ACTIVE"));
        let t = t.trim();
        if t.is_empty() { "_bootstrap".into() } else { t.into() }
    }
    fn running(&self, cid: &str, cell: &str) -> (i32, String) {
        self.run(&["--campaign-id", cid, "--phase-set", "bench", "--node", "main", "--state", "running", "--proof-predicate", "측정 중",
                   "--proof-source", "fixture", "--cell", cell, "--started-utc", UTC])
    }
    fn testlog(&self, name: &str) -> String {
        std::fs::write(self.root.join("docs/testlog").join(name), "# 포기 요약\n").expect("testlog");
        format!("docs/testlog/{name}")
    }
    fn journey_len(&self, cid: &str) -> usize {
        read(&self.campaigns().join(cid).join("journey.jsonl")).lines().filter(|l| !l.trim().is_empty()).count()
    }
    fn journey(&self, cid: &str) -> Vec<V> {
        read(&self.campaigns().join(cid).join("journey.jsonl")).lines().filter_map(|l| pv::loads(l).ok()).collect()
    }
    fn fold_in(&self, root: &Path, cid: &str) -> V {
        let (rc, out) = self.run_in(root, &["--lifecycle", cid]);
        if rc != 0 {
            return V::None;
        }
        pv::loads(&out).unwrap_or(V::None)
    }
    fn fold(&self, cid: &str) -> V {
        self.fold_in(&self.root, cid)
    }
    fn state(&self, cid: &str) -> String {
        py_str(g(&self.fold(cid), "state"))
    }
    fn line(&self) -> String {
        let (_, out) = self.run(&["--lifecycle"]);
        py_str(g(&pv::loads(&out).unwrap_or(V::None), "line"))
    }
    fn purge_reasons(&self, cid: &str) -> Vec<String> {
        let (_, out) = self.run(&["--verify-purge-gate", cid]);
        out.lines().filter_map(|l| l.strip_prefix("  - ").map(String::from)).collect()
    }
    fn brief_first(&self) -> String {
        let (_, out) = self.run(&["--resume-brief"]);
        out.lines().next().unwrap_or("").to_string()
    }
    fn decl_in(campaigns: &Path, cid: &str) -> V {
        pv::loads(&read(&campaigns.join(cid).join("campaign.yaml"))).unwrap_or(V::None)
    }
    fn decl(&self, cid: &str) -> V {
        Fx::decl_in(&self.campaigns(), cid)
    }
    fn json(path: &Path) -> V {
        pv::loads(&read(path)).unwrap_or(V::None)
    }
}

fn strs(v: &V) -> Vec<String> {
    items(v).iter().map(py_str).collect()
}

fn g_state(c: &mut Checks) {
    let fx = Fx::new();
    c.group("state — journey fold");
    let legacy = fx.camp("L-closed", false, false, false);
    std::fs::write(legacy.join("journey.jsonl"), format!("{}\n", dumps(&pv::obj(vec![("utc", s(UTC)), ("kind", s("campaign_closed"))])))).expect("legacy");
    let f = fx.fold("L-closed");
    c.ck("old campaign_closed line → closed(completed) · no migration event",
         py_str(g(&f, "state")) == "closed" && py_str(g(&f, "close_reason")) == "completed" && fx.journey_len("L-closed") == 1, || dumps(&f));
    fx.camp("L-active", true, false, false);
    c.ck("no events + ACTIVE names it → active", fx.state("L-active") == "active", String::new);
    fx.set_active("_bootstrap");
    c.ck("no events + ACTIVE does not name it → orphan", fx.state("L-active") == "orphan", String::new);
    c.ck("the purge gate guides orphan to abandon", fx.purge_reasons("L-active").iter().any(|r| r.contains("--abandon")), || format!("{:?}", fx.purge_reasons("L-active")));
    let u = fx.camp("U1", false, true, false);
    Fx::append(&u, &pv::obj(vec![("utc", s(UTC)), ("kind", s("campaign_teleported")), ("lifecycle_v", V::int(1))]));
    c.ck("unknown campaign_* event → unknown", fx.state("U1") == "unknown", String::new);
    let u2 = fx.camp("U2", false, true, false);
    Fx::append(&u2, &pv::obj(vec![("utc", s(UTC)), ("kind", s("campaign_suspended")), ("lifecycle_v", V::int(99))]));
    c.ck("future lifecycle_v → unknown (version gap · D15)", fx.state("U2") == "unknown", String::new);
    let tl = fx.testlog("testlog_26101711_포기요약.md");
    let (rc, msg) = fx.run(&["--campaign-id", "U1", "--evidence-add", "--kind", "testlog", "--path", &tl]);
    c.ck("an unknown campaign refuses even tidy writes (rc≠0 · sync guidance)", rc != 0 && msg.contains("동기화"), || msg.clone());
    c.ck("the first line shows unknown as something to tidy", fx.line().contains("U1=unknown"), || fx.line());
    // Sub render tree fallback (2026-10-17 live): a node without the four-way predicate reads the topology from its
    // manifest. Observed through the resume refusal that names the checkout topology.
    let x = fx.camp_in(&fx.campaigns(), "TOPO", "multi", false, true, false);
    Fx::append(&x, &Fx::event("campaign_suspended", &[("approved_by", s("fixture"))]));
    let here = |fx: &Fx| fx.run(&["--resume-campaign", "TOPO", "--approved-by", "fixture", "--utc", UTC, "--apply"]).1;
    let _ = std::fs::remove_dir_all(fx.root.join("output"));
    let m = here(&fx);
    c.ck("no predicate + no manifest → unknown (None · no guess)", m.contains("체크아웃은 None 다"), || m.clone());
    super::write(&fx.root.join("output/single/manifest.yaml"), "topology: single  # 실측\n");
    let m = here(&fx);
    c.ck("no predicate → the topology of the manifest (output/single)", m.contains("체크아웃은 'single' 다"), || m.clone());
    super::write(&fx.root.join("output/multi/manifest.yaml"), "topology: multi\n");
    let m = here(&fx);
    c.ck("two manifests → unknown (None)", m.contains("체크아웃은 None 다"), || m.clone());
}

fn g_transition(c: &mut Checks) {
    let fx = Fx::new();
    c.group("transition — the single gate");
    fx.camp("A", true, true, false);
    let (rc, msg) = fx.run(&["--init", "B", "--plan-ref", "docs/plan/p.md", "--apply"]);
    c.ck("loophole closed: --init without disposition rc≠0 · ACTIVE unchanged · B not created",
         rc != 0 && msg.contains("처분 없이") && fx.active() == "A" && !fx.campaigns().join("B").exists(), || msg.clone());
    c.ck("the refusal names the normal-block class and the three paths (suspend · close · abandon)",
         msg.contains("정상 차단") && msg.contains("--suspend-current") && msg.contains("--close") && msg.contains("--abandon"), || msg.clone());
    let (prc, pmsg) = fx.running("A", "c1");
    c.ck("fixture: running phase recorded", prc == 0, || pmsg.clone());
    let n0 = fx.journey_len("A");
    let (rc, msg) = fx.run(&["--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A", "--release", "cfgA",
                             "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("cooperative refusal: a running cell → rc≠0 · '--preempt' guidance · nothing written",
         rc != 0 && msg.contains("--preempt") && fx.active() == "A" && fx.journey_len("A") == n0 && fx.calls().is_empty(), || msg.clone());
    fx.release_rc(2);
    let (rc, msg) = fx.run(&["--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A", "--preempt", "--release",
                             "cfgA", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("teardown failure → fail-closed: rc≠0 · ACTIVE unchanged · journey unchanged",
         rc != 0 && msg.contains("teardown 실패") && fx.active() == "A" && fx.journey_len("A") == n0, || msg.clone());
    fx.release_rc(0);
    let (rc, msg) = fx.run(&["--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A", "--approved-by", "fixture",
                             "--utc", UTC, "--preempt", "--apply"]);
    c.ck("a missing --release is refused (no silent skip)", rc != 0 && msg.contains("--release") && fx.active() == "A", || msg.clone());
    fx.clear_calls();
    let (rc, msg) = fx.run(&["--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A", "--preempt", "--release",
                             "cfgA", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let (fa, fb) = (fx.fold("A"), fx.fold("B"));
    c.ck("--preempt atomic transition: rc 0 · ACTIVE=B · A suspended · B active(parent=A)",
         rc == 0 && fx.active() == "B" && py_str(g(&fa, "state")) == "suspended" && py_str(g(&fb, "state")) == "active"
             && py_str(g(&fb, "parent")) == "A", || format!("{msg} {}", dumps(&fa)));
    let calls = fx.calls();
    c.ck("the formal teardown entry point was called with the config (single_serve_down.sh cfgA)",
         calls.len() == 1 && calls[0].split(' ').next().is_some_and(|p| p.ends_with("single_serve_down.sh")) && calls[0].ends_with(" cfgA"),
         || format!("{calls:?}"));
    let phase = Fx::json(&fx.campaigns().join("A/phases/main/bench.status.json"));
    c.ck("the interrupted cell c1 stays interrupted and the phase is failed(preempted)",
         strs(g(&fa, "interrupted_cells")) == ["c1"] && py_str(g(&phase, "state")) == "failed", || dumps(&phase));
    let parked = py_str(g(&fa, "parked_devlog"));
    c.ck("one PARKED devlog landed on the docs plane",
         !parked.is_empty() && fx.root.join(&parked).is_file() && read(&fx.root.join(&parked)).contains("PARKED:"), || parked.clone());
    c.ck("B's declaration carries the target topology",
         py_str(g(g(&fx.decl("B"), "control_variables"), "topology")) == "single", String::new);

    c.group("transition — two-step topology crossing");
    fx.camp("X", false, true, false);
    fx.set_active("_bootstrap");
    Fx::append(&fx.campaigns().join("B"), &Fx::event("campaign_closed", &[("reason", s("completed"))]));
    fx.set_active("X");
    let (rc, msg) = fx.run(&["--init", "M", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "X", "--target-topology", "multi",
                             "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--dispose", "A=keep", "--apply"]);
    c.ck("step 1: X suspended · ACTIVE=_bootstrap · M not yet there · opening pending shown",
         rc == 0 && fx.state("X") == "suspended" && fx.active() == "_bootstrap" && !fx.campaigns().join("M").exists()
             && fx.line().contains("개설 대기 M@multi"), || format!("{msg} | {}", fx.line()));
    let (rc, msg) = fx.run(&["--init", "M", "--plan-ref", "docs/plan/p.md", "--utc", UTC, "--apply"]);
    c.ck("step 2 attempted on the other branch (single) → refused (four-way · no misdelivery)",
         rc != 0 && msg.contains("대상 브랜치") && !fx.campaigns().join("M").exists(), || msg.clone());
    fx.topology("multi", false);
    let (rc, msg) = fx.run(&["--init", "M", "--plan-ref", "docs/plan/p.md", "--utc", UTC, "--apply"]);
    let fm = fx.fold("M");
    c.ck("step 2 (multi checkout): M active · parent=X · topology=multi · opening pending cleared",
         rc == 0 && py_str(g(&fm, "state")) == "active" && py_str(g(&fm, "parent")) == "X"
             && py_str(g(g(&fx.decl("M"), "control_variables"), "topology")) == "multi" && !fx.line().contains("개설 대기"), || msg.clone());
    let (rc, msg) = fx.run(&["--resume-campaign", "X", "--approved-by", "fixture", "--utc", UTC, "--accept-drift", "--apply"]);
    c.ck("resuming a single campaign on multi → always refused (not drift · --accept-drift powerless)",
         rc != 0 && msg.contains("drift 가 아니라") && fx.state("X") == "suspended", || msg.clone());
    let tl = fx.testlog("testlog_M.md");
    let (rc, msg) = fx.run(&["--abandon", "M", "--reason", "fixture", "--summary-testlog", &tl, "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    c.ck("abandoning M → a pending resume on X (trigger a · abandoned once too)",
         rc == 0 && fx.state("X") == "resume_pending" && fx.line().contains("X←M 종결(abandoned): 재개?"), || format!("{msg} | {}", fx.line()));
    fx.topology("single", false);
    let (rc, msg) = fx.run(&["--resume-campaign", "X", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let fxx = fx.fold("X");
    let hp = items(g(&fxx, "handoff_pointers")).first().cloned().unwrap_or(V::None);
    c.ck("back on single X resumes · the handoff pointer carries M's close reason",
         rc == 0 && fx.active() == "X" && py_str(g(&fxx, "state")) == "active" && py_str(g(&hp, "child")) == "M"
             && py_str(g(&hp, "child_reason")) == "abandoned", || format!("{msg} {}", dumps(&fxx)));
}

fn g_requery(c: &mut Checks) {
    let fx = Fx::new();
    c.group("requery — triggers (a)(d) only");
    fx.camp("RA", true, true, false);
    fx.run(&["--init", "RB", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "RA", "--release", "none",
             "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let n0 = fx.journey_len("RA");
    let first: Vec<String> = (0..5).map(|_| fx.brief_first()).collect();
    c.ck("revolving 5×: same first line · 0 pending · journey unchanged (0 re-queries)",
         first.iter().all(|l| *l == first[0]) && first[0].contains("미결 질의 0") && fx.journey_len("RA") == n0, || first[0].clone());
    c.ck("first line fixed: resume-brief line 1 is the '상태:' line", fx.brief_first().starts_with("상태: "), || fx.brief_first());
    let tl = fx.testlog("testlog_RB.md");
    let (rc, msg) = fx.run(&["--abandon", "RB", "--reason", "탐색 종료", "--summary-testlog", &tl, "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    let line = fx.line();
    c.ck("a child closing → exactly 1 pending on the parent", rc == 0 && line.matches("RA←RB").count() == 1 && line.contains("미결 질의 1"),
         || format!("{msg} | {line}"));
    let first: Vec<String> = (0..3).map(|_| fx.brief_first()).collect();
    c.ck("unattended revolving: the pending shows only as a line; writes do not grow",
         first.iter().all(|l| *l == first[0]) && fx.journey_len("RA") == n0 + 1, || format!("{} {}", first[0], fx.journey_len("RA")));
    let (rc, msg) = fx.run(&["--answer-query", "RA", "--query-decision", "keep", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("answer keep → suspended · answered · 0 pending", rc == 0 && fx.state("RA") == "suspended" && fx.line().contains("미결 질의 0"), || msg.clone());
    let (rc, msg) = fx.run(&["--answer-query", "RA", "--query-decision", "keep", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("the same answer again is an idempotent no-op", rc == 0 && msg.contains("no-op"), || msg.clone());
    let (rc, msg) = fx.run(&["--init", "RC", "--plan-ref", "docs/plan/p.md", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("unrelated opening (trigger d): already answered RA is not asked again → opens without disposition",
         rc == 0 && fx.state("RC") == "active", || msg.clone());
    let (rc, msg) = fx.run(&["--init", "RD", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    c.ck("RC being suspended now is itself the disposition, so it is not asked", rc == 0 && fx.state("RC") == "suspended", || msg.clone());
    let (rc, msg) = fx.run(&["--init", "RE", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    let tail = msg.split("처분이 필요하다:").last().unwrap_or("").to_string();
    c.ck("only RC (newly suspended after the answer) is asked (not RA) · refused without disposition",
         rc != 0 && tail.contains("RC") && !tail.contains("RA") && fx.state("RD") == "active", || msg.clone());
    let (rc, msg) = fx.run(&["--init", "RE", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none", "--dispose", "RC=keep",
                             "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("transcribed with --dispose RC=keep → opens · RC answered",
         rc == 0 && fx.state("RE") == "active" && pv::truthy(g(&fx.fold("RC"), "answered")), || msg.clone());
    let (rc, msg) = fx.run(&["--init", "RF", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "RE", "--release", "none",
                             "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("a derived opening is not trigger d (RD unanswered is not asked)", rc == 0 && fx.state("RF") == "active", || msg.clone());
}

fn g_guard(c: &mut Checks) {
    let fx = Fx::new();
    c.group("guard — write · purge · resume gates");
    let a = fx.camp("GA", true, true, true);
    super::write(&a.join("cells/c1/cell.status.json"), &dumps(&j(r#"{"schema_version": 1, "cell_id": "c1", "cell_outcome": "measured",
        "image": {"tag": "img:1", "digest": "sha256:aaa"}}"#)));
    fx.image("img:1", "sha256:aaa");
    let (rc, msg) = fx.run(&["--suspend", "GA", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let fa = fx.fold("GA");
    c.ck("standalone suspend · with a sub target 'sub unconverged'",
         rc == 0 && py_str(g(&fa, "state")) == "suspended" && py_str(g(&fa, "sub_convergence")) == "unconverged"
             && fx.line().contains("서브 미수렴 GA@sub1"), || format!("{msg} {}", dumps(&fa)));
    let (rc, msg) = fx.run(&["--sub-echo", "GA", "--node", "sub1", "--control", "suspend", "--utc", UTC, "--apply"]);
    c.ck("sub echo → converged", rc == 0 && py_str(g(&fx.fold("GA"), "sub_convergence")) == "converged", || msg.clone());
    for flag in [vec!["--cell-set", "c1", "--outcome", "pending"], vec!["--phase-set", "serve", "--node", "main", "--state", "running"]] {
        let mut args = vec!["--campaign-id", "GA"];
        args.extend(flag.iter());
        let (rc, msg) = fx.run(&args);
        c.ck(&format!("suspended campaign refuses progressive write {}", flag[0]), rc != 0 && msg.contains("진행성 쓰기"), || msg.clone());
    }
    let tl = fx.testlog("testlog_GA.md");
    let (rc, msg) = fx.run(&["--campaign-id", "GA", "--evidence-add", "--kind", "testlog", "--node", "main", "--path", &tl]);
    c.ck("suspended campaign allows a tidy write (--evidence-add)", rc == 0, || msg.clone());
    c.ck("no purge while suspended", fx.purge_reasons("GA").iter().any(|r| r.contains("보류 상태")), || format!("{:?}", fx.purge_reasons("GA")));
    let tl2 = fx.testlog("testlog_GA2.md");
    let (rc, msg) = fx.run(&["--abandon", "GA", "--reason", "x", "--summary-testlog", &tl2, "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("abandoned shrink gate: measured cell c1 has no evidence pointer → refused", rc != 0 && msg.contains("c1"), || msg.clone());
    fx.image("img:1", "sha256:bbb");
    let (rc, msg) = fx.run(&["--resume-campaign", "GA", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("resume gate: digest drift → refused · --accept-drift guidance", rc != 0 && msg.contains("--accept-drift") && fx.state("GA") == "suspended",
         || msg.clone());
    let (rc, msg) = fx.run(&["--resume-campaign", "GA", "--approved-by", "fixture", "--utc", UTC, "--accept-drift", "--apply"]);
    c.ck("--accept-drift resumes · the sub resume is A2A guidance", rc == 0 && fx.state("GA") == "active" && msg.contains("campaign-control"), || msg.clone());
    // interrupted → pending
    fx.camp("GB", false, true, false);
    fx.set_active("GB");
    let (rc, msg) = fx.run(&["--campaign-id", "GB", "--cell-set", "c1", "--outcome", "measured", "--measurement-decode-tps", "10",
                             "--measurement-source", "fixture", "--utc", UTC]);
    c.ck("fixture: measured cell recorded", rc == 0, || msg.clone());
    fx.running("GB", "c1");
    fx.run(&["--suspend", "GB", "--preempt", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    fx.set_active("_bootstrap");
    let (rc, msg) = fx.run(&["--resume-campaign", "GB", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let cell = Fx::json(&fx.campaigns().join("GB/cells/c1/cell.status.json"));
    c.ck("on resume the interrupted cell goes back to pending",
         rc == 0 && py_str(g(&cell, "cell_outcome")) == "pending" && items(g(&fx.fold("GB"), "interrupted_cells")).is_empty(), || msg.clone());
    let (rc, _) = fx.run(&["--resume-campaign", "GB", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    c.ck("resuming an active campaign again is refused (already active)", rc != 0, String::new);
    let (rc, msg) = fx.run(&["--resume-campaign", "GA", "--utc", UTC, "--apply"]);
    c.ck("a transition without --approved-by is refused (no unapproved transition)", rc != 0 && msg.contains("--approved-by"), || msg.clone());
    // 2026-10-17 live E2: a sub's own instance (self_role=sub) does not count itself as an echo target.
    let sb = fx.camp("GS", true, true, true);
    let mut d = fx.decl("GS");
    set(&mut d, "self_role", s("sub"));
    let subs: Vec<V> = items(g(&d, "nodes")).iter().filter(|n| py_str(g(n, "role")) == "sub").cloned().collect();
    set(&mut d, "nodes", V::List(subs));
    std::fs::write(sb.join("campaign.yaml"), dumps(&d)).expect("decl");
    fx.run(&["--suspend", "GS", "--release", "none", "--approved-by", "main-a2a", "--utc", UTC, "--apply"]);
    // 2026-10-17 live E7: abandoning a two-node campaign attributes the summary evidence to the main node.
    fx.camp("G2", true, true, true);
    let tl = fx.testlog("testlog_G2.md");
    let (rc, msg) = fx.run(&["--abandon", "G2", "--reason", "x", "--summary-testlog", &tl, "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    let ptrs = Fx::json(&fx.campaigns().join("G2/evidence_pointers.json"));
    c.ck("two-node abandon: the summary evidence belongs to main and the campaign closes",
         rc == 0 && fx.state("G2") == "closed" && items(g(&ptrs, "pointers")).iter().any(|p| py_str(g(p, "node_id")) == "main"), || msg.clone());
    // 2026-10-17 live (plan_26101722 orphan sweep): an old sub instance without the self_role key — the manifest identity decides.
    let old_sub = fx.camp("GO", false, true, true);
    let tlgo = fx.testlog("t_GO.md");
    Fx::append(&old_sub, &Fx::event("campaign_closed", &[("reason", s("abandoned")), ("sub_nodes", j(r#"["sub1"]"#)), ("summary_testlog", s(&tlgo))]));
    fx.topology("single", true);
    let fgo = fx.fold("GO");
    let line = fx.line();
    // `_sub_nodes(GO2) == []` observed as the close line a sub checkout writes for a two-node declaration.
    fx.camp("GO2", false, true, true);
    let tl3 = fx.testlog("t_GO2.md");
    let (rc2, m2) = fx.run(&["--abandon", "GO2", "--reason", "x", "--summary-testlog", &tl3, "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    let go2 = fx.fold("GO2");
    c.ck("a sub checkout (manifest self_role=sub) does not wait for its own echo even on an old close line",
         py_str(g(&fgo, "state")) == "closed" && pv::is_none(g(&fgo, "sub_convergence")) && !line.contains("GO@")
             && rc2 == 0 && items(g(&go2, "sub_nodes")).is_empty(), || format!("{} | {line} | {m2} {}", dumps(&fgo), dumps(&go2)));
    fx.topology("single", false);
    c.ck("★negative control: on a main checkout the same line waits for the sub",
         py_str(g(&fx.fold("GO"), "sub_convergence")) == "unconverged", String::new);
    c.ck("suspending a sub's own instance does not create 'sub unconverged'",
         pv::is_none(g(&fx.fold("GS"), "sub_convergence")) && !fx.line().contains("GS@"), String::new);
}

fn capture<T>(f: impl FnOnce() -> T) -> (T, String) {
    relay::CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
    let out = f();
    let text = relay::CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default();
    (out, text)
}

fn decide_hook(v: Option<V>) {
    crate::relay::dispatch::SUPERVISE_DECIDE_HOOK.with(|h| *h.borrow_mut() = v);
}

/// The relay reads the campaign writer through the launcher; it must see this fixture as its checkout.
fn with_owner<T>(root: &Path, f: impl FnOnce() -> T) -> T {
    let saved = std::env::var_os(OWNER_ENV);
    std::env::set_var(OWNER_ENV, owner_of(root));
    let out = f();
    match saved {
        Some(v) => std::env::set_var(OWNER_ENV, v),
        None => std::env::remove_var(OWNER_ENV),
    }
    out
}

fn echo(fields: &str) -> V {
    pv::obj(vec![("campaign_control_echo", j(fields))])
}

fn g_relay(c: &mut Checks) {
    let fx = Fx::new();
    c.group("guard(relay) — supervisor hold · A2A control instructions · echo judge");
    let none = V::None;
    let t = relay::campaign_control_task("SA", "suspend", false, None, &none).unwrap_or_default();
    c.ck("suspend instruction: --suspend · --release · echo shape · no opening (--init)",
         t.contains("--suspend SA") && t.contains("--release") && t.contains("campaign_control_echo") && !t.contains("--init"), || t.clone());
    let r = relay::campaign_control_task("SA", "resume", false, None, &none).unwrap_or_default();
    c.ck("resume instruction: --resume-campaign · no drift bypass", r.contains("--resume-campaign SA") && r.contains("우회하지 말고"), || r.clone());
    let (ok, _) = relay::campaign_control_echo_ok(&echo(r#"{"campaign_id": "SA", "control": "suspend", "state": "suspended"}"#), "SA", "suspend", &none);
    let (bad, why) = relay::campaign_control_echo_ok(&echo(r#"{"campaign_id": "SA", "control": "suspend", "state": "active"}"#), "SA", "suspend", &none);
    let (nothing, _) = relay::campaign_control_echo_ok(&j("{}"), "SA", "suspend", &none);
    c.ck("echo judge: match = converged · state mismatch/absent = unconverged", ok && !bad && why.contains("active") && !nothing, || why.clone());
    relay::set_relay_campaign(Some("SA".into()));
    let rr = relay::relay_root(&fx.root.display().to_string()).unwrap_or_default();
    relay::set_relay_campaign(None);
    c.ck("the lifecycle control ledger lives under the target campaign's relay (not the waiting room even with ACTIVE=_bootstrap)",
         rr.ends_with("campaigns/SA/relay"), || rr.clone());
    let base = fx.camp("SA", true, true, false);
    let led = base.join("relay/ctx-sa.json");
    std::fs::write(&led, dumps(&j(r#"{"context_id": "ctx-sa", "campaign_node": "main", "attempts": [{"x": 1}]}"#))).expect("ledger");
    let args = crate::relay::cli::Args { supervise_step: Some("SA".into()), repo_root: fx.root.display().to_string(), utc: Some(UTC.into()),
                                         ..Default::default() };
    let step = |args: &crate::relay::cli::Args| with_owner(&fx.root, || capture(|| crate::relay::dispatch::supervise_step(args)));
    decide_hook(Some(j(r#"{"action": "next_cell", "reason": "fixture"}"#)));
    let (_, out) = step(&args);
    c.ck("active campaign: the supervisor's forward decision as is (next_cell)", out.contains("next_cell"), || out.clone());
    fx.run(&["--suspend", "SA", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let (_, out) = step(&args);
    let steps = items(g(&Fx::json(&led), "supervisor_steps")).to_vec();
    c.ck("suspended campaign: the supervisor turns the forward decision into hold and writes it to the ledger",
         out.contains("hold") && steps.last().is_some_and(|x| py_str(g(x, "action")) == "hold"), || out.clone());
    decide_hook(Some(j(r#"{"action": "popup", "reason": "처음 보는 종료(fixture)"}"#)));
    let (rc3, out) = step(&args);
    decide_hook(None);
    c.ck("suspended campaign: a popup becomes hold too — not asked again every revolution (live E4)",
         !out.contains("→ popup") && out.contains("→ hold") && matches!(rc3, Ok(0)), || out.clone());
}

fn g_propagate(c: &mut Checks) {
    let fx = Fx::new();
    c.group("propagate — sub propagation of a main close (K1~K3)");
    fx.camp("PA", true, true, true);
    let tl = fx.testlog("testlog_PA.md");
    let (rc, msg) = fx.run(&["--abandon", "PA", "--reason", "x", "--summary-testlog", &tl, "--release", "none", "--approved-by", "fixture",
                             "--utc", UTC, "--apply"]);
    let f = fx.fold("PA");
    c.ck("two-node abandon: the close line names the sub and folds unconverged · relay abandon guidance",
         rc == 0 && strs(g(&f, "sub_nodes")) == ["sub1"] && py_str(g(&f, "sub_convergence")) == "unconverged"
             && msg.contains("--campaign-control abandon"), || msg.clone());
    c.ck("first line has 'sub unconverged PA@sub1'", fx.line().contains("서브 미수렴 PA@sub1"), || fx.line());
    c.ck("purge gate: closed while the sub close is unconverged, naming relay and acceptance",
         fx.purge_reasons("PA").iter().any(|r| r.contains("서브 종결이 수렴하지 않았다") && r.contains("--sub-unconverged-accept")),
         || format!("{:?}", fx.purge_reasons("PA")));
    fx.run(&["--sub-echo", "PA", "--node", "sub1", "--control", "close", "--utc", UTC, "--apply"]);
    c.ck("★negative control: an echo with another reason (close) does not converge an abandon close",
         py_str(g(&fx.fold("PA"), "sub_convergence")) == "unconverged", String::new);
    let (rc, _) = fx.run(&["--sub-echo", "PA", "--node", "sub1", "--control", "abandon", "--utc", UTC, "--apply"]);
    c.ck("abandon echo → converged · the purge gate's sub reason cleared",
         rc == 0 && py_str(g(&fx.fold("PA"), "sub_convergence")) == "converged"
             && !fx.purge_reasons("PA").iter().any(|r| r.contains("서브 종결")) && !fx.line().contains("PA@"), String::new);
    fx.camp("PB", true, true, true);
    let tl = fx.testlog("testlog_PB.md");
    fx.run(&["--abandon", "PB", "--reason", "x", "--summary-testlog", &tl, "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let (rc, msg) = fx.run(&["--sub-unconverged-accept", "PB", "--node", "sub1", "--utc", UTC, "--apply"]);
    c.ck("★negative control: acceptance without a human transcription is refused", rc != 0 && msg.contains("--approved-by"), || msg.clone());
    let (rc, _) = fx.run(&["--sub-unconverged-accept", "PB", "--node", "sub1", "--approved-by", "사람: 서브 미도달 수용", "--utc", UTC, "--apply"]);
    let fb = fx.fold("PB");
    c.ck("human acceptance → converged · separate acceptance record · purge sub reason cleared",
         rc == 0 && py_str(g(&fb, "sub_convergence")) == "converged" && strs(g(&fb, "sub_accepted")) == ["sub1"]
             && !fx.purge_reasons("PB").iter().any(|r| r.contains("서브 종결")), || dumps(&fb));
    fx.camp("PC", true, true, true);
    let (rc, _) = fx.run(&["--sub-unconverged-accept", "PC", "--node", "sub1", "--approved-by", "x", "--utc", UTC, "--apply"]);
    c.ck("★negative control: nothing to accept on an active campaign (suspend/resume unconverged is not a block)", rc != 0, String::new);
    fx.set_active("_bootstrap");
    let old = fx.camp("PD", false, false, true);
    let tlpd = fx.testlog("t_PD.md");
    std::fs::write(old.join("journey.jsonl"), format!("{}\n", dumps(&pv::obj(vec![("utc", s(UTC)), ("kind", s("campaign_closed")),
        ("reason", s("abandoned")), ("lifecycle_v", V::int(1)), ("summary_testlog", s(&tlpd))])))).expect("journey");
    c.ck("an old close (no sub_nodes) is exempt from the propagation condition — no retroactive block",
         pv::is_none(g(&fx.fold("PD"), "sub_convergence")) && !fx.purge_reasons("PD").iter().any(|r| r.contains("서브 종결")), String::new);
    let none = V::None;
    let tc = relay::campaign_control_task("PA", "close", false, None, &none).unwrap_or_default();
    let ta = relay::campaign_control_task("PA", "abandon", false, Some("모드3 탐색으로 전환"), &none).unwrap_or_default();
    c.ck("relay instruction: close=--close · abandon=--abandon+reason+summary testlog · close_reason in the echo",
         tc.contains("--close PA") && ta.contains("--abandon PA") && ta.contains("모드3 탐색으로 전환") && ta.contains("--summary-testlog")
             && ta.contains("close_reason") && !ta.contains("--init"), || ta.clone());
    let (ok, _) = relay::campaign_control_echo_ok(&echo(r#"{"campaign_id": "PA", "control": "abandon", "state": "closed", "close_reason": "abandoned"}"#),
                                                  "PA", "abandon", &none);
    let (bad, why) = relay::campaign_control_echo_ok(&echo(r#"{"campaign_id": "PA", "control": "abandon", "state": "closed", "close_reason": "completed"}"#),
                                                     "PA", "abandon", &none);
    c.ck("relay echo judge: the same closed with another reason is unconverged", ok && !bad && why.contains("사유"), || why.clone());
    let ids = vec!["o1".to_string(), "o2".to_string()];
    let ts = relay::orphan_sweep_task(&ids, "옛 허점 잔재");
    c.ck("orphan sweep instruction: --abandon for every target · one summary testlog · nothing outside the list",
         ts.contains("--abandon o1") && ts.contains("--abandon o2") && ts.contains("목록 밖") && !ts.contains("--init"), || ts.clone());
    let sweep = |res: &str| echo(&format!(r#"{{"control": "orphan_sweep", "summary_testlog": "t.md", "results": {res}}}"#));
    let (ok, _) = relay::orphan_sweep_echo_ok(&sweep(r#"[{"campaign_id": "o1", "state": "closed", "close_reason": "abandoned"},
        {"campaign_id": "o2", "state": "closed", "close_reason": "abandoned"}]"#), &ids);
    let (part, why) = relay::orphan_sweep_echo_ok(&sweep(r#"[{"campaign_id": "o1", "state": "closed", "close_reason": "abandoned"},
        {"campaign_id": "o2", "state": "orphan", "refusal": "게이트"}]"#), &ids);
    let (extra, why2) = relay::orphan_sweep_echo_ok(&sweep(r#"[{"campaign_id": "o1", "state": "closed", "close_reason": "abandoned"},
        {"campaign_id": "o2", "state": "closed", "close_reason": "abandoned"}, {"campaign_id": "o3", "state": "closed", "close_reason": "abandoned"}]"#), &ids);
    c.ck("★orphan sweep echo: all abandoned = converged · partial refusal = untidy(reason) · touching outside the list = refused",
         ok && !part && why.contains("o2") && why.contains("게이트") && !extra && why2.contains("o3"), || format!("{why} | {why2}"));
}

fn g_assign(c: &mut Checks) {
    let fx = Fx::new();
    c.group("assign — assignment writer · reslice (K8)");
    let base = fx.camp("AS", true, true, true);
    let mut d = fx.decl("AS");
    set(&mut d, "declared_utc", s(UTC));
    set(&mut d, "assignments", j(r#"{"main": [{"cell": "c1"}, {"cell": "c2"}], "sub1": [{"cell": "s1"}]}"#));
    set(&mut d, "hint_targets", j(&format!(r#"[{{"node_id": "main", "cells": ["c2"], "approval": {{"approved_by": "사람", "utc": "{UTC}"}}}}]"#)));
    std::fs::write(base.join("campaign.yaml"), dumps(&d)).expect("decl");
    let four = ["--assign-reason", "모드3 조건 추가", "--assign-evidence", "docs/plan/p.md", "--assign-approved-by", "사람: 승인", "--utc", UTC];
    let assign = |node: &str, cells: &str, extra: &[&str]| {
        let mut a = vec!["--assign-set", node, "--assign-cells", cells];
        a.extend(extra.iter());
        fx.run(&a)
    };
    let (rc, msg) = assign("main", "c2,c1:HITL,c3", &["--assign-reason", "x", "--utc", UTC]);
    c.ck("★negative control: missing the four fields is refused", rc != 0 && msg.contains("필수"), || msg.clone());
    let (rc, msg) = assign("main", "c2,c1:HITL,c3", &four);
    let d2 = fx.decl("AS");
    let cells: Vec<String> = items(g(g(&d2, "assignments"), "main")).iter().map(|i| py_str(g(i, "cell"))).collect();
    let revs = items(g(&d2, "assignment_revisions")).to_vec();
    c.ck("adding a pending cell · order · mode change → assignments updated · revisions[0]=t0 · one journey line",
         rc == 0 && cells == ["c2", "c1", "c3"] && revs.len() >= 2 && py_str(g(&revs[0], "approved_by")) == "t0"
             && strs(g(&revs[1], "added")) == ["c3"]
             && fx.journey("AS").iter().any(|e| py_str(g(e, "kind")) == "campaign_assignments_revised") && fx.state("AS") == "active",
         || msg.clone());
    super::write(&base.join("cells/c2/cell.status.json"), &dumps(&pv::obj(vec![("cell_outcome", s("<<FILL>>"))])));
    // `_cell_lock_reason(..) is None` for a template-value cell, observed as the writer letting it go (c1: no hint approval).
    super::write(&base.join("cells/c1/cell.status.json"), &dumps(&pv::obj(vec![("cell_outcome", s("<<FILL>>"))])));
    let (rc1, m1) = assign("main", "c2,c3", &four);
    let (rc2, m2) = assign("main", "c2,c1:HITL,c3", &four);
    c.ck("a template-value (<<FILL>>) cell right after materialize is not locked (live L5)", rc1 == 0 && rc2 == 0, || format!("{m1} | {m2}"));
    fx.run(&["--campaign-id", "AS", "--phase-set", "bench", "--node", "main", "--state", "done", "--proof-predicate", "x", "--proof-ok",
             "--proof-source", "fixture", "--cell", "c3", "--started-utc", UTC, "--ended-utc", UTC]);
    let (rc, msg) = assign("main", "c2,c1:HITL", &four);
    c.ck("★negative control: removing a done cell is refused (the reason names the phase)", rc != 0 && msg.contains("c3") && msg.contains("pending"),
         || msg.clone());
    let (rc, msg) = assign("main", "c1:HITL,c3", &four);
    c.ck("★negative control: removing a hint-approved cell is refused", rc != 0 && msg.contains("hint"), || msg.clone());
    let (rc, msg) = assign("main", "c2,c1:HITL,c3,s1", &four);
    c.ck("★negative control: putting another node's cell is refused (a move = remove + add)", rc != 0 && msg.contains("sub1"), || msg.clone());
    let (rc, msg) = assign("sub1", "s1,s2", &four);
    let f = fx.fold("AS");
    let pending = g(g(&f, "reslice_pending"), "sub1").clone();
    c.ck("reassigning the sub's share → reslice unconverged · first line · relay reslice guidance",
         rc == 0 && !pv::is_none(&pending) && fx.line().contains("재배정 미수렴 AS@sub1") && msg.contains("--campaign-control reslice"),
         || format!("{msg} | {}", dumps(&f)));
    let (_, sl_text) = fx.run(&["--emit-slice", "sub1", "--campaign-id", "AS", "--utc", UTC]);
    let sl = pv::loads(&sl_text).unwrap_or(V::None);
    c.ck("the derived declaration carries the reassignment meta", pv::eq(g(g(&sl, "_assignment_revision"), "rev"), &pending), || sl_text.clone());
    let (rc, _) = fx.run(&["--sub-echo", "AS", "--node", "sub1", "--control", "reslice", "--utc", UTC, "--apply"]);
    c.ck("★negative control: a reslice echo without rev is refused", rc != 0, String::new);
    Fx::append(&base, &Fx::event("sub_control_echo", &[("node", s("sub1")), ("control", s("reslice")), ("rev", pending.clone())]));
    c.ck("reslice echo (rev matches) → reslice unconverged cleared",
         !pv::truthy(g(&fx.fold("AS"), "reslice_pending")) && !fx.line().contains("AS@"), || dumps(&fx.fold("AS")));
    let t = relay::campaign_control_task("AS", "reslice", false, None, &sl).unwrap_or_default();
    let rev = g(g(&sl, "_assignment_revision"), "rev").clone();
    let mk = |cells: &str| {
        let mut e = j(&format!(r#"{{"campaign_id": "AS", "control": "reslice", "state": "active", "assigned_cells": {cells}}}"#));
        set(&mut e, "rev", rev.clone());
        pv::obj(vec![("campaign_control_echo", e)])
    };
    let (ok, _) = relay::campaign_control_echo_ok(&mk(r#"["s1", "s2"]"#), "AS", "reslice", &sl);
    let (bad, _) = relay::campaign_control_echo_ok(&mk(r#"["s1"]"#), "AS", "reslice", &sl);
    c.ck("relay reslice: the instruction carries the declaration and --slice-update - · the echo checks cells and rev",
         t.contains("--slice-update -") && t.contains("\"s2\"") && ok && !bad, || t.clone());

    // The sub side — its own instance (same id) in another checkout.
    let sub_root = fx.root.parent().expect("tmp").join("subrepo");
    layout(&sub_root, &fx.bin);
    super::write(&sub_root.join("output/single/manifest.yaml"), "topology: single\n");
    let sub_campaigns = sub_root.join("campaigns");
    let sbase = sub_campaigns.join("AS");
    std::fs::create_dir_all(sbase.join("cells")).expect("sub cells");
    let mut own = V::Dict(Default::default());
    for (k, v) in pv::entries(&sl) {
        if !k.starts_with('_') {
            set(&mut own, &k, v.clone());
        }
    }
    set(&mut own, "assignments", j(r#"{"sub1": [{"cell": "s1"}]}"#));
    std::fs::write(sbase.join("campaign.yaml"), dumps(&own)).expect("sub decl");
    std::fs::write(sbase.join("evidence_pointers.json"), dumps(&j(r#"{"pointers": []}"#))).expect("sub ep");
    Fx::append(&sbase, &Fx::event("campaign_opened", &[("topology", s("single")), ("approved_by", s("main-a2a"))]));
    std::fs::write(sub_campaigns.join("ACTIVE"), "AS\n").expect("sub ACTIVE");
    let sp = fx.root.parent().expect("tmp").join("slice.json");
    let sps = sp.display().to_string();
    std::fs::write(&sp, dumps(&sl)).expect("slice");
    let (rc, msg) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC, "--apply"]);
    let sd = Fx::decl_in(&sub_campaigns, "AS");
    let scells: Vec<String> = items(g(g(&sd, "assignments"), "sub1")).iter().map(|i| py_str(g(i, "cell"))).collect();
    let last_rev = items(g(&sd, "assignment_revisions")).last().map(|r| py_str(g(r, "approved_by"))).unwrap_or_default();
    c.ck("sub --slice-update → its own assignment equals the main declaration · approval transcribed as main-a2a",
         rc == 0 && scells == ["s1", "s2"] && last_rev.starts_with("main-a2a"), || msg.clone());
    // A layer-1-only revision must not touch assignment history or hint approval.
    let mut before = Fx::decl_in(&sub_campaigns, "AS");
    set(&mut before, "hint_targets", j(&format!(r#"[{{"node_id": "sub1", "cells": ["s1"], "approval": {{
        "approved_by": "user fixture", "approved_utc": "{UTC}", "source": "declaration-popup"}}}}]"#)));
    std::fs::write(sbase.join("campaign.yaml"), dumps(&before)).expect("sub decl");
    let (rc, msg) = fx.run(&["--campaign-id", "AS", "--revise", "--revise-set", "vllm_version=0.32.0", "--revise-reason", "user requested latest",
                             "--revise-evidence", "fixture release", "--revise-approved-by", "user: latest", "--utc", UTC]);
    c.ck("fixture: main control revision recorded", rc == 0, || msg.clone());
    let (_, rv_text) = fx.run(&["--emit-slice", "sub1", "--campaign-id", "AS", "--utc", UTC]);
    let revised = pv::loads(&rv_text).unwrap_or(V::None);
    std::fs::write(&sp, dumps(&revised)).expect("slice");
    let old_bytes = std::fs::read(sbase.join("campaign.yaml")).unwrap_or_default();
    let old_journey = std::fs::read(sbase.join("journey.jsonl")).unwrap_or_default();
    let (rc, msg) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC]);
    c.ck("control revision dry run does not write", rc == 0 && std::fs::read(sbase.join("campaign.yaml")).unwrap_or_default() == old_bytes, || msg.clone());
    let (rc, msg) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC, "--apply"]);
    let updated = Fx::decl_in(&sub_campaigns, "AS");
    c.ck("main control revision imports exact history; existing assignment and approval untouched",
         rc == 0 && pv::eq(g(&updated, "revisions"), g(&revised, "revisions"))
             && pv::eq(g(&updated, "control_variables"), g(&before, "control_variables"))
             && pv::eq(g(&updated, "assignments"), g(&before, "assignments"))
             && pv::eq(g(&updated, "assignment_revisions"), g(&before, "assignment_revisions"))
             && pv::eq(g(&updated, "hint_targets"), g(&before, "hint_targets"))
             && std::fs::read(sbase.join("journey.jsonl")).unwrap_or_default() == old_journey, || msg.clone());
    let applied = std::fs::read(sbase.join("campaign.yaml")).unwrap_or_default();
    let (rc, msg) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC, "--apply"]);
    c.ck("control revision reimport is idempotent", rc == 0 && std::fs::read(sbase.join("campaign.yaml")).unwrap_or_default() == applied, || msg.clone());
    type Mutate = fn(&mut V);
    let mutations: [(&str, Mutate); 6] = [
        ("downgrade", |d| set(d, "revisions", V::List(vec![]))),
        ("conflicting prefix", |d| { if let Some(V::List(l)) = pv::get_mut(d, "revisions") { set(&mut l[1], "values", j(r#"{"vllm_version": "bad"}"#)); } }),
        ("missing approval", |d| { if let Some(V::List(l)) = pv::get_mut(d, "revisions") { set(&mut l[1], "approved_by", s("")); } }),
        ("wrong main", |d| set(d, "derived_from", s("other"))),
        ("changed t0", |d| { if let Some(cv) = pv::get_mut(d, "control_variables") { set(cv, "vllm_version", s("bad")); } }),
        ("wrong node role", |d| { if let Some(V::List(l)) = pv::get_mut(d, "nodes") { set(&mut l[0], "role", s("main")); } }),
    ];
    for (label, mutate) in mutations {
        let mut bad = revised.clone();
        mutate(&mut bad);
        std::fs::write(&sp, dumps(&bad)).expect("slice");
        let (rc, _) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC, "--apply"]);
        c.ck(&format!("revision reject {label}"), rc != 0 && std::fs::read(sbase.join("campaign.yaml")).unwrap_or_default() == applied, String::new);
    }
    std::fs::write(&sp, dumps(&revised)).expect("slice");
    Fx::append(&sbase, &Fx::event("campaign_suspended", &[("approved_by", s("x"))]));
    let (rc, msg) = fx.run_in(&sub_root, &["--slice-update", &sps, "--utc", UTC, "--apply"]);
    c.ck("★negative control: a sub reassignment while suspended is refused (progressive write)", rc != 0 && msg.contains("suspended"), || msg.clone());
    let (rc, msg) = fx.run(&["--slice-update", &sps, "--utc", UTC, "--apply"]);
    c.ck("★negative control: slice-update on the main instance is refused (main uses --assign-set)",
         rc != 0 && msg.contains("서브 자기 인스턴스가 아니다"), || msg.clone());
    fx.run(&["--suspend", "AS", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply"]);
    let mut a = vec!["--assign-set", "main", "--assign-cells", "c2,c1:HITL,c3", "--campaign-id", "AS"];
    a.extend(four.iter());
    let (rc, _) = fx.run(&a);
    c.ck("★negative control: reassigning a suspended campaign is refused", rc != 0, String::new);
    // K5: phase-set with only --cell derives the node from the assignments (lite/sweep runner closing part).
    let pb = fx.camp("PH", true, true, false);
    let (rc, _) = fx.run(&["--phase-set", "bench", "--cell", "c1", "--state", "running", "--started-utc", UTC]);
    let st = Fx::json(&pb.join("phases/main/bench.status.json"));
    c.ck("phase-set --cell (node omitted) → recorded under the assigned node",
         rc == 0 && py_str(g(&st, "state")) == "running" && py_str(g(&st, "cell_id")) == "c1", || dumps(&st));
    let (rc, msg) = fx.run(&["--phase-set", "bench", "--cell", "zz", "--state", "running"]);
    c.ck("★negative control: a cell outside the assignments cannot derive a node and is refused", rc != 0 && msg.contains("assignments"),
         || msg.clone());
}

type Group = fn(&mut Checks);

pub fn run() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let pick = argv.iter().position(|a| a == "-k").and_then(|i| argv.get(i + 1)).cloned();
    let groups: [(&str, Group); 7] = [("state", g_state), ("transition", g_transition), ("requery", g_requery), ("guard", g_guard),
        ("guard_relay", g_relay), ("propagate", g_propagate), ("assign", g_assign)];
    let mut c = Checks::new("selftest_campaign_lifecycle");
    for (name, f) in groups {
        if pick.as_deref().is_some_and(|p| !name.contains(p)) {
            continue;
        }
        f(&mut c);
    }
    c.finish()
}
