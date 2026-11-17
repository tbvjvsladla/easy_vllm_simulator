//! The CLI layer (E1-E17) of `scripts/selftest_gmu_roles_batch.py`, over the installed Rust `recipe` program: gmu's two roles
//! (`gate_margin` = safety_margin, `deploy_gmu` = target_gpu.target_gmu), the max-num-seqs rule (`min(concurrency_requirement,
//! KV_fit@typical_request_tokens)` derived and re-verified by the two-phase trial) and the machine stamp of the converged lockset
//! (`provenance` and the `*_source` knobs, checked against the vocabulary owner the program itself reads).
//!
//! Each case runs `recipe simulate --mock-profile` from `main()`'s Flag gate to the three-file set and the lockset write, inside a
//! throwaway copy of the skill (the copy is the deployed bytes, at the same relative paths, under a fresh `git init`), so live
//! `output/**`, `docs/simlog/**` and `campaigns/**` are never touched. The mock profile is the test plane's isolation of the
//! engine (docker/NAS/GPU are not needed) and every converged lockset must say so itself (`trial_provenance=mock`,
//! `measured=false`): a mock never passes for a measurement.
//!
//! The other two layers of the original: the U layer (function units) lives under its owner
//! (`recipe_cmd::simulate_body::gmu_units`, re-exported as `recipe_cmd::gmu_unit_layer`); the F layer ([`engine_layer`]) runs the
//! simulate command in process with a deterministic fake engine through the check-only seam `recipe_cmd::simulate_with_engine`
//! (the original monkeypatched `run_trial`, `preload_ram_gate` and `feedback_append` of an imported `recipe.py`).

use crate::observe::same_value;
use crate::recipe_cmd::lockset_vocabulary;
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const GIB: f64 = 1073741824.0;
/// `estimate_vram.KV_BLOCK_ALIGN_BUFFER`; the self-test cross-checks it against the owner before computing expectations.
const BUFFER: f64 = 1.02;
const MAX_LEN: i64 = 32768;
const L_TYP: i64 = 8192;
const SAFETY: f64 = 0.90;
const TARGET_GMU: f64 = 0.85;
const SKILL_REL: &str = ".claude/skills/vllm-recipe-explorer";
/// What the copy carries besides the skill: the lockset vocabulary owner and the GPU specification table the program reads.
const EXTRA_COPIES: [&str; 2] = [
    ".claude/skills/adversarial-benchmark/rust/src/campaign_validator.rs",
    ".claude/skills/wiki-desk/reference/references.md",
];
/// Untracked local products inside the skill (`.gitignore`) and build output: the copy never carries an operator's inputs.
const SKIP_NAMES: [&str; 4] = ["__pycache__", "feedback", "config.yaml", "target"];
const MANIFEST: &str = "topology: single\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nmodel_source: managed\nterraforming:\n  complete: true\n  branch_verified: true\n";
const MODEL_CONFIG: &str = r#"{"architectures": ["LlamaForCausalLM"], "model_type": "llama", "hidden_size": 4096, "num_hidden_layers": 32, "num_attention_heads": 32, "num_key_value_heads": 8, "head_dim": 128, "max_position_embeddings": 131072, "torch_dtype": "bfloat16", "vocab_size": 128256}"#;
/// Near the deployment ceiling (96 GiB x 0.85 - 10 - 2 = 69.6 GiB): engine tokens 700,000 exceed the phase-1 KV-fit (about
/// 682k), so phase 2 passes.
const PROFILE_NEAR: &str = r#"{"vllm_profile": {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 70.0, "kv_cache_tokens": 700000, "max_concurrency": 21.36}, "functional": {"passed": true}}"#;
/// weights+overhead 84 GiB: passes the gate ceiling (96 x 0.90 = 86.4) and exceeds the clamp ceiling (96 x 0.85 = 81.6).
const PROFILE_HEAVY: &str = r#"{"vllm_profile": {"weights_gib": 70.0, "non_kv_overhead_gib": 14.0, "kv_cache_gib": 1.0, "kv_cache_tokens": 1000, "max_concurrency": 0.03}, "functional": {"passed": true}}"#;
/// The same per-token as NEAR with a smaller measured pool: engine tokens 400,000 < the phase-1 KV-fit.
const PROFILE_SHORT: &str = r#"{"vllm_profile": {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 40.0, "kv_cache_tokens": 400000, "max_concurrency": 12.2}, "functional": {"passed": true}}"#;
const TARGET: &str = r#"{"gpu_model": "NVIDIA RTX PRO 6000", "per_card_vram_gib": 96, "target_gmu": 0.85, "cards_per_node": 1}"#;
const TARGET_NO_GMU: &str =
    r#"{"gpu_model": "NVIDIA RTX PRO 6000", "per_card_vram_gib": 96, "cards_per_node": 1}"#;

/// The self-test's report: one line per check, in order.
#[derive(Default)]
pub struct Report {
    pub lines: Vec<String>,
    pub failures: Vec<String>,
}

impl Report {
    pub fn ck(&mut self, name: &str, cond: bool, detail: impl FnOnce() -> String) {
        if cond {
            self.lines.push(format!("  [PASS] {name}"));
        } else {
            self.lines.push(format!("  [FAIL] {name}  → {}", detail()));
            self.failures.push(name.to_string());
        }
    }
}

// ── JSON value helpers (Python `dict.get` semantics: a missing key or a non-object is None) ───────────────────────────

fn at(v: &PyValue, key: &str) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn has(v: &PyValue, key: &str) -> bool {
    matches!(v.peel(), PyValue::Dict(d) if d.contains_key(key))
}

fn none(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::None)
}

fn num(v: &PyValue) -> Option<f64> {
    match v.peel() {
        PyValue::Int(n) => n.to_string().parse().ok(),
        PyValue::Float(f) => Some(*f),
        _ => None,
    }
}

fn int(v: &PyValue) -> Option<i64> {
    match v.peel() {
        PyValue::Int(n) => i64::try_from(n.clone()).ok(),
        _ => None,
    }
}

fn eq_num(v: &PyValue, want: f64) -> bool {
    num(v) == Some(want)
}

fn text(v: &PyValue) -> Option<String> {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}

fn eq_text(v: &PyValue, want: &str) -> bool {
    text(v).as_deref() == Some(want)
}

fn contains(v: &PyValue, needle: &str) -> bool {
    text(v).is_some_and(|t| t.contains(needle))
}

fn items(v: &PyValue) -> Vec<PyValue> {
    match v.peel() {
        PyValue::List(x) | PyValue::Tuple(x) => x.clone(),
        _ => Vec::new(),
    }
}

/// `str(value)` of an int/None (what the generated yaml prints for the clamp).
fn str_of(v: &PyValue) -> String {
    v.display_text().to_utf8().unwrap_or_default()
}

fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// An ordered JSON object from `(key, JSON text)` pairs; `update` replaces in place or appends (`dict.update`), and a value of
/// `null` set by an update drops the key (the original's `{k: v ... if v is not None}` for the config).
#[derive(Clone)]
struct Obj(Vec<(String, String)>);

impl Obj {
    fn update(mut self, extra: &[(&str, String)]) -> Obj {
        for (k, v) in extra {
            match self.0.iter_mut().find(|(key, _)| key == k) {
                Some(slot) => slot.1 = v.clone(),
                None => self.0.push((k.to_string(), v.clone())),
            }
        }
        self
    }
    fn without_nulls(mut self) -> Obj {
        self.0.retain(|(_, v)| v != "null");
        self
    }
    fn json(&self) -> String {
        let body: Vec<String> = self
            .0
            .iter()
            .map(|(k, v)| format!("{}: {}", js(k), v))
            .collect();
        format!("{{{}}}", body.join(", "))
    }
}

fn load_json(path: &Path) -> Option<PyValue> {
    let bytes = std::fs::read(path).ok()?;
    loads_text(&PyStr::from_surrogateescape(&bytes)).ok()
}

fn yaml_value(yaml: Option<&str>, key: &str) -> Option<String> {
    yaml?.lines().find_map(|line| {
        line.strip_prefix(&format!("{key}:"))
            .map(|rest| rest.trim().to_string())
    })
}

// ── fixture ─────────────────────────────────────────────────────────────────────────────────────────────────────────

struct Fixture {
    tmp: PathBuf,
    root: PathBuf,
    recipe: PathBuf,
}

