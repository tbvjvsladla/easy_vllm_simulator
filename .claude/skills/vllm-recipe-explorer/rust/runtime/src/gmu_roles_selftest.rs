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
//! Not ported here (they exercised private functions of the retired `recipe.py` in-process): the U layer (function units) and
//! the F layer (a deterministic fake engine swapped in for `run_trial`); see the bundle's PORTING notes.

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
    fn ck(&mut self, name: &str, cond: bool, detail: impl FnOnce() -> String) {
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
    // The buffer of the expectations below mirrors the owner's constant (so the expectations cannot silently lag behind it).
    r.ck(
        "U0 픽스처 버퍼 상수가 정본(estimate_vram::KV_BLOCK_ALIGN_BUFFER)과 같다(기대값 계산이 거울로 늦지 않게)",
        crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER == BUFFER,
        || format!("정본={}", crate::estimate_vram::KV_BLOCK_ALIGN_BUFFER),
    );
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

/// The command line: header, one line per check, verdict; status 0 when every check passed, 1 otherwise, 2 when the program
/// under test is absent.
pub fn main_with(repo: &Path, recipe: &Path) -> i32 {
    println!(
        "[selftest_gmu_roles_batch] plan_26091407 §4.2·§4.3 · §7 O3 — E(CLI 층 · Rust recipe)"
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
    let report = cli_layer(dir.path(), repo, recipe);
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
