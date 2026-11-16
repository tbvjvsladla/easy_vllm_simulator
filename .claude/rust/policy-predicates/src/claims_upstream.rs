//! Clauses ported against the upstream-version-watch owner (see claims.rs for the mapping):
//! RUNTIME_PATCH_NO_CARRY_FORWARD.C1/C4 and ARCH_WALL_VARIANT_LADDER.C1-C5.
//!
//! The original imported `render_dockerfile.py` / `classify_failure.py` / `policy_registry.py` and called them in-process.
//! Here the same owner functions are linked in-process from their Rust owners:
//!   * `upstream_runtime::render_template::{build_context, patch_guard, VALIDATED_SOURCE_BUILD_KEYS}` and
//!     `upstream_runtime::render_dockerfile::RUNNER_SCRIPTS` (render_dockerfile);
//!   * `upstream_runtime::classify_source::{load_patterns, matched_lines}` (classify_failure);
//!   * `policy_audit::arch::arch_variant_contract_violations` (policy_registry).
//!
//! Re-targeted atoms (the original inspected Python objects that have no Rust equivalent):
//!   * `inspect.signature(build_context)` -> the parameter list of `pub fn build_context(` read from the owner's Rust
//!     source (`render_template.rs`). The Rust function carries the original's module global `ALLOW_UNVALIDATED` as an
//!     explicit `allow_unvalidated` render flag; the invariant kept is the same: no model/patch-identifying input.
//!   * the `ALLOW_UNVALIDATED = True` monkeypatch -> `patch_guard(.., allow_unvalidated=true)`; "the frozen set is never
//!     mutated" -> the set is a Rust `const` in the owner source (asserted textually) and equal before/after.
//!   * `_arch_fixture_git` (stubs git's index oracle so the index = the bytes on disk) -> a real `git init` in the fixture
//!     and `git add -A` before each verdict, which makes the index exactly the bytes on disk.
#![allow(unused_imports)]
use crate::support::*;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use upstream_runtime::render_dockerfile::V;
use upstream_runtime::render_template as rt;

const SYNC_TO_SUB: &str = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";
/// The Rust owner of `render_dockerfile.build_context` (read for the signature atom).
pub const RENDER_TEMPLATE_RS: &str = ".claude/skills/upstream-version-watch/rust/runtime/src/render_template.rs";
const FAILURE_PATTERNS: &str = ".claude/skills/upstream-version-watch/failure_patterns.yaml";
const LEDGER: &str = ".claude/policies/arch_variant_ledger.json";
const WORKFLOW: &str = ".claude/rules/workflow.md";
const DOCKERFILE_TEMPLATE: &str = ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template";
const UPSTREAM_SKILL: &str = ".claude/skills/upstream-version-watch/SKILL.md";
const PROVENANCE: &str = ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md";
const VARIANT: &str = "deepseek-v4-flash-sm12x";

// ---------------------------------------------------------------- small value helpers