struct Out {
    rc: i32,
    stderr: String,
    lockset: PyValue,
    input_lock: PyValue,
    summary: Option<PyValue>,
    yaml: Option<String>,
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if SKIP_NAMES.contains(&name_text.as_ref()) {
            continue;
        }
        let kind = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(&name);
        if kind.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else if kind.is_file() {
            if name_text.ends_with(".pyc")
                || (name_text.starts_with("lockset") && name_text.ends_with(".json"))
            {
                continue;
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

impl Fixture {
    fn new(tmp: &Path, repo: &Path, recipe: &Path) -> std::io::Result<Fixture> {
        let root = tmp.join("repo");
        let skill = root.join(SKILL_REL);
        std::fs::create_dir_all(&skill)?;
        copy_tree(&repo.join(SKILL_REL), &skill)?;
        for rel in EXTRA_COPIES {
            let to = root.join(rel);
            std::fs::create_dir_all(to.parent().expect("a parent"))?;
            std::fs::copy(repo.join(rel), &to)?;
        }
        // The copy's repository root comes from git (it never leaks into the outer repository); without git the path fallback
        // names the same root.
        let _ = Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&root)
            .output();
        std::fs::create_dir_all(root.join("output/single"))?;
        std::fs::write(root.join("output/single/manifest.yaml"), MANIFEST)?;
        let model = tmp.join("nas/fx-org/fx-model");
        std::fs::create_dir_all(&model)?;
        std::fs::write(model.join("config.json"), MODEL_CONFIG)?;
        // plan_26111708 B L3: the same shape as a hybrid (linear-attention) model — the parser marks it `is_hybrid`.
        let hybrid = tmp.join("nas/fx-org/fx-hybrid");
        std::fs::create_dir_all(&hybrid)?;
        let layer_types: Vec<&str> = (0..32).map(|i| if i % 4 == 3 { "\"full_attention\"" } else { "\"linear_attention\"" }).collect();
        std::fs::write(
            hybrid.join("config.json"),
            MODEL_CONFIG.replacen('}', &format!(", \"layer_types\": [{}]}}", layer_types.join(", ")), 1),
        )?;
        for (name, blob) in [
            ("near", PROFILE_NEAR),
            ("short", PROFILE_SHORT),
            ("heavy", PROFILE_HEAVY),
        ] {
            std::fs::write(tmp.join(format!("profile_{name}.json")), blob)?;
        }
        Ok(Fixture {
            tmp: tmp.to_path_buf(),
            root,
            recipe: recipe.to_path_buf(),
        })
    }

    fn base_lock(name: &str) -> Obj {
        Obj(vec![
            ("id".into(), js(&format!("fx-{name}"))),
            ("quantization".into(), js("native")),
            ("max_model_len".into(), MAX_LEN.to_string()),
            ("batch".into(), "null".into()),
            ("kv_cache_quant".into(), "null".into()),
            ("kv_cache_memory_bytes".into(), "null".into()),
            ("attention_backend".into(), "null".into()),
            ("attention_backend_candidates".into(), "[]".into()),
            ("tool_call_parser".into(), "null".into()),
            ("tool_call_parser_candidates".into(), "[]".into()),
            ("reasoning_parser".into(), "null".into()),
            ("reasoning_parser_candidates".into(), "[]".into()),
            ("model_capabilities".into(), "{}".into()),
        ])
    }

    /// One `recipe simulate --mock-profile` run. The config is written as JSON (a YAML document the program's YAML reader loads).
    #[allow(clippy::too_many_arguments)]
    fn run_case(
        &self,
        name: &str,
        cfg_extra: &[(&str, String)],
        lockset: &[(&str, String)],
        profile: &str,
        target: bool,
        lock_path: Option<PathBuf>,
        lockset_out: bool,
    ) -> Out {
        let case = self.tmp.join("cases").join(name);
        std::fs::create_dir_all(&case).expect("a case directory");
        let mut cfg = Obj(vec![
            (
                "target_model".into(),
                r#"{"path": "/app/models/fx-org/fx-model"}"#.into(),
            ),
            (
                "nas_host_root".into(),
                js(&self.tmp.join("nas").to_string_lossy()),
            ),
            ("vram_budget_gb".into(), "96".into()),
            ("safety_margin".into(), SAFETY.to_string()),
            ("kv_cache_dtype_bytes".into(), "2".into()),
            (
                "serving".into(),
                format!(
                    r#"{{"config_name": {}, "port": 18080, "served_model_name": "fx"}}"#,
                    js(&format!("fx-{name}"))
                ),
            ),
        ]);
        if target {
            cfg = cfg.update(&[("target_gpu", TARGET.to_string())]);
        }
        let cfg = cfg.update(cfg_extra).without_nulls();
        let cfg_path = case.join("config.yaml");
        std::fs::write(&cfg_path, cfg.json()).expect("write the config");
        let base = Self::base_lock(name).update(lockset);
        let lk = lock_path.unwrap_or_else(|| case.join("lockset.json"));
        std::fs::create_dir_all(lk.parent().expect("a parent")).expect("the lockset directory");
        std::fs::write(&lk, base.json()).expect("write the lockset");
        let input_lock = load_json(&lk).expect("the input lockset");
        let mut command = Command::new(&self.recipe);
        // argv[0] names the copy's installed program, as the launcher does: the program's skill root is the copy.
        command.arg0(
            self.root
                .join(SKILL_REL)
                .join("rust/runtime/target/release/recipe"),
        );
        command
            .arg("simulate")
            .arg("--config")
            .arg(&cfg_path)
            .arg("--candidate")
            .arg(&lk)
            .arg("--mock-profile")
            .arg(self.tmp.join(format!("profile_{profile}.json")))
            .arg("--run-id")
            .arg(format!("fx_{name}"));
        if lockset_out {
            command.arg("--lockset-out").arg(&lk);
        }
        command
            .current_dir(&self.root)
            .env_remove("EASY_VLLM_RECIPE_RUNTIME_ROOT");
        let output = command.output().expect("run recipe simulate");
        let summary = load_json(
            &self
                .root
                .join("docs/simlog")
                .join(format!("fx_{name}"))
                .join("run_summary.json"),
        );
        let yaml = std::fs::read(
            self.root
                .join("output/single/configs")
                .join(format!("fx-{name}.yaml")),
        )
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
        Out {
            rc: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            lockset: load_json(&lk).unwrap_or(PyValue::None),
            input_lock,
            summary,
            yaml,
        }
    }
}

fn tail(text: &str, n: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[chars.len().saturating_sub(n)..].iter().collect()
}

fn decl(req: i64, typ: i64) -> (&'static str, String) {
    (
        "declared_axes",
        format!(r#"{{"concurrency_requirement": {req}, "typical_request_tokens": {typ}}}"#),
    )
}

// ── E layer ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// Runs E1-E17 with `recipe` (the program under test) against a copy of the skill taken from `repo`, in a directory below `tmp`.
pub fn cli_layer(tmp: &Path, repo: &Path, recipe: &Path) -> Report {
    let mut r = Report::default();
    // The buffer of the expectations below mirrors the owner's constant (checked as U0 in the unit layer; repeated as a guard
    // here because this layer also runs alone, from `checks/gmu_roles_checks.rs`).
    if crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER != BUFFER {
        r.ck("E0 기대값 버퍼 상수 = 정본(estimate_vram::KV_BLOCK_ALIGN_BUFFER)", false, || {
            format!("정본={}", crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER)
        });
        return r;
    }
    let fx = match Fixture::new(tmp, repo, recipe) {
        Ok(fx) => fx,
        Err(error) => {
            r.ck("E0 픽스처 저장소 사본", false, || error.to_string());
            return r;
        }
    };
    let vocab = lockset_vocabulary(&PyStr::from_surrogateescape(fx.root.as_os_str().as_bytes()));
    let stamped = |lk: &PyValue| -> bool {
        let Ok((provenance, knobs)) = &vocab else {
            return false;
        };
        let prov_ok = text(&at(lk, "provenance")).is_some_and(|p| provenance.contains(&p))
            && eq_text(&at(lk, "provenance"), "explorer-phase2");
        let knobs_ok = knobs.iter().all(|(k, allowed)| {
            let v = at(lk, k);
            none(&v) || text(&v).is_some_and(|t| allowed.contains(&t))
        });
        prov_ok && knobs_ok
    };
    let run =
        |name: &str,
         cfg: &[(&str, String)],
         lock: &[(&str, String)],
         profile: &str,
         target: bool| { fx.run_case(name, cfg, lock, profile, target, None, true) };

    // E1 — requirement <= KV-fit. The declared tp=4 differs from the resolved TP 1 and is only recorded.
    let o = run(
        "e1",
        &[(
            "declared_axes",
            format!(
                r#"{{"concurrency_requirement": 16, "typical_request_tokens": {L_TYP}, "tp": 4}}"#
            ),
        )],
        &[],
        "near",
        true,
    );
    let (lk, roles, der) = (
        &o.lockset,
        at(&o.lockset, "gmu_roles"),
        at(&o.lockset, "batch_derivation"),
    );
    r.ck("E1 수렴(rc 0)", o.rc == 0, || tail(&o.stderr, 800));
    r.ck(
        "E1 lockset 기계 각인: provenance=explorer-phase2 · *_source 가 소유자 어휘 안(같은 어휘 소유자로 확인)",
        stamped(lk),
        || lk.repr(),
    );
    r.ck(
        "E1 batch=16 · batch_source=declared-requirement · 위상 2 재검증 통과(verified)",
        eq_num(&at(lk, "batch"), 16.0)
            && eq_text(&at(lk, "batch_source"), "declared-requirement")
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true)),
        || der.repr(),
    );
    r.ck(
        "E1 gmu_source=target_gmu · kv_source=measured-clamp",
        eq_text(&at(lk, "gmu_source"), "target_gmu")
            && eq_text(&at(lk, "kv_source"), "measured-clamp"),
        || lk.repr(),
    );
    r.ck(
        "E1 역할 분리 기록: deploy_gmu == target_gmu(0.85) · gate_margin == safety_margin(0.90)",
        eq_num(&at(&roles, "deploy_gmu"), TARGET_GMU) && eq_num(&at(&roles, "gate_margin"), SAFETY),
        || roles.repr(),
    );
    let y = o.yaml.as_deref();
    r.ck(
        "E1 서빙 yaml: gpu-memory-utilization == target_gmu(≠ safety_margin) · max-num-seqs == 산출 batch · 클램프 emit",
        yaml_value(y, "gpu-memory-utilization").as_deref() == Some("0.85")
            && yaml_value(y, "max-num-seqs").as_deref() == Some("16")
            && yaml_value(y, "kv-cache-memory-bytes") == Some(str_of(&at(lk, "kv_cache_memory_bytes"))),
        || y.unwrap_or("yaml 없음").to_string(),
    );
    r.ck(
        "E1 yaml 에 출처 주석(batch_source·gmu_source)이 실린다 — serve 키가 아닌 주석 줄",
        y.is_some_and(|t| {
            t.contains("batch_source=declared-requirement") && t.contains("gmu_source=target_gmu")
        }),
        || y.unwrap_or("").to_string(),
    );
    r.ck(
        "E1 mock 은 실측인 척하지 않는다: lockset trial_provenance=mock · measured=false · run_summary provenance=mock",
        eq_text(&at(lk, "trial_provenance"), "mock")
            && matches!(at(lk, "measured").peel(), PyValue::Bool(false))
            && o.summary.as_ref().is_some_and(|s| eq_text(&at(s, "provenance"), "mock")),
        || o.summary.as_ref().map_or("None".into(), |s| s.repr()),
    );
    r.ck(
        "E1 엔진 max_concurrency 는 보수 하한으로 기재된다(산식 입력 ✗ · 소비자 0 해소)",
        items(&at(&at(&der, "engine_max_concurrency"), "trials"))
            .iter()
            .any(|t| eq_num(&at(t, "conservative_floor"), 21.0)),
        || at(&der, "engine_max_concurrency").repr(),
    );
    let leaked: Vec<&str> = [
        "gpu_memory_utilization",
        "model_path_container",
        "served_model_name",
        "model_id",
    ]
    .into_iter()
    .filter(|k| has(lk, k) && !has(&o.input_lock, k))
    .collect();
    r.ck(
        "E1 트라이얼 주입값(gmu·모델 경로·서빙명)은 lockset 칸으로 되쓰이지 않는다(다음 실행에서 옛 gmu 가 deploy_gmu 를 이기지 않게)",
        leaked.is_empty(),
        || format!("새어 든 칸={leaked:?}"),
    );
    let tpchk = at(&at(lk, "declared_axes_check"), "tp");
    r.ck(
        "E1 declared_axes.tp=4 ≠ 해소 TP 1 → 기재(match=false · stderr ⚠) · rc 0(차단 ✗)",
        eq_num(&at(&tpchk, "declared"), 4.0)
            && eq_num(&at(&tpchk, "resolved"), 1.0)
            && matches!(at(&tpchk, "match").peel(), PyValue::Bool(false))
            && o.stderr.contains("declared_axes.tp=4"),
        || tpchk.repr(),
    );

    // E2 — requirement > KV-fit: phase 1 lowers it to the KV-fit.
    let o = run("e2", &[decl(128, L_TYP)], &[], "near", true);
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    r.ck(
        "E2 수렴(rc 0) · batch = KV-fit < 요구 128 · kv-fit-measured · 낮춘 사유",
        o.rc == 0
            && !none(&at(&der, "kv_fit"))
            && num(&at(lk, "batch")).is_some()
            && num(&at(lk, "batch")) == num(&at(&der, "kv_fit"))
            && num(&at(&der, "kv_fit")).is_some_and(|k| k < 128.0)
            && eq_text(&at(lk, "batch_source"), "kv-fit-measured")
            && contains(&at(&der, "reason"), "낮춤"),
        || format!("{}{}", der.repr(), tail(&o.stderr, 400)),
    );
    r.ck(
        "E2 경계 산출 batch 로 산정한 클램프가 거부되지 않았다(vram_infeasible ✗) · 재검증 통과",
        matches!(at(&der, "verified").peel(), PyValue::Bool(true))
            && yaml_value(o.yaml.as_deref(), "max-num-seqs") == Some(str_of(&at(lk, "batch"))),
        || der.repr(),
    );

    // E3 — no requirement (no declared_axes block — the default path outside a campaign): batch undecided, one-request clamp.
    let o = run("e3", &[], &[], "near", true);
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    let hist = o
        .summary
        .as_ref()
        .map(|s| items(&at(s, "correction_history")))
        .unwrap_or_default();
    r.ck(
        "E3 수렴 · 요구 absent → batch 미정 · batch_source 없음 · max-num-seqs 미emit · KV-fit 은 참고값으로만",
        o.rc == 0
            && eq_text(&at(&der, "concurrency_requirement_state"), "absent")
            && none(&at(lk, "batch"))
            && none(&at(lk, "batch_source"))
            && yaml_value(o.yaml.as_deref(), "max-num-seqs").is_none()
            && int(&at(&der, "kv_fit")).is_some()
            && contains(&at(&der, "reason"), "참고값"),
        || format!("{}{}", der.repr(), tail(&o.stderr, 400)),
    );
    let want_kv = (70.0 * GIB / 700000.0 * MAX_LEN as f64 * BUFFER).trunc();
    r.ck(
        "E3 클램프는 max_model_len 1건 기준(천장 클램프 ✗ — 선언 없는 config 가 배포 천장 전체를 받지 않는다)",
        hist.iter().any(|h| contains(&at(h, "after_note"), &format!("max_model_len={MAX_LEN}×batch=1")))
            && eq_num(&at(lk, "kv_cache_memory_bytes"), want_kv),
        || hist.iter().map(PyValue::repr).collect::<Vec<_>>().join(", ").chars().take(500).collect(),
    );

    // E4 — phase-2 re-verification lowers batch (engine tokens 400,000 / 8192 = 48 < the phase-1 result 64).
    let o = run("e4", &[decl(64, L_TYP)], &[], "short", true);
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    let hist = o
        .summary
        .as_ref()
        .map(|s| items(&at(s, "correction_history")))
        .unwrap_or_default();
    let first_adjust = items(&at(&der, "phase2_adjustments"))
        .first()
        .cloned()
        .unwrap_or(PyValue::None);
    r.ck(
        "E4 수렴 · 위상 2 에서 adjust_target=batch 로 64→48 · kv-fit-measured · 재검증 통과",
        o.rc == 0
            && eq_num(&at(lk, "batch"), (400000 / L_TYP) as f64)
            && eq_text(&at(lk, "batch_source"), "kv-fit-measured")
            && hist.iter().any(|h| {
                eq_text(&at(h, "adjust_target"), "batch")
                    && eq_num(&at(&at(h, "before"), "batch"), 64.0)
            })
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true))
            && eq_num(&at(&first_adjust, "batch_before"), 64.0),
        || format!("{}{}", der.repr(), tail(&o.stderr, 400)),
    );

    // E5 — host flow: no target_gpu → filled from the manifest · deploy_gmu inherits safety_margin (shown as such).
    let o = run(
        "e5",
        &[
            decl(4, L_TYP),
            ("safety_margin", "0.88".into()),
            ("test_device_total_gib", "100".into()),
        ],
        &[],
        "near",
        false,
    );
    let (lk, roles) = (&o.lockset, at(&o.lockset, "gmu_roles"));
    r.ck(
        "E5 host 흐름 수렴 · gmu_source=hand · deploy_gmu=gate_margin=0.88 · 승계 사실이 출처 문장과 stderr 에",
        o.rc == 0
            && eq_text(&at(lk, "gmu_source"), "hand")
            && eq_text(&at(&roles, "flow"), "host")
            && eq_num(&at(&roles, "deploy_gmu"), 0.88)
            && eq_num(&at(&roles, "gate_margin"), 0.88)
            && contains(&at(&roles, "deploy_gmu_source"), "승계")
            && o.stderr.contains("정본 칸")
            && yaml_value(o.yaml.as_deref(), "gpu-memory-utilization").as_deref() == Some("0.88"),
        || format!("{}{}", roles.repr(), tail(&o.stderr, 600)),
    );

    // E6 — a hand lever is kept (negative control): a person's batch is not overwritten; the formula value is only recorded.
    let o = run(
        "e6",
        &[decl(64, L_TYP)],
        &[("batch", "8".into())],
        "near",
        true,
    );
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    r.ck(
        "E6 손레버 batch=8 유지 · batch_source=hand-lever · 산식 대조값(64)은 derived_batch_would_be 로만",
        o.rc == 0
            && eq_num(&at(lk, "batch"), 8.0)
            && eq_text(&at(lk, "batch_source"), "hand-lever")
            && matches!(at(&der, "applied").peel(), PyValue::Bool(false))
            && eq_num(&at(&der, "derived_batch_would_be"), 64.0)
            && yaml_value(o.yaml.as_deref(), "max-num-seqs").as_deref() == Some("8"),
        || format!("{}{}", der.repr(), tail(&o.stderr, 400)),
    );

    // E7 — the previous explorer product is not carried forward: batch and clamp are cleared and both phases run again.
    let o = run(
        "e7",
        &[decl(16, L_TYP)],
        &[
            ("batch", "99".into()),
            ("batch_source", js("kv-fit-measured")),
            ("kv_cache_memory_bytes", "12345".into()),
            ("kv_source", js("measured-clamp")),
            ("provenance", js("explorer-phase2")),
        ],
        "near",
        true,
    );
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    let hist = o
        .summary
        .as_ref()
        .map(|s| items(&at(s, "correction_history")))
        .unwrap_or_default();
    let before = hist
        .first()
        .map(|h| at(h, "before"))
        .unwrap_or(PyValue::None);
    r.ck(
        "E7 직전 batch 99·클램프 12345 비승계 → trial1 언클램프 실측부터 · batch 16 재산출 · 새 클램프",
        o.rc == 0
            && eq_num(&at(lk, "batch"), 16.0)
            && eq_num(&at(&der, "prior_batch"), 99.0)
            && eq_num(&at(&der, "prior_kv"), 12345.0)
            && !none(&at(lk, "kv_cache_memory_bytes"))
            && !eq_num(&at(lk, "kv_cache_memory_bytes"), 12345.0)
            && !hist.is_empty()
            && none(&at(&before, "kv_cache_memory_bytes"))
            && none(&at(&before, "batch")),
        || format!("{}{}", before.repr(), tail(&o.stderr, 400)),
    );

    // E8 — negative control: target_gmu not declared → exit 5 · no trial, no three-file set, no lockset stamp.
    let o = run(
        "e8",
        &[decl(16, L_TYP), ("target_gpu", TARGET_NO_GMU.into())],
        &[],
        "near",
        true,
    );
    r.ck(
        "E8 target_gmu 미선언 → exit 5 · yaml 없음 · lockset 미각인(입력 그대로)",
        o.rc == 5
            && o.yaml.is_none()
            && !has(&o.lockset, "provenance")
            && o.stderr.contains("target_gmu"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 400)),
    );

    // E9 — an input source outside the vocabulary: recorded, not stopped, and treated as a person's value (never overwritten).
    let o = run(
        "e9",
        &[decl(16, L_TYP)],
        &[("batch", "12".into()), ("batch_source", js("bogus"))],
        "near",
        true,
    );
    let (lk, der) = (&o.lockset, at(&o.lockset, "batch_derivation"));
    r.ck(
        "E9 입력 batch_source 어휘 밖 → rc 0 · 손레버로 유지(12) · 이탈 사유가 기록과 stderr 에",
        o.rc == 0
            && eq_num(&at(lk, "batch"), 12.0)
            && eq_text(&at(lk, "batch_source"), "hand-lever")
            && items(&at(&der, "input_source_anomalies"))
                .iter()
                .any(|a| contains(a, "어휘 밖"))
            && o.stderr.contains("어휘 밖"),
        || format!("{}{}", der.repr(), tail(&o.stderr, 400)),
    );

    // E10 — role separation at the wiring: the trial classification (gate) uses gate_margin, the clamp ceiling deploy_gmu.
    let o = run("e10", &[decl(16, L_TYP)], &[], "heavy", true);
    let s = o.summary.clone().unwrap_or(PyValue::None);
    r.ck(
        "E10 분류는 gate_margin 천장(86.4)으로 통과 → 클램프 천장(deploy_gmu 81.6)에서 vram_infeasible(HITL rc 3)",
        o.rc == 3
            && o.stderr.contains("classify → none")
            && eq_text(&at(&s, "failure_class"), "vram_infeasible")
            && !contains(&at(&s, "note"), "budget*margin")
            && contains(&at(&s, "note"), "weights+overhead(실측)"),
        || format!("rc={} failure_class={} note={}", o.rc, at(&s, "failure_class").repr(), at(&s, "note").repr()),
    );
    let sroles = at(&s, "gmu_roles");
    r.ck(
        "E10 미수렴 요약에도 역할 기록이 실린다(gate_margin 0.90 · deploy_gmu 0.85) · lockset 은 각인하지 않는다",
        eq_num(&at(&sroles, "gate_margin"), SAFETY)
            && eq_num(&at(&sroles, "deploy_gmu"), TARGET_GMU)
            && !has(&o.lockset, "provenance"),
        || sroles.repr(),
    );

    // E11 — a shape defect of a record-only declared axis does not stop simulate.
    let o = run(
        "e11",
        &[(
            "declared_axes",
            format!(
                r#"{{"concurrency_requirement": 16, "typical_request_tokens": {L_TYP}, "tp": "2"}}"#
            ),
        )],
        &[],
        "near",
        true,
    );
    let tpchk = at(&at(&o.lockset, "declared_axes_check"), "tp");
    r.ck(
        "E11 declared_axes.tp='2'(문자열) → rc 0 · declared_state=malformed 기재 · stderr ⚠(기재가 게이트로 격상되지 않는다)",
        o.rc == 0
            && eq_text(&at(&tpchk, "declared_state"), "malformed")
            && none(&at(&tpchk, "match"))
            && o.stderr.contains("대조하지 못했다"),
        || format!("rc={} {} {}", o.rc, tpchk.repr(), tail(&o.stderr, 300)),
    );

    // E12 — host flow: the declared budget (carve-out 24) conflicts with the node's per_card (100) → exit 5 · no products.
    let o = run(
        "e12",
        &[
            decl(4, L_TYP),
            ("vram_budget_gb", "24".into()),
            ("test_device_total_gib", "100".into()),
        ],
        &[],
        "near",
        false,
    );
    r.ck(
        "E12 host 흐름 vram_budget_gb=24 ↔ per_card 100 → exit 5 · yaml·각인 0(실측이 선언 carve-out 을 덮어 rc 0 으로 내지 않는다)",
        o.rc == 5 && o.yaml.is_none() && !has(&o.lockset, "provenance") && o.stderr.contains("target_gpu 블록"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 400)),
    );

    // E13 — a campaign cell lockset is stamped in place even without --lockset-out.
    let cell_lock = fx.root.join("campaigns/camp-fx/cells/e13/lockset.json");
    let o = fx.run_case(
        "e13",
        &[decl(16, L_TYP)],
        &[],
        "near",
        true,
        Some(cell_lock),
        false,
    );
    r.ck(
        "E13 --candidate 가 campaigns/<id>/cells/<cell>/lockset.json 이면 --lockset-out 없이 그 파일에 각인",
        o.rc == 0
            && eq_text(&at(&o.lockset, "provenance"), "explorer-phase2")
            && eq_text(&at(&o.lockset, "trial_provenance"), "mock")
            && o.stderr.contains("--lockset-out 기본값"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 400)),
    );
    // E14 — negative control: a lockset outside a campaign is not overwritten without --lockset-out.
    let o = fx.run_case("e14", &[decl(16, L_TYP)], &[], "near", true, None, false);
    r.ck(
        "E14 음성대조: 캠페인 밖 lockset 은 입력 그대로 · 각인은 run_summary 에만 · 안내 줄",
        o.rc == 0
            && same_value(&o.input_lock, &o.lockset)
            && o.summary
                .as_ref()
                .is_some_and(|s| eq_text(&at(&at(s, "lockset"), "provenance"), "explorer-phase2"))
            && o.stderr.contains("캠페인 셀 lockset 이 아니다"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 300)),
    );

    // E15-E17 — lite ② cap charges: the cap shrinks by the charges of the cell state; exhausted, the run stops before a trial.
    let cell_status = |cell: &str, n: usize, status: Option<&str>| -> PathBuf {
        let d = fx.root.join("campaigns/camp-fx/cells").join(cell);
        std::fs::create_dir_all(&d).expect("a cell directory");
        let charges: Vec<String> = (0..n)
            .map(|i| format!(r#"{{"key": "2026-01-01T0{i}:00:00Z"}}"#))
            .collect();
        let body = format!(
            r#"{{"reconciliation": {{"charges": [{}], "spent": {n}}}, "reentry": {{"status": {}}}}}"#,
            charges.join(", "),
            status.map_or("null".to_string(), js)
        );
        std::fs::write(d.join("cell.status.json"), body).expect("write the cell state");
        d.join("lockset.json")
    };
    let o = fx.run_case(
        "e15",
        &[decl(16, L_TYP)],
        &[],
        "near",
        true,
        Some(cell_status("e15", 1, Some("approved"))),
        false,
    );
    r.ck(
        "E15 셀 상태 lite ② 차감 1 → cap 3 − 1 = 2 (출처 cell.status) · 결정 기록(approved)이면 경고 없음 · 수렴은 그대로",
        o.rc == 0
            && o.stderr.contains("cap 3 − 1 = 2")
            && o.stderr.contains("reconciliation.charges")
            && !o.stderr.contains("재발동 결정 기록이 없다"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 400)),
    );
    let o = fx.run_case(
        "e16",
        &[decl(16, L_TYP)],
        &[],
        "near",
        true,
        Some(cell_status("e16", 3, Some("proposed"))),
        false,
    );
    r.ck(
        "E16 차감 3(= cap) → 트라이얼 없이 Model-C(rc 3) · 결정 기록 없음 경고 · lockset 각인 0 · run 요약 0",
        o.rc == 3
            && o.stderr.contains("reconciliation_cap 소진")
            && o.stderr.contains("재발동 결정 기록이 없다")
            && !o.stderr.contains("trial 1/")
            && !has(&o.lockset, "provenance")
            && o.summary.is_none(),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 400)),
    );
    let o = fx.run_case(
        "e17",
        &[decl(16, L_TYP)],
        &[],
        "near",
        true,
        Some(cell_status("e17", 0, None)),
        false,
    );
    r.ck(
        "E17 음성대조: 차감 0 이면 cap 안내 줄 없음(캠페인 셀이어도 종전 그대로)",
        o.rc == 0 && !o.stderr.contains("lite ② 차감"),
        || format!("rc={} {}", o.rc, tail(&o.stderr, 300)),
    );
    r
}

// ── F layer (a deterministic fake engine in place of the trial engine) ──────────────────────────────────────────────

const MANIFEST_MULTI2: &str = "topology: multi\ngpus_per_node: 1\ngpu_model: \"NVIDIA RTX PRO 6000\"\nmodel_source: managed\nnodes:\n  - role: main\n  - role: sub\nterraforming:\n  complete: true\n  branch_verified: true\n";

/// The fake engine's physics (`ENGINE` of the original plus the per-case overrides).
#[derive(Clone)]
struct Engine {
    per_card_gib: f64,
    weights_gib: f64,
    overhead_gib: f64,
    per_token_bytes: i64,
    block: i64,
    tp: i64,
    oom_trials: Vec<i64>,
    clamped_tokens_unobserved: bool,
    clamped_per_token_scale: f64,
    /// plan_26111708 B L3: the engine refuses `max_num_seqs` (default 256 when no batch) above this many Mamba cache blocks.
    mamba_blocks: Option<i64>,
    /// plan_26111708 B L2: one max-length request needs this many GiB; a clamp minus `null_block_gib` below it is refused.
    one_request_gib: Option<f64>,
    null_block_gib: f64,
}

impl Default for Engine {
    fn default() -> Engine {
        Engine {
            per_card_gib: 96.0,
            weights_gib: 10.0,
            overhead_gib: 2.0,
            per_token_bytes: 131072,
            block: 16,
            tp: 1,
            oom_trials: Vec::new(),
            clamped_tokens_unobserved: false,
            clamped_per_token_scale: 1.0,
            mamba_blocks: None,
            one_request_gib: None,
            null_block_gib: 0.0,
        }
    }
}