fn vs(s: &str) -> V {
    V::Str(s.to_string())
}
fn vd(items: Vec<(&str, V)>) -> V {
    V::Dict(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// `build_context(manifest, resolved)` -> the ordered placeholder map (a failure is the clause's failure).
fn build_context(manifest: &V, resolved: &V) -> R<Vec<(&'static str, String)>> {
    rt::build_context(manifest, resolved, false).map_err(|e| Fail::new(e.class, e.message))
}

fn ctx_get(ctx: &[(&'static str, String)], key: &str) -> R<String> {
    ctx.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).ok_or_else(|| Fail::new("KeyError", repr(key)))
}

fn validated_has(ngc: &str, vllm: &str) -> bool {
    rt::VALIDATED_SOURCE_BUILD_KEYS.iter().any(|(a, b)| *a == ngc && *b == vllm)
}

/// The parameter names of `pub fn <name>(` in a Rust source (the Rust form of `list(inspect.signature(fn).parameters)`).
fn rust_fn_params(src: &str, name: &str) -> R<Vec<String>> {
    let head = format!("pub fn {name}(");
    let start = index(src, &head)? + head.len();
    let end = index_from(src, ")", start)?;
    Ok(src[start..end]
        .split(',')
        .map(|p| strip(p.split(':').next().unwrap_or("")).to_string())
        .filter(|p| !p.is_empty())
        .collect())
}

// ---------------------------------------------------------------- RUNTIME_PATCH_NO_CARRY_FORWARD

pub fn predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C1() -> R {
    let src = read(SYNC_TO_SUB)?;
    let fun = extract_bash_function(&src, "_band2_filters")?;
    crate::claims_text::stem_allowlist(&src)?;
    req(!fun.contains("'/configs/*_patch.py'"), "wildcard runtime-patch transfer is forbidden")?;
    let band2_configs = extract_bash_array(&src, "BAND2_CONFIGS")?;
    req(band2_configs.contains("serve_runner.sh") && band2_configs.contains("debug-init.sh"), "predicate requirement failed at original line 1444")?;
    req(!band2_configs.contains("_patch.py"), "runtime patches must remain separate from generic runner configs")?;

    // "never a derived image": the image-tag/Dockerfile-selection builder takes no model/patch input at all. The Rust
    // owner's parameters are the original's two plus the original's module global ALLOW_UNVALIDATED as an explicit flag.
    let owner = read(RENDER_TEMPLATE_RS)?;
    let params = rust_fn_params(&owner, "build_context")?;
    req(params == ["manifest", "resolved", "allow_unvalidated"], "the image-tag builder must never accept a model/patch parameter -- a model-derived image would require exactly that, and its absence is what forces the stock-image, no-rebuild path")?;
    let sample = build_context(
        &vd(vec![("cpu_arch", vs("aarch64"))]),
        &vd(vec![
            ("vllm_version", vs("0.25.1")),
            ("ngc_base", vd(vec![("tag", vs("26.05-py3")), ("cuda_version", vs("13.2"))])),
            ("build_track", vd(vec![("decision", vs("source-build"))])),
            ("source_build", vd(vec![("torch_cuda_arch", vs("12.1a"))])),
        ]),
    )?;
    let tag = ctx_get(&sample, "IMAGE_TAG")?;
    req(tag == "0.25.1-cu132-aarch64-source", tag.clone())?;
    req(upstream_runtime::render_dockerfile::RUNNER_SCRIPTS.contains(&"arm_patch.sh"), "arm_patch.sh must be one of the generic, model-agnostic runner scripts materialized into the stock image's output pathway regardless of which model is later served")?;

    let arm_src = shared_asset("arm_patch.sh")?;
    req(arm_src.contains("_cfg=\"${RUNTIME_PATCH_STEM:-${CONFIG_FILE:-default}}\"") && arm_src.contains("_patch=\"/app/configs/${_cfg}_patch.py\""), "the patch path must be resolved from a runtime env var, never a name baked in at build time")?;

    let gitignore = read(".gitignore")?;
    let bstart = index(&gitignore, "output/*/*")?;
    let bend = index_from(&gitignore, "\n\n", bstart)?;
    let block = &gitignore[bstart..bend];
    let reincludes = findall(r"(?m)^!(\S+)", block);
    let expected = [
        "output/*/.gitkeep",
        "output/multi/Dockerfile", "output/multi/Dockerfile.source-build",
        "output/multi/Dockerfile.source-build-upstage", "output/multi/docker-compose.yaml",
        "output/multi/requirements.txt",
        "output/single/Dockerfile", "output/single/Dockerfile.source-build",
        "output/single/Dockerfile.source-build-upstage", "output/single/docker-compose.yaml",
        "output/single/requirements.txt",
        "output/*/build_patches/", "output/*/build_patches_src/",
        "output/*/build_patches/.gitkeep", "output/*/build_patches_src/.gitkeep",
    ];
    req(reincludes == expected, format!("unexpected output/ gitignore carve-outs -- must never re-include configs/: {}", repr_list(&reincludes)))?;
    let bp_allowed: BTreeSet<&str> = ["output/*/build_patches/", "output/*/build_patches_src/", "output/*/build_patches/.gitkeep", "output/*/build_patches_src/.gitkeep"].into_iter().collect();
    let bp: BTreeSet<&str> = reincludes.iter().map(String::as_str).filter(|r| r.contains("build_patches")).collect();
    let extra: Vec<&str> = bp.difference(&bp_allowed).copied().collect();
    req(extra.is_empty(), format!("build_patches/ · build_patches_src/ 의 **산출물** 재포함이 되살아났다 — 뼈대(.gitkeep)만 추적한다 (2026-09-10 철회 · 되살리려면 CLAUDE.md 추적 규정부터 고쳐라): {}", repr_list(&extra)))?;
    req(!reincludes.iter().any(|r| r.contains("configs")), "output/<topology>/configs/ (where the compose bind-mount and the runtime patch actually live) must have NO re-inclusion carve-out -- proving it is genuinely, structurally untracked")?;
    let compose = rendered("compose")?;
    req(compose.contains("- ./configs:/app/configs:ro"), "the container must bind-mount the SAME blanket-ignored configs/ directory the patch lives in")?;
    Ok(())
}

pub fn predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C4() -> R {
    req(validated_has("26.05-py3", "0.24.0"), "predicate requirement failed at original line 1614")?;
    let guarded = rt::patch_guard("26.05-py3", "0.24.0", false);
    req(guarded.contains("validated patch set") && !guarded.contains("exit 1"), "predicate requirement failed at original line 1616")?;

    // The frozen set is a compile-time `const` of the owner (the only way to add a key is a source edit) ...
    let owner = read(RENDER_TEMPLATE_RS)?;
    req(search(r"(?m)^pub const VALIDATED_SOURCE_BUILD_KEYS: \[\(&str, &str\); \d+\] = \[", &owner).is_some(), "the validated source-build key set must be a compile-time constant of render_template.rs -- freezing a key must be a source edit")?;
    let before: Vec<(&str, &str)> = rt::VALIDATED_SOURCE_BUILD_KEYS.to_vec();
    // ... and the HITL override flag (the original's ALLOW_UNVALIDATED = True) only warns.
    let warned = rt::patch_guard("99.99-py3", "9.9.9", true);
    req(warned.contains("UNVALIDATED") && warned.contains("HITL"), "predicate requirement failed at original line 1623")?;
    req(rt::VALIDATED_SOURCE_BUILD_KEYS.to_vec() == before, "even an approved attempt-build must never silently mutate the frozen validated set")?;
    let default_refused = rt::patch_guard("99.99-py3", "9.9.9", false);
    req(default_refused.trim_end().ends_with("exit 1"), "predicate requirement failed at original line 1629")?;

    let dockerfile = rendered("source-build")?;
    let from_idx = index(&dockerfile, "FROM nvcr.io/nvidia/pytorch:")?;
    let from_line_end = index_from(&dockerfile, "\n", from_idx)?;
    let from_line = &dockerfile[from_idx..from_line_end];
    let ngc_tag = strip(from_line.splitn(2, ':').nth(1).unwrap_or("")).to_string();
    let guard_idx = index_from(&dockerfile, "RUN echo \"[guard] source-build key", from_line_end)?;
    let next_run_idx = index_from(&dockerfile, "\nRUN ", guard_idx + 1)?;
    let guard_block = &dockerfile[guard_idx..next_run_idx];
    req(count(guard_block, "RUN echo") == 1, "the frozen guard must be the sole RUN instruction between FROM and the first real build step -- matching the template's own FROM-then-guard placement contract")?;
    req(!guard_block.contains("exit 1"), "the guard actually baked into the tracked, buildable Dockerfile must be a validated/proceeding guard, not a refusal -- proving THIS specific combo was really graduated, not merely theorized in the abstract Python catalog")?;
    let vref = index(&dockerfile, "ARG VLLM_REF=v")?;
    let vref_end = index_from(&dockerfile, "\n", vref)?;
    let line = &dockerfile[vref..vref_end];
    let tracked_vllm = strip(line.splitn(2, "=v").nth(1).unwrap_or("")).to_string();
    req(validated_has(&ngc_tag, &tracked_vllm), format!("tracked source-build key ({}, {}) must be graduated in the production catalog", repr(&ngc_tag), repr(&tracked_vllm)))?;
    let exact = rt::patch_guard(&ngc_tag, &tracked_vllm, false);
    req(exact.contains("validated patch set") && !exact.contains("exit 1"), "production guard must accept the exact tracked Dockerfile source-build pair")?;
    Ok(())
}

// ---------------------------------------------------------------- ARCH_WALL_VARIANT_LADDER fixture

/// `_arch_contract_repo(tmp)`: the arch-contract inputs copied into `tmp`. The slave-forward owner file is the one the
/// auditor itself gates on (`policy_audit::arch::SLAVE_FORWARD_REL`), so the fixture follows the auditor.
fn arch_contract_repo(tmp: &Path) -> R<PathBuf> {
    let root = tmp.to_path_buf();
    let rels = [
        LEDGER,
        DOCKERFILE_TEMPLATE,
        ".claude/policies/arch_variant_evidence/source-sm12x-vllm-0.23.0-approval.json",
        PROVENANCE,
        ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh",
        policy_audit::arch::SLAVE_FORWARD_REL,
        policy_audit::arch::COMPOSE_MULTI_TEMPLATE_REL,
        WORKFLOW,
        UPSTREAM_SKILL,
    ];
    for rel in rels {
        let dst = root.join(rel);
        if let Some(d) = dst.parent() {
            mkdirs(d)?;
        }
        std::fs::copy(repo_root().join(rel), &dst).map_err(|e| Fail::new("FileNotFoundError", format!("{e}: '{}'", repo_root().join(rel).display())))?;
    }
    git(&root, &["init", "-q"])?;
    Ok(root)
}

/// `_arch_codes(root)`: the auditor's reason codes, with the fixture's index = the bytes on disk.
fn arch_codes(root: &Path) -> R<BTreeSet<String>> {
    git(root, &["add", "-A"])?;
    Ok(policy_audit::arch::arch_variant_contract_violations(root).into_iter().map(|v| v.reason_code).collect())
}

fn has(root: &Path, code: &str) -> R<bool> {
    Ok(arch_codes(root)?.contains(code))
}

fn load_json_path(path: &Path) -> R<PyValue> {
    json_loads(&read_path(path)?)
}

fn dumps(v: &PyValue) -> R<String> {
    v.dumps_ascii().map_err(|e| Fail::new("TypeError", format!("{e:?}")))
}

fn dumps_sorted(v: &PyValue) -> R<String> {
    dumps(&sort_keys(v))
}

fn sort_keys(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut items: Vec<(PyStr, PyValue)> = d.iter().map(|(k, v)| (k.clone(), sort_keys(v))).collect();
            items.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, v) in items {
                out.insert(k, v);
            }
            PyValue::Dict(out)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

fn dict_mut(v: &mut PyValue) -> R<&mut PyDict> {
    match v {
        PyValue::Dict(d) => Ok(d),
        _ => Err(Fail::new("TypeError", "not a dict")),
    }
}

fn dget<'a>(v: &'a PyValue, k: &str) -> R<&'a PyValue> {
    jget(v, k)
}

fn ps(s: &str) -> PyValue {
    PyValue::str(s)
}

/// The ledger document of the fixture and a handle on one variant entry.
struct Ledger {
    root: PathBuf,
    doc: PyValue,
}

impl Ledger {
    fn load(root: &Path) -> R<Ledger> {
        Ok(Ledger { root: root.to_path_buf(), doc: load_json_path(&root.join(LEDGER))? })
    }
    fn variants(&mut self) -> R<&mut PyDict> {
        let d = dict_mut(&mut self.doc)?;
        dict_mut(d.get_mut("source_build_variants").ok_or_else(|| Fail::new("KeyError", "'source_build_variants'"))?)
    }
    fn entry(&mut self, id: &str) -> R<&mut PyDict> {
        dict_mut(self.variants()?.get_mut(id).ok_or_else(|| Fail::new("KeyError", repr(id)))?)
    }
    fn entry_value(&self, id: &str) -> R<PyValue> {
        Ok(dget(dget(&self.doc, "source_build_variants")?, id)?.clone())
    }
    fn save(&self) -> R {
        write(&self.root.join(LEDGER), &dumps(&self.doc)?)
    }
}

/// `PyDict` with one key removed (`dict.pop(k, None)`), insertion order kept.
fn without(d: &PyDict, key: &str) -> PyDict {
    let mut out = PyDict::new();
    for (k, v) in d.iter() {
        if !k.equals(key) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn pop(d: &mut PyDict, key: &str) {
    *d = without(d, key);
}

fn entry_str(e: &PyDict, k: &str) -> R<PyValue> {
    e.get(k).cloned().ok_or_else(|| Fail::new("KeyError", repr(k)))
}

/// `_write_arch_regression_artifact(root, entry)`.
fn write_arch_regression_artifact(root: &Path, entry: &mut PyDict) -> R {
    let rel = ".claude/policies/arch_variant_evidence/test-regression.json";
    let mut a = PyDict::new();
    a.set("schema_version", PyValue::int(1));
    a.set("kind", ps("arch_variant_regression"));
    a.set("result", ps("PASS"));
    for k in ["variant_id", "vllm_repo", "vllm_ref", "track", "architecture", "image_tag"] {
        a.set(k, entry_str(entry, k)?);
    }
    a.set("existing_models", PyValue::List(vec![ps("fixture-existing-model")]));
    write(&root.join(rel), &dumps_sorted(&PyValue::Dict(a))?)?;
    let mut ptr = PyDict::new();
    ptr.set("path", ps(rel));
    entry.set("regression_evidence", PyValue::Dict(ptr));
    Ok(())
}

/// `_mutate_arch_artifact(root, entry, field, mutate)`.
fn mutate_arch_artifact(root: &Path, entry: &PyValue, field: &str, mutate: impl FnOnce(&mut PyDict)) -> R {
    let rel = jstr(dget(dget(entry, field)?, "path")?).ok_or_else(|| Fail::new("TypeError", "pointer path"))?;
    let path = root.join(rel);
    let mut artifact = load_json_path(&path)?;
    mutate(dict_mut(&mut artifact)?);
    write(&path, &dumps_sorted(&artifact)?)
}

fn replace_in(path: &Path, old: &str, new: &str) -> R {
    let text = read_path(path)?;
    write(path, &text.replace(old, new))
}

// ---------------------------------------------------------------- ARCH_WALL_VARIANT_LADDER

/// `classify(text)` over the owner's `load_patterns` + Python-`re` matching (`classify_failure.matched_lines`).
fn classify(patterns: &[PyDict], text: &str) -> R<String> {
    for p in patterns {
        let sig = p.get("signature").cloned().unwrap_or(PyValue::None);
        let m = upstream_runtime::classify_source::matched_lines(&ps(text), &sig, &PyValue::int(5), &PyValue::int(1000))
            .map_err(|e| Fail::new("re.error", format!("{e:?}")))?;
        // `matched_lines` returns the matched log lines joined by "\n" -- non-empty iff `re.search` found the signature
        // on a non-blank line (every registry signature matches visible text).
        let hit = match m.value.peel() {
            PyValue::Str(s) => !s.is_empty(),
            _ => return Err(Fail::new("TypeError", "matched_lines must return a str")),
        };
        if hit {
            return Ok(p.get("class").and_then(jstr).unwrap_or_default());
        }
    }
    Ok("unknown".into())
}

pub fn predicate_ARCH_WALL_VARIANT_LADDER_C1() -> R {
    let path = repo_root().join(FAILURE_PATTERNS);
    let loaded = upstream_runtime::classify_source::load_patterns(&ps(&path.to_string_lossy()))
        .map_err(|e| Fail::new("OSError", format!("load_patterns failed: {e:?}")))?;
    let patterns = loaded.patterns;
    req(classify(&patterns, "ModuleNotFoundError: No module named 'uvloop'")? == "requirements-fixable", "predicate requirement failed at original line 2753")?;
    req(classify(&patterns, "No module named 'torch._opaque_base'")? == "source-build-class", "predicate requirement failed at original line 2754")?;
    let tmp = TempDir::new("arch-c1-")?;
    let root = arch_contract_repo(tmp.path())?;
    req(arch_codes(&root)?.is_empty(), "predicate requirement failed at original line 2757")?;
    let workflow = root.join(WORKFLOW);
    replace_in(&workflow,
        "deps-패치 → 소스-게이트 패치 → **자체 이식** → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체",
        "deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체")?;
    req(has(&root, "ARCH_VARIANT_LADDER_ORDER_MISSING")?, "predicate requirement failed at original line 2762")?;
    write(&workflow, &read(WORKFLOW)?)?;
    replace_in(&root.join(DOCKERFILE_TEMPLATE), "checkout --detach ${VLLM_REF}", "checkout main")?;
    req(has(&root, "ARCH_VARIANT_SOURCE_OVERRIDE_UNWIRED")?, "predicate requirement failed at original line 2766")?;
    Ok(())
}

pub fn predicate_ARCH_WALL_VARIANT_LADDER_C2() -> R {
    let resolved = vd(vec![
        ("vllm_version", vs("0.25.1")),
        ("torch", vd(vec![("pin", vs("2.11.0"))])),
        ("ngc_base", vd(vec![("tag", vs("26.05-py3")), ("cuda_version", vs("13.2"))])),
        ("build_track", vd(vec![("decision", vs("source-build"))])),
        ("wheel", vd(vec![])),
        ("source_build", vd(vec![("torch_cuda_arch", vs("12.1a"))])),
    ]);
    let manifest = vd(vec![("cpu_arch", vs("aarch64")), ("nas_model_path", vs(""))]);
    let tag = ctx_get(&build_context(&manifest, &resolved)?, "IMAGE_TAG")?;
    req(tag == "0.25.1-cu132-aarch64-source", "predicate requirement failed at original line 2776")?;
    let with_model = vd(vec![("cpu_arch", vs("aarch64")), ("nas_model_path", vs("")), ("model_name", vs("deepseek-v4-flash"))]);
    req(ctx_get(&build_context(&with_model, &resolved)?, "IMAGE_TAG")? == tag, "predicate requirement failed at original line 2778")?;

    let tmp = TempDir::new("arch-c2-")?;
    let root = arch_contract_repo(tmp.path())?;
    let mut l = Ledger::load(&root)?;
    l.entry(VARIANT)?.set("image_tag", ps("easy-vllm:deepseek-v4-flash-source-sm12x"));
    l.save()?;
    req(has(&root, "ARCH_VARIANT_MODEL_KEYED_IMAGE")?, "predicate requirement failed at original line 2785")?;
    // `doc["source_build_variants"]["track-sm12x"] = doc[...].pop("deepseek-v4-flash-sm12x")`
    let moved = l.entry_value(VARIANT)?;
    {
        let vars = l.variants()?;
        pop(vars, VARIANT);
        vars.set("track-sm12x", moved);
    }
    l.save()?;
    req(has(&root, "ARCH_VARIANT_MODEL_KEYED_IMAGE")?, "renaming the attacker-controlled ledger key must not hide a model-keyed image")?;
    l.entry("track-sm12x")?.set("image_tag", ps("easy-vllm:0.23.0-cu132-aarch64-source-wrong"));
    l.save()?;
    req(has(&root, "ARCH_VARIANT_NOT_SUPERSET_TAG")?, "predicate requirement failed at original line 2794")?;
    Ok(())
}

pub fn predicate_ARCH_WALL_VARIANT_LADDER_C3() -> R {
    {
        let tmp = TempDir::new("arch-c3a-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        {
            let base = l.entry(VARIANT)?;
            base.set("status", ps("VALIDATED"));
            write_arch_regression_artifact(&root, base)?;
        }
        l.save()?;
        req(!has(&root, "ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS")?, "predicate requirement failed at original line 2806")?;
        let mut other = match l.entry_value(VARIANT)?.peel() {
            PyValue::Dict(d) => d.clone(),
            _ => return Err(Fail::new("TypeError", "entry")),
        };
        other.set("image_tag", ps("easy-vllm:0.23.0-cu132-aarch64-source-sm121a"));
        other.set("track", ps("source-sm121a"));
        l.variants()?.set("other-model-sm121a", PyValue::Dict(other));
        l.save()?;
        req(has(&root, "ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS")?, "predicate requirement failed at original line 2810")?;
    }
    {
        let tmp = TempDir::new("arch-c3b-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        let mut hidden = match l.entry_value(VARIANT)?.peel() {
            PyValue::Dict(d) => d.clone(),
            _ => return Err(Fail::new("TypeError", "entry")),
        };
        pop(l.variants()?, VARIANT);
        hidden.set("status", ps("VALIDATED"));
        pop(&mut hidden, "regression_evidence");
        hidden.set("image_tag", ps("easy-vllm:deepseek-v4-flash-source-sm12x"));
        l.variants()?.set("_hidden_active_variant", PyValue::Dict(hidden));
        l.save()?;
        req(has(&root, "ARCH_VARIANT_METADATA_KEY_INVALID")?, "predicate requirement failed at original line 2821")?;
    }
    {
        let tmp = TempDir::new("arch-c3c-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        {
            let e = l.entry(VARIANT)?;
            e.set("status", ps("UNVALIDATED"));
            write_arch_regression_artifact(&root, e)?;
        }
        l.save()?;
        req(has(&root, "ARCH_VARIANT_ACTIVE_NOT_VALIDATED")?, "predicate requirement failed at original line 2830")?;
    }
    {
        let tmp = TempDir::new("arch-c3d-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        for bad in ["SUPERSEDED_PENDING", "SUPERSEDED", "SUPERSEDED@0.24", "SUPERSEDED@0.24.0.1"] {
            {
                let e = l.entry(VARIANT)?;
                e.set("status", ps(bad));
                pop(e, "regression_evidence");
            }
            l.save()?;
            let codes = arch_codes(&root)?;
            req(codes.contains("ARCH_VARIANT_STATUS_INVALID"), format!("{} must be refused as a malformed retirement marker", repr(bad)))?;
            req(codes.contains("ARCH_VARIANT_ACTIVE_NOT_VALIDATED"), format!("{} must fall through to the active-track gate, never be silently retired", repr(bad)))?;
        }
        l.entry(VARIANT)?.set("status", ps("SUPERSEDED@0.24.0; whatever prose the maintainer chooses to write here"));
        l.save()?;
        let codes = arch_codes(&root)?;
        req(!codes.contains("ARCH_VARIANT_STATUS_INVALID"), "a well-formed retirement marker must be admitted regardless of its trailing prose")?;
        req(!codes.contains("ARCH_VARIANT_ACTIVE_NOT_VALIDATED"), "a retired variant must not be evaluated as an active track")?;
    }
    {
        let tmp = TempDir::new("arch-c3e-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        {
            let d = dict_mut(&mut l.doc)?;
            d.set("schema_version", PyValue::List(vec![]));
            d.set("unexpected", PyValue::Bool(true));
        }
        l.entry(VARIANT)?.set("unexpected", PyValue::Bool(true));
        l.save()?;
        let codes = arch_codes(&root)?;
        req(codes.contains("ARCH_VARIANT_LEDGER_SHAPE_INVALID"), "predicate requirement failed at original line 2856")?;
        req(codes.contains("ARCH_VARIANT_ENTRY_UNKNOWN_FIELD"), "predicate requirement failed at original line 2857")?;
    }
    Ok(())
}

/// One fixture whose variant's approval artifact is rewritten by `mutate`; returns whether `code` fires.
fn approval_mutation_fires(tag: &str, code: &str, mutate: impl FnOnce(&mut PyDict)) -> R<bool> {
    let tmp = TempDir::new(tag)?;
    let root = arch_contract_repo(tmp.path())?;
    let l = Ledger::load(&root)?;
    let entry = l.entry_value(VARIANT)?;
    mutate_arch_artifact(&root, &entry, "evidence", mutate)?;
    l.save()?;
    has(&root, code)
}

pub fn predicate_ARCH_WALL_VARIANT_LADDER_C4() -> R {
    let guard = rt::patch_guard("99.99-py3", "9.9.9", false);
    req(guard.contains("HITL discovery loop"), "predicate requirement failed at original line 2863")?;
    req(guard.trim_end().ends_with("exit 1"), "predicate requirement failed at original line 2864")?;
    req(guard.contains("UNVALIDATED, not IMPOSSIBLE"), "predicate requirement failed at original line 2865")?;
    {
        let tmp = TempDir::new("arch-c4a-")?;
        let root = arch_contract_repo(tmp.path())?;
        let mut l = Ledger::load(&root)?;
        let original_pointer = entry_str(l.entry(VARIANT)?, "evidence")?;
        let mut missing = PyDict::new();
        missing.set("path", ps("missing-approval.json"));
        l.entry(VARIANT)?.set("evidence", PyValue::Dict(missing));
        l.save()?;
        req(has(&root, "ARCH_VARIANT_ARTIFACT_UNBOUND")?, "predicate requirement failed at original line 2873")?;

        l.entry(VARIANT)?.set("evidence", original_pointer.clone());
        let apath = root.join(jstr(dget(&original_pointer, "path")?).unwrap_or_default());
        let mut artifact = load_json_path(&apath)?;
        dict_mut(&mut artifact)?.set("result", ps("FAIL"));
        write(&apath, &dumps_sorted(&artifact)?)?;
        l.save()?;
        req(has(&root, "ARCH_VARIANT_ARTIFACT_BINDING_MISMATCH")?, "predicate requirement failed at original line 2893")?;

        l.entry(VARIANT)?.set("evidence", ps(""));
        l.save()?;
        req(has(&root, "ARCH_VARIANT_ARTIFACT_POINTER_INVALID")?, "predicate requirement failed at original line 2897")?;
        replace_in(&root.join(WORKFLOW), "testlog 기록 + 사람 승인", "사람 승인")?;
        req(has(&root, "ARCH_VARIANT_HITL_EVIDENCE_ORDER_INVALID")?, "predicate requirement failed at original line 2900")?;
    }
    req(approval_mutation_fires("arch-c4b-", "ARCH_VARIANT_APPROVAL_IDENTITY_MISSING", |a| a.set("approved_by", ps("HITL:AUTOMATION:unattended")))?,
        "predicate requirement failed at original line 2909")?;
    req(approval_mutation_fires("arch-c4c-", "ARCH_VARIANT_APPROVAL_GROUNDING_MISSING", |a| {
        a.set("source_evidence", ps("x"));
        a.set("approved_scope", ps("x"));
    })?, "predicate requirement failed at original line 2920")?;
    for (field, value) in [
        ("source_evidence", "plan_26062818 §999 R999 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
        ("approved_scope", "This explicitly denies that serving smoke remains final arbitration for this variant."),
    ] {
        req(approval_mutation_fires("arch-c4d-", "ARCH_VARIANT_APPROVAL_GROUNDING_MISSING", |a| a.set(field, ps(value)))?,
            "predicate requirement failed at original line 2933")?;
    }
    {
        let tmp = TempDir::new("arch-c4e-")?;
        let root = arch_contract_repo(tmp.path())?;
        std::fs::remove_file(root.join(PROVENANCE)).map_err(|e| Fail::new("OSError", e.to_string()))?;
        req(has(&root, "ARCH_VARIANT_APPROVAL_GROUNDING_MISSING")?, "predicate requirement failed at original line 2938")?;
    }
    req(approval_mutation_fires("arch-c4f-", "ARCH_VARIANT_ARTIFACT_SHAPE_INVALID", |a| {
        pop(a, "schema_version");
        a.set("unexpected", PyValue::Bool(true));
    })?, "predicate requirement failed at original line 2949")?;
    Ok(())
}

pub fn predicate_ARCH_WALL_VARIANT_LADDER_C5() -> R {
    let tmp = TempDir::new("arch-c5-")?;
    let root = arch_contract_repo(tmp.path())?;
    let mut l = Ledger::load(&root)?;
    {
        let e = l.entry(VARIANT)?;
        e.set("status", ps("VALIDATED"));
        pop(e, "regression_evidence");
    }
    l.save()?;
    req(has(&root, "ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING")?, "predicate requirement failed at original line 2961")?;
    write_arch_regression_artifact(&root, l.entry(VARIANT)?)?;
    l.save()?;
    req(!has(&root, "ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING")?, "predicate requirement failed at original line 2964")?;
    let entry = l.entry_value(VARIANT)?;
    let rpath = root.join(jstr(dget(dget(&entry, "regression_evidence")?, "path")?).unwrap_or_default());
    let mut regression = load_json_path(&rpath)?;
    dict_mut(&mut regression)?.set("existing_models", PyValue::List(vec![]));
    write(&rpath, &dumps_sorted(&regression)?)?;
    l.save()?;
    req(has(&root, "ARCH_VARIANT_REGRESSION_MODELS_MISSING")?, "predicate requirement failed at original line 2982")?;
    mutate_arch_artifact(&root, &entry, "regression_evidence", |a| a.set("existing_models", PyValue::Bool(true)))?;
    l.save()?;
    req(has(&root, "ARCH_VARIANT_REGRESSION_MODELS_MISSING")?, "predicate requirement failed at original line 2986")?;
    replace_in(&root.join(UPSTREAM_SKILL), "기존모델 회귀 재스모크", "기존모델 확인")?;
    req(has(&root, "ARCH_VARIANT_PROMOTION_GUARDS_MISSING")?, "predicate requirement failed at original line 2989")?;
    Ok(())
}