/// One engine call as the original recorded it (`{"t", "clamp", "batch", "tokens", "load_ok"}`).
#[derive(Clone, Debug)]
struct Call {
    clamp: Option<i64>,
    batch: Option<i64>,
    tokens: i64,
    load_ok: bool,
}

/// `fake_run_trial` and the captured `feedback_append` of the original driver.
struct FakeEngine {
    eng: Engine,
    calls: Vec<Call>,
    feedback: Vec<PyValue>,
    /// Where a refusing trial writes its engine log (the loop reads the statement from `log_path`).
    log_dir: PathBuf,
}

fn dict_get(d: &agent_runtime_support::pyvalue::PyDict, k: &str) -> PyValue {
    d.get(k).cloned().unwrap_or(PyValue::None)
}

impl crate::recipe_cmd::TrialEngine for FakeEngine {
    fn run_trial(&mut self, cand: &agent_runtime_support::pyvalue::PyDict, trial_number: i64) -> agent_runtime_support::pyvalue::PyDict {
        use agent_runtime_support::pyvalue::PyDict;
        let e = &self.eng;
        let clamp = int(&dict_get(cand, "kv_cache_memory_bytes"));
        let tp = e.tp as f64;
        let (kv_bytes, per_tok) = match clamp {
            // unclamped: the engine takes KV up to the trial gmu ceiling (per GPU)
            None => {
                let gmu = num(&dict_get(cand, "gpu_memory_utilization")).unwrap_or(0.0);
                (
                    (e.per_card_gib * gmu * GIB) as i64 - (e.weights_gib / tp * GIB) as i64 - (e.overhead_gib / tp * GIB) as i64,
                    e.per_token_bytes,
                )
            }
            // clamped: engine tokens follow the clamp (block rounding)
            Some(c) => (c, (e.per_token_bytes as f64 * e.clamped_per_token_scale) as i64),
        };
        let tokens = kv_bytes.div_euclid(per_tok * e.block) * e.block;
        let max_len = num(&dict_get(cand, "max_model_len")).unwrap_or(1.0);
        // plan_26111708 B L2 · L3: the engine's own refusals, in vLLM 0.32.0's words (config/compilation.py:1519-1523 ·
        // v1/core/kv_cache_utils.py:1030-1040), written to the trial log the loop reads.
        let batch = int(&dict_get(cand, "batch"));
        let mut refusal: Option<String> = None;
        if let Some(blocks) = e.mamba_blocks {
            let seqs = batch.unwrap_or(256);
            if seqs > blocks {
                refusal = Some(format!("ValueError: max_num_seqs ({seqs}) exceeds available Mamba cache blocks ({blocks}). Each decode sequence requires one Mamba cache block, so CUDA graph capture cannot proceed. Please lower max_num_seqs to at most {blocks} or increase gpu_memory_utilization."));
            }
        }
        if let (None, Some(need), Some(c)) = (&refusal, e.one_request_gib, clamp) {
            let available = c as f64 / GIB - e.null_block_gib;
            if available < need {
                refusal = Some(format!("ValueError: To serve at least one request with the model's max seq len ({}), ({:.2} GiB KV cache is needed, which is larger than the available KV cache memory ({:.2} GiB). Based on the available memory, the estimated maximum model length is 4096. Try increasing `gpu_memory_utilization` or decreasing `max_model_len` when initializing the engine.", max_len as i64, need, available));
            }
        }
        let log_path = refusal.as_ref().map(|text| {
            let path = self.log_dir.join(format!("fake_trial{trial_number:02}_vllm.log"));
            let _ = std::fs::write(&path, format!("(EngineCore) ERROR [core.py:1510] {text}\n"));
            path
        });
        let load_ok = !e.oom_trials.contains(&trial_number) && refusal.is_none();
        let unobserved = clamp.is_some() && e.clamped_tokens_unobserved;
        let mut prof = PyDict::new();
        prof.set("weights_gib", PyValue::Float(e.weights_gib));
        prof.set("non_kv_overhead_gib", PyValue::Float(e.overhead_gib));
        prof.set("kv_cache_gib", PyValue::Float(kv_bytes as f64 / GIB));
        prof.set("kv_cache_tokens", if unobserved { PyValue::None } else { PyValue::int(tokens) });
        prof.set("max_concurrency", if unobserved { PyValue::None } else { PyValue::Float(tokens as f64 / max_len) });
        self.calls.push(Call { clamp, batch: int(&dict_get(cand, "batch")), tokens, load_ok });
        // OOM injection = the host watchdog SIGKILL shape (KV taken, then a silent end → the classifier's vram_oom)
        let mut functional = PyDict::new();
        functional.set("passed", PyValue::Bool(true));
        let mut out = PyDict::new();
        out.set("trial_number", PyValue::int(trial_number));
        out.set("candidate", PyValue::Dict(cand.clone()));
        out.set("load_ok", PyValue::Bool(load_ok));
        out.set("vllm_profile", PyValue::Dict(prof));
        out.set("functional", if load_ok { PyValue::Dict(functional) } else { PyValue::None });
        out.set(
            "log_path",
            log_path.map_or(PyValue::None, |p| PyValue::Str(PyStr::from_surrogateescape(p.as_os_str().as_bytes()))),
        );
        out.set("error_excerpt", PyValue::str(refusal.as_deref().unwrap_or("")));
        out.set("provenance", PyValue::str("mock"));
        out.set("health_wait", PyValue::None);
        out.set("effective_gmu", dict_get(cand, "gpu_memory_utilization"));
        out
    }

    fn feedback(&mut self, record: &PyValue) {
        self.feedback.push(record.clone());
    }
}

struct EngineOut {
    rc: i32,
    calls: Vec<Call>,
    feedback: Vec<PyValue>,
    stderr: String,
    lockset: PyValue,
    summary: Option<PyValue>,
}

fn decl_opt(req: Option<i64>, typ: Option<i64>) -> (&'static str, String) {
    let show = |v: Option<i64>| v.map_or("null".to_string(), |x| x.to_string());
    (
        "declared_axes",
        format!(r#"{{"concurrency_requirement": {}, "typical_request_tokens": {}}}"#, show(req), show(typ)),
    )
}

impl Fixture {
    /// One `simulate` run in process with the fake engine (the original's driver: the copy's manifest is rewritten first, the
    /// trial engine, RAM gate and feedback writer are replaced, the Flag gate is not on the path).
    fn engine_case(
        &self,
        name: &str,
        cfg_extra: &[(&str, String)],
        lockset: &[(&str, String)],
        engine: Engine,
        manifest: &str,
        cap: i64,
    ) -> EngineOut {
        let case = self.tmp.join("fcases").join(name);
        std::fs::create_dir_all(&case).expect("a case directory");
        let cname = format!("f{name}");
        let mut extra: Vec<(&str, String)> = cfg_extra.to_vec();
        if !extra.iter().any(|(k, _)| *k == "test_device_total_gib") {
            extra.push(("test_device_total_gib", "96".to_string()));
        }
        extra.push(("target_gpu", TARGET.to_string()));
        let cfg = Obj(vec![
            ("target_model".into(), r#"{"path": "/app/models/fx-org/fx-model"}"#.into()),
            ("nas_host_root".into(), js(&self.tmp.join("nas").to_string_lossy())),
            ("vram_budget_gb".into(), "96".into()),
            ("safety_margin".into(), SAFETY.to_string()),
            ("kv_cache_dtype_bytes".into(), "2".into()),
            (
                "serving".into(),
                format!(r#"{{"config_name": {}, "port": 18080, "served_model_name": "fx"}}"#, js(&format!("fx-{cname}"))),
            ),
            ("target_gpu".into(), TARGET.to_string()),
        ])
        .update(&extra)
        .without_nulls();
        let cfg_path = case.join("config.yaml");
        std::fs::write(&cfg_path, cfg.json()).expect("write the config");
        let lk = case.join("lockset.json");
        std::fs::write(&lk, Self::base_lock(&cname).update(lockset).json()).expect("write the lockset");
        std::fs::write(self.root.join("output/single/manifest.yaml"), manifest).expect("the copy's manifest");
        let p = |path: &Path| PyStr::from_surrogateescape(path.as_os_str().as_bytes());
        let run_id = format!("ffx_{name}");
        let args = vec![
            PyStr::new("--config"),
            p(&cfg_path),
            PyStr::new("--candidate"),
            p(&lk),
            PyStr::new("--lockset-out"),
            p(&lk),
            PyStr::new("--run-id"),
            PyStr::new(&run_id),
            PyStr::new("--cap"),
            PyStr::new(&cap.to_string()),
        ];
        let mut fake = FakeEngine { eng: engine, calls: Vec::new(), feedback: Vec::new(), log_dir: case.clone() };
        let run = crate::recipe_cmd::simulate_with_engine(&args, &p(&self.root.join(SKILL_REL)), &mut fake);
        let stderr = run.stderr.to_utf8().unwrap_or_default();
        EngineOut {
            rc: run.status,
            calls: fake.calls,
            feedback: fake.feedback,
            stderr: tail(&stderr, 3000),
            lockset: load_json(&lk).unwrap_or(PyValue::None),
            summary: load_json(&self.root.join("docs/simlog").join(&run_id).join("run_summary.json")),
        }
    }
}

/// `_unclamped`: the engine's unclamped KV bytes and tokens.
fn unclamped(w: f64, o: f64, tp: f64) -> (i64, i64) {
    let kv = (96.0 * 0.85 * GIB) as i64 - (w / tp * GIB) as i64 - (o / tp * GIB) as i64;
    let tokens = kv.div_euclid(131072 * 16) * 16;
    (kv, tokens)
}

/// `_fit_tokens`: the unclamped KV-fit conversion of the owner (expectation only — the product is what is compared).
fn fit_tokens(max_safe: i64, per_token: f64) -> i64 {
    let mut tokens = (max_safe as f64 / (per_token * BUFFER)).floor() as i64;
    while tokens > 0 && (per_token * tokens as f64 * BUFFER) as i64 > max_safe {
        tokens -= 1;
    }
    tokens
}

fn calls_text(calls: &[Call]) -> String {
    format!("{calls:?}")
}

/// Runs F1-F14 (F10-F14: the engine refusals of plan_26111708 B L2 · L3) against a copy of the skill taken from `repo`, in a directory below `tmp` (no program is started: the simulate
/// command runs in this process over the copy, with the trial engine replaced).
#[allow(clippy::too_many_lines)]
pub fn engine_layer(tmp: &Path, repo: &Path) -> Report {
    let mut r = Report::default();
    let fx = match Fixture::new(tmp, repo, Path::new("recipe")) {
        Ok(fx) => fx,
        Err(error) => {
            r.ck("F0 픽스처 저장소 사본", false, || error.to_string());
            return r;
        }
    };
    let max_safe = (96.0 * 0.85 * GIB) as i64 - (10.0 * GIB) as i64 - (2.0 * GIB) as i64;
    let fb_ok = |o: &EngineOut| {
        o.feedback.len() == 1
            && eq_num(&at(&o.feedback[0], "safety_margin_threshold"), SAFETY)
            && eq_num(&at(&o.feedback[0], "gpu_memory_utilization"), TARGET_GMU)
    };
    let der_of = |o: &EngineOut| at(&o.lockset, "batch_derivation");
    let fb_text = |o: &EngineOut| tail(&format!("{:?}", o.feedback.iter().map(|f| f.dumps_ascii().unwrap_or_default()).collect::<Vec<_>>()), 400);
    let oom1 = Engine { oom_trials: vec![1], ..Engine::default() };

    // F1 — first trial OOM → provisional clamp → pass → requirement kept at the deployment-ceiling KV-fit → clamp re-derived
    let o = fx.engine_case("oomfirst", &[decl_opt(Some(16), Some(L_TYP))], &[], oom1.clone(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    let raw_fit = calls.get(1).map(|c| c.tokens / L_TYP);
    r.ck(
        &format!("F1 음성대조 전제: 잠정 클램프 트라이얼의 원토큰으로 세면 KV-fit 은 요구보다 작다({} < 16)", raw_fit.map_or("None".into(), |v| v.to_string())),
        raw_fit.is_some_and(|f| f < 16) && calls[1].clamp.is_some() && !calls[0].load_ok,
        || calls_text(calls),
    );
    r.ck(
        "F1 OOM-first: batch 16 · declared-requirement(잠정 클램프 원토큰으로 깎지 않는다) · KV-fit 은 배포 천장 환산",
        o.rc == 0
            && int(&at(lk, "batch")) == Some(16)
            && eq_text(&at(lk, "batch_source"), "declared-requirement")
            && contains(&at(&der, "kv_fit_tokens_source"), "언클램프 측정 환산")
            && int(&at(&der, "kv_fit")).unwrap_or(0) >= 16,
        || format!("{} {}", tail(&der.dumps_ascii().unwrap_or_default(), 700), tail(&o.stderr, 600)),
    );
    let clamp3 = calls.get(2).and_then(|c| c.clamp);
    let per_tok_t2 = calls.get(1).filter(|c| c.tokens != 0).and_then(|c| c.clamp.map(|k| k as f64 / c.tokens as f64));
    r.ck(
        "F1 잠정 클램프를 산출 batch 로 재산정(required = per_token × max(len, L×16) × 버퍼) → 3번째 트라이얼이 재검증 · verified",
        calls.len() == 3
            && per_tok_t2.is_some_and(|pt| clamp3 == Some((pt * (MAX_LEN.max(L_TYP * 16)) as f64 * BUFFER) as i64))
            && calls[2].batch == Some(16)
            && int(&at(lk, "kv_cache_memory_bytes")) == clamp3
            && eq_text(&at(lk, "kv_source"), "measured-clamp")
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true)),
        || format!("calls={} clamp3={clamp3:?}", calls_text(calls)),
    );
    r.ck(
        "F1 되먹임: safety_margin_threshold == gate_margin(0.90) · gpu_memory_utilization == deploy_gmu(0.85)",
        fb_ok(&o),
        || fb_text(&o),
    );

    // F2 — first trial OOM and no requirement → batch undecided · the provisional clamp is final (one request) · 2 trials
    let o = fx.engine_case("oomnoreq", &[decl_opt(None, None)], &[], oom1, MANIFEST, 3);
    let (lk, calls) = (&o.lockset, &o.calls);
    r.ck(
        "F2 OOM-first ∧ 요구 없음 → batch 미정 · 2트라이얼 수렴 · 클램프 = 1건(max_model_len) 기준",
        o.rc == 0
            && calls.len() == 2
            && none(&at(lk, "batch"))
            && none(&at(lk, "batch_source"))
            && int(&at(lk, "kv_cache_memory_bytes")) == calls[1].clamp
            && calls[1].clamp.is_some_and(|c| (c as f64) < 0.1 * max_safe as f64),
        || format!("calls={} {}", calls_text(calls), tail(&o.stderr, 400)),
    );

    // F3 — no requirement (normal): no ceiling clamp · one max_model_len request clamp
    let o = fx.engine_case("noreq", &[decl_opt(None, None)], &[], Engine::default(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    let (kv1, tok1) = unclamped(10.0, 2.0, 1.0);
    let kv = int(&at(lk, "kv_cache_memory_bytes"));
    r.ck(
        "F3 요구 없음 → batch 미정 · 클램프 = per_token × max_model_len × 버퍼(1건) · 배포 천장의 10% 미만 · KV-fit 참고값",
        o.rc == 0
            && none(&at(lk, "batch"))
            && calls.len() == 2
            && kv == Some((kv1 as f64 / tok1 as f64 * MAX_LEN as f64 * BUFFER) as i64)
            && kv.is_some_and(|k| (k as f64) < 0.1 * max_safe as f64)
            && int(&at(&der, "kv_fit")).is_some(),
        || format!("kv={kv:?} calls={} {}", calls_text(calls), tail(&o.stderr, 400)),
    );

    // F4 — an input hand clamp (40 GiB) and no batch: that clamp is the deployed value → raw engine tokens · re-verified · hand
    let forty = 40 * (GIB as i64);
    let o = fx.engine_case("handclamp", &[decl_opt(Some(16), Some(L_TYP))], &[("kv_cache_memory_bytes", forty.to_string())], Engine::default(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    let hand_tokens = forty.div_euclid(131072 * 16) * 16;
    r.ck(
        &format!("F4 사람 클램프 → KV-fit = 그 클램프의 엔진 원토큰 ÷ L({}) · batch 16 · 재검증 트라이얼 · kv_source=hand", hand_tokens / L_TYP),
        o.rc == 0
            && int(&at(&der, "kv_fit")) == Some(hand_tokens / L_TYP)
            && contains(&at(&der, "kv_fit_tokens_source"), "배포 클램프 트라이얼")
            && int(&at(lk, "batch")) == Some(16)
            && calls.len() == 2
            && calls[1].batch == Some(16)
            && int(&at(lk, "kv_cache_memory_bytes")) == Some(forty)
            && eq_text(&at(lk, "kv_source"), "hand")
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true)),
        || format!("{} {}", tail(&der.dumps_ascii().unwrap_or_default(), 500), calls_text(calls)),
    );

    // F5 — TP=2 target flow: measured weights/overhead divided by TP for the KV-fit (the same ÷TP as the clamp formula)
    let tp2 = Engine { weights_gib: 40.0, overhead_gib: 4.0, tp: 2, ..Engine::default() };
    let o = fx.engine_case("tp2", &[decl_opt(Some(16), Some(L_TYP))], &[], tp2, MANIFEST_MULTI2, 3);
    let (lk, der) = (&o.lockset, der_of(&o));
    let (kv_t, tok_t) = unclamped(40.0, 4.0, 2.0);
    let safe_div = (96.0 * 0.85 * GIB) as i64 - (20.0 * GIB) as i64 - (2.0 * GIB) as i64;
    let safe_nodiv = (96.0 * 0.85 * GIB) as i64 - (40.0 * GIB) as i64 - (4.0 * GIB) as i64;
    let per_t = kv_t as f64 / tok_t as f64;
    let (want_div, want_nodiv) = (fit_tokens(safe_div, per_t), fit_tokens(safe_nodiv, per_t));
    r.ck(
        &format!("F5 TP=2: tp_divisor=2 · KV-fit 토큰 = (천장 − weights/2 − overhead/2) 환산({want_div}) — ÷TP 를 빼면 {want_nodiv} 가 된다"),
        o.rc == 0
            && int(&at(&at(lk, "gmu_roles"), "tp_divisor")) == Some(2)
            && int(&at(&der, "kv_fit_tokens")) == Some(want_div)
            && want_div != want_nodiv
            && int(&at(lk, "batch")) == Some(16),
        || format!("{} {}", tail(&der.dumps_ascii().unwrap_or_default(), 500), tail(&o.stderr, 600)),
    );

    // F6 — the clamp trial's engine tokens unobserved → phase 2 cannot re-verify → verified=false
    let unobs = Engine { clamped_tokens_unobserved: true, ..Engine::default() };
    let o = fx.engine_case("unobserved", &[decl_opt(Some(16), Some(L_TYP))], &[], unobs, MANIFEST, 3);
    let (lk, der) = (&o.lockset, der_of(&o));
    r.ck(
        "F6 클램프 토큰 미관측 → 수렴하되 verified=false · verified_gap 에 사유(미관측)",
        o.rc == 0
            && int(&at(lk, "batch")) == Some(16)
            && matches!(at(&der, "verified").peel(), PyValue::Bool(false))
            && contains(&at(&der, "verified_gap"), "미관측"),
        || tail(&der.dumps_ascii().unwrap_or_default(), 500),
    );

    // F7 — per-token mismatch (clamp-trial engine per-token × 1.34): phase 2 really lowers
    let mismatch = Engine { clamped_per_token_scale: 1.34, ..Engine::default() };
    let o = fx.engine_case("mismatch", &[decl_opt(Some(64), Some(L_TYP))], &[], mismatch.clone(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    let low = calls.get(1).map(|c| c.tokens / L_TYP);
    let first_adj = items(&at(&der, "phase2_adjustments")).first().cloned().unwrap_or(PyValue::None);
    r.ck(
        &format!("F7 per-token 불일치 → 위상 2 가 64→{} 로 낮추고 같은 클램프로 재검증 통과(클램프는 재산정하지 않는다)", low.map_or("None".into(), |v| v.to_string())),
        o.rc == 0
            && low.is_some_and(|l| l < 64)
            && int(&at(lk, "batch")) == low
            && int(&at(&first_adj, "batch_before")) == Some(64)
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true))
            && calls.len() == 3
            && calls[1].clamp == calls[2].clamp,
        || format!("{} {}", calls_text(calls), tail(&der.dumps_ascii().unwrap_or_default(), 300)),
    );

    // F8 — the same with --cap 2: lowered in the last slot → HITL (rc 3) summary says why · batch_derivation carried
    let o = fx.engine_case("mismatch_cap2", &[decl_opt(Some(64), Some(L_TYP))], &[], mismatch, MANIFEST, 2);
    let s = o.summary.clone().unwrap_or(PyValue::None);
    let adj = items(&at(&at(&s, "batch_derivation"), "phase2_adjustments")).first().cloned().unwrap_or(PyValue::None);
    r.ck(
        "F8 cap 2 소진 → rc 3 · note 에 'cap(2) 소진 — 위상 2' · 요약 batch_derivation.phase2_adjustments · lockset 미각인",
        o.rc == 3
            && contains(&at(&s, "note"), "cap(2) 소진")
            && contains(&at(&s, "note"), "위상 2")
            && int(&at(&adj, "batch_before")) == Some(64)
            && !has(&o.lockset, "provenance"),
        || format!("rc={} class={:?} note={:?}", o.rc, text(&at(&s, "failure_class")), text(&at(&s, "note"))),
    );

    // F9 — per-token match (the real boundary): requirement 64 ≤ converted KV-fit → no phase-2 lowering · 2 trials
    let o = fx.engine_case("realistic64", &[decl_opt(Some(64), Some(L_TYP))], &[], Engine::default(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    r.ck(
        "F9 per-token 일치 → 클램프 트라이얼 엔진 토큰이 batch×L 을 담는다 · 하향 없음 · verified · 2칸",
        o.rc == 0
            && int(&at(lk, "batch")) == Some(64)
            && items(&at(&der, "phase2_adjustments")).is_empty()
            && matches!(at(&der, "verified").peel(), PyValue::Bool(true))
            && calls.len() == 2
            && calls[1].tokens / L_TYP >= 64
            && fb_ok(&o),
        || format!("{} {}", calls_text(calls), tail(&der.dumps_ascii().unwrap_or_default(), 300)),
    );

    // ── plan_26111708 B L2 · L3 (the engine's own refusals) ──
    let history_text = |o: &EngineOut| {
        o.summary.as_ref().map_or(String::new(), |s| at(s, "correction_history").dumps_ascii().unwrap_or_default())
    };
    let one_request = (131072.0 * MAX_LEN as f64 * BUFFER) / GIB; // the formula clamp of one max_model_len request (GiB)

    // F10 — the clamp leaves less than one request after the null block → raised by the engine's printed shortfall → passes
    let short = Engine { one_request_gib: Some(one_request + 0.02), null_block_gib: 0.05, ..Engine::default() };
    let o = fx.engine_case("shortfall", &[decl_opt(None, None)], &[], short, MANIFEST, 3);
    let (lk, calls) = (&o.lockset, &o.calls);
    let raised = calls.get(2).and_then(|c| c.clamp);
    r.ck(
        "F10 L2 클램프 1건 부족 거부 → 엔진 실측 부족분만큼 올림(축소 ✗) · 널 블록 뒤에도 1건 · 3트라이얼 수렴 · kv_source=measured-clamp",
        o.rc == 0
            && calls.len() == 3
            && !calls[1].load_ok
            && raised.zip(calls[1].clamp).is_some_and(|(up, before)| up > before)
            && raised.is_some_and(|up| up as f64 / GIB - 0.05 >= one_request + 0.02)
            && int(&at(lk, "kv_cache_memory_bytes")) == raised
            && eq_text(&at(lk, "kv_source"), "measured-clamp")
            && history_text(&o).contains("vllm-needed-measured"),
        || format!("calls={} {}", calls_text(calls), tail(&o.stderr, 600)),
    );

    // F11 — the engine's need is over the deployed ceiling → structural (HITL), no further trial, nothing stamped
    let huge = Engine { one_request_gib: Some(80.0), null_block_gib: 0.05, ..Engine::default() };
    let o = fx.engine_case("shortfall_huge", &[decl_opt(None, None)], &[], huge, MANIFEST, 3);
    let s = o.summary.clone().unwrap_or(PyValue::None);
    r.ck(
        "F11 L2 음성: 엔진 실측 1건 > 배포 천장 → vram_infeasible · 2칸에서 멈춤 · lockset 미각인",
        o.rc == 3
            && o.calls.len() == 2
            && eq_text(&at(&s, "failure_class"), "vram_infeasible")
            && contains(&at(&s, "note"), "배포 천장")
            && !has(&o.lockset, "provenance"),
        || format!("rc={} calls={} note={:?}", o.rc, calls_text(&o.calls), text(&at(&s, "note"))),
    );

    let hybrid_model = ("target_model", r#"{"path": "/app/models/fx-org/fx-hybrid"}"#.to_string());
    let mamba = Engine { mamba_blocks: Some(83), ..Engine::default() };

    // F12 — hybrid ∧ declared requirement → the first trial already carries the requirement (never the engine default 256)
    let o = fx.engine_case("hybrid_req", &[decl_opt(Some(16), Some(L_TYP)), hybrid_model.clone()], &[], mamba.clone(), MANIFEST, 3);
    let calls = &o.calls;
    r.ck(
        "F12 L3 하이브리드 ∧ 요구 16 → 첫 트라이얼 batch=16(엔진 기본 256 ✗) · Mamba 거부 0 · 수렴",
        o.rc == 0 && calls.first().is_some_and(|c| c.batch == Some(16) && c.load_ok) && calls.iter().all(|c| c.load_ok),
        || format!("calls={} {}", calls_text(calls), tail(&o.stderr, 600)),
    );
    // F12 음성대조: the same model as a non-hybrid would have gone in without a batch (the trial-1 batch is the L3 rule)
    let o = fx.engine_case("nonhybrid_req", &[decl_opt(Some(16), Some(L_TYP))], &[], Engine::default(), MANIFEST, 3);
    r.ck(
        "F12 음성대조: 비하이브리드는 첫 트라이얼 batch 없음(종전 그대로)",
        o.rc == 0 && o.calls.first().is_some_and(|c| c.batch.is_none()),
        || calls_text(&o.calls),
    );

    // F13 — hybrid ∧ no requirement → refused at the default → batch = the engine's Mamba blocks (measured) · the clamp stays
    // one request (the cap is an admission ceiling, not a concurrency claim)
    let o = fx.engine_case("hybrid_noreq", &[decl_opt(None, None), hybrid_model.clone()], &[], mamba.clone(), MANIFEST, 3);
    let (lk, der, calls) = (&o.lockset, der_of(&o), &o.calls);
    r.ck(
        "F13 L3 요구 없음 → 기본값 거부 → batch=83(엔진 실측 Mamba 블록) · batch_source=kv-fit-measured · 클램프 = 1건 · 3칸",
        o.rc == 0
            && calls.len() == 3
            && !calls[0].load_ok
            && calls[0].batch.is_none()
            && calls[1].batch == Some(83)
            && int(&at(lk, "batch")) == Some(83)
            && eq_text(&at(lk, "batch_source"), "kv-fit-measured")
            && int(&at(&der, "mamba_cache_cap")) == Some(83)
            && int(&at(lk, "kv_cache_memory_bytes")).is_some_and(|k| (k as f64) < 1.5 * one_request * GIB)
            && history_text(&o).contains("Mamba"),
        || format!("calls={} {} {}", calls_text(calls), tail(&der.dumps_ascii().unwrap_or_default(), 400), tail(&o.stderr, 400)),
    );

    // F14 — a hand-lever batch over the measured blocks is not overwritten → HITL with the measured limit
    let o = fx.engine_case("hybrid_hand", &[decl_opt(None, None), hybrid_model], &[("batch", "128".to_string())], mamba, MANIFEST, 3);
    let s = o.summary.clone().unwrap_or(PyValue::None);
    r.ck(
        "F14 L3 음성: 손레버 batch 128 > Mamba 블록 83 → 덮어쓰지 않고 HITL(rc 3 · 1칸 · note 에 83)",
        o.rc == 3 && o.calls.len() == 1 && contains(&at(&s, "note"), "손레버") && contains(&at(&s, "note"), "83"),
        || format!("rc={} calls={} note={:?}", o.rc, calls_text(&o.calls), text(&at(&s, "note"))),
    );
    r
}

/// The command line: header, one line per check (U, E, F), verdict; status 0 when every check passed, 1 otherwise, 2 when the
/// program under test is absent.
pub fn main_with(repo: &Path, recipe: &Path) -> i32 {
    println!(
        "[selftest_gmu_roles_batch] plan_26091407 §4.2·§4.3 · §7 O3 — U(함수 층) · E(CLI 층 · Rust recipe) · F(가짜 엔진 층)"
    );
    if !recipe.is_file() {
        eprintln!(
            "[selftest_gmu_roles_batch] ERROR the program under test is missing: {}",
            recipe.display()
        );
        eprintln!("[selftest_gmu_roles_batch] build first: bash .claude/skills/terraforming_node/scripts/build_rust_packages.sh");
        return 2;
    }
    let dir = match crate::model_files::PrivateDirectory::create("selftest_gmu_roles_batch_") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("[selftest_gmu_roles_batch] ERROR temporary directory: {error}");
            return 2;
        }
    };
    let mut report = crate::recipe_cmd::gmu_unit_layer(&dir.path().join("u"), repo);
    for layer in [cli_layer(&dir.path().join("e"), repo, recipe), engine_layer(&dir.path().join("f"), repo)] {
        report.lines.extend(layer.lines);
        report.failures.extend(layer.failures);
    }
    for line in &report.lines {
        println!("{line}");
    }
    if report.failures.is_empty() {
        println!("[selftest_gmu_roles_batch] PASS");
        0
    } else {
        println!(
            "[selftest_gmu_roles_batch] FAIL ({})",
            report.failures.len()
        );
        1
    }
}
