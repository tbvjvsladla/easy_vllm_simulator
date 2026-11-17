//! `recipe.py estimate` port: the Flag gate runs before the config is read (exit 4 on a tree with no manifest), the unported
//! sub-commands stop with status 2 and a named message, and an empty command line is a usage error.

use agent_runtime_support::pystr::PyStr;
use recipe_runtime::recipe_cmd::recipe_main;

fn args(items: &[&str]) -> Vec<PyStr> {
    items.iter().map(|a| PyStr::new(a)).collect()
}

/// A skill root three directories below a directory that is not a git tree, so the repo root falls back to that directory
/// (the original's abspath 3-up) and holds no `output/<topology>/manifest.yaml`.
fn bare_skill_root() -> (std::path::PathBuf, PyStr) {
    // One directory per call: the tests run in parallel in one process and each removes its own base (a shared base raced).
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("recipe_cmd_cases_{}_{n}", std::process::id()));
    let skill = base.join("x").join("y").join("z");
    std::fs::create_dir_all(&skill).unwrap();
    let text = skill.to_str().unwrap().to_string();
    (base, PyStr::new(&text))
}

#[test]
fn flag_gate_precedes_config_load() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["estimate", "--auto", "--config", "does/not/exist.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 4);
    assert!(run.stderr.to_utf8().unwrap().contains("manifest 부재"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn no_subcommand_stops_with_the_unported_message() {
    let (base, root) = bare_skill_root();
    for name in ["simulate", "estimate", "generate"] {
        let run = recipe_main(&args(&[name]), Some("80"), &root);
        assert_eq!(run.status, 2);
        assert!(!run.stderr.to_utf8().unwrap().contains("미이식"), "{name}");
    }
    let _ = std::fs::remove_dir_all(base);
}

/// Reference: `python3 recipe.py simulate` / `simulate --config x.yaml` (COLUMNS=80) end with this error line and exit 2.
#[test]
fn simulate_argument_errors_match_the_original() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["simulate"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let err = run.stderr.to_utf8().unwrap();
    assert!(err.contains("recipe.py simulate: error: the following arguments are required: --config, --candidate"), "{err}");
    let run = recipe_main(&args(&["simulate", "--config", "x.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let err = run.stderr.to_utf8().unwrap();
    assert!(err.contains("recipe.py simulate: error: the following arguments are required: --candidate"), "{err}");
    let _ = std::fs::remove_dir_all(base);
}

/// Reference: `python3 recipe.py generate` / `generate --config x.yaml` (COLUMNS=80) print the argparse usage plus this error
/// line and exit 2; the Flag gate does not run before argument parsing.
#[test]
fn generate_argument_errors_match_the_original() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["generate"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let err = run.stderr.to_utf8().unwrap();
    assert!(err.contains("recipe.py generate: error: the following arguments are required: --config, --recipe-id"), "{err}");
    let run = recipe_main(&args(&["generate", "--config", "x.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let err = run.stderr.to_utf8().unwrap();
    assert!(err.contains("recipe.py generate: error: the following arguments are required: --recipe-id"), "{err}");
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn missing_command_is_a_usage_error() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&[], Some("80"), &root);
    assert_eq!(run.status, 2);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn estimate_requires_a_candidate_source() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["estimate", "--config", "c.yaml"]), Some("80"), &root);
    assert_eq!(run.status, 2);
    let _ = std::fs::remove_dir_all(base);
}

// ── plan_26111708 B L1 · L4 · L10 ────────────────────────────────────────────────────────────────────────────────────────

/// L1: on a terraformed tree a real trial (no `--dry-run`, no `--mock-profile`) without `--image` fails loudly (exit 2) and
/// names what to pass; the docker-free paths and a named image are never refused for the image. On an unterraformed tree the
/// Flag gate answers first (exit 4, policy:TERRAFORM_FLAG_GATE) whatever the image.
#[test]
fn simulate_without_an_image_fails_loudly_and_only_for_a_real_trial() {
    let (base, root) = bare_skill_root();
    let run = recipe_main(&args(&["simulate", "--config", "x.yaml", "--candidate", "c.json"]), Some("80"), &root);
    assert_eq!(run.status, 4, "the Flag gate precedes the image check: {}", run.stderr.to_utf8().unwrap());
    let _ = std::fs::remove_dir_all(base);

    let tree = GenerateTree::new("l1-image", "single", 0);
    let simulate = |extra: &[&str]| {
        let mut argv = vec!["simulate", "--config", tree.config.as_str(), "--candidate", "c.json"];
        argv.extend_from_slice(extra);
        recipe_main(&args(&argv), Some("80"), &tree.skill)
    };
    let run = simulate(&[]);
    assert_eq!(run.status, 2);
    let err = run.stderr.to_utf8().unwrap();
    assert!(err.contains("--image 미지정") && err.contains("--image <IMAGE_TAG>"), "{err}");
    let run = simulate(&["--image", ""]);
    assert_eq!(run.status, 2, "an empty image is no image");
    // Only the docker-free paths here: a named image on a terraformed tree would go on to a real trial (never in a test).
    for extra in [&["--dry-run"][..], &["--mock-profile", "m.json"][..]] {
        let run = simulate(extra);
        let err = run.stderr.to_utf8().unwrap();
        assert!(!err.contains("--image 미지정"), "{extra:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&tree.base);
}

/// A terraformed tree for `recipe generate`: the manifest (in the channel `topology_of` picks off-git: `output/single`), a model
/// directory, a config and the estimate snapshot with one ranked candidate.
struct GenerateTree {
    base: std::path::PathBuf,
    skill: PyStr,
    config: String,
}

impl GenerateTree {
    fn new(label: &str, manifest_topology: &str, nodes: usize) -> GenerateTree {
        let base = std::env::temp_dir().join(format!("recipe_cmd_generate_{}_{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let skill = base.join("x").join("y").join("z");
        std::fs::create_dir_all(skill.join("feedback")).unwrap();
        std::fs::create_dir_all(base.join("output/single")).unwrap();
        let mut manifest = format!(
            "topology: {manifest_topology}\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nmodel_source: managed\nterraforming:\n  complete: true\n  branch_verified: true\n"
        );
        if nodes > 0 {
            manifest.push_str("nodes:\n");
            for index in 0..nodes {
                manifest.push_str(&format!("  - role: {}\n", if index == 0 { "main" } else { "sub" }));
            }
        }
        std::fs::write(base.join("output/single/manifest.yaml"), manifest).unwrap();
        let model = base.join("nas/Org/Model");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(
            model.join("config.json"),
            r#"{"model_type": "llama", "architectures": ["LlamaForCausalLM"], "hidden_size": 64, "num_hidden_layers": 2,
               "num_attention_heads": 4, "num_key_value_heads": 2, "vocab_size": 128, "max_position_embeddings": 4096,
               "torch_dtype": "bfloat16"}"#,
        )
        .unwrap();
        let config = base.join("config.yaml");
        std::fs::write(
            &config,
            format!(
                r#"{{"target_model": {{"path": "/app/models/Org/Model"}}, "nas_host_root": "{}", "vram_budget_gb": 96,
                    "serving": {{"config_name": "cell", "port": 18080, "served_model_name": "served"}}}}"#,
                base.join("nas").display()
            ),
        )
        .unwrap();
        std::fs::write(
            skill.join("feedback/.last_ranking.json"),
            r#"{"model_id": "Org/Model", "tp": 2, "result": {"ranked": [{"id": "r1", "quantization": null, "max_model_len": 4096, "gpu_memory_utilization": 0.9}]}}"#,
        )
        .unwrap();
        let skill = PyStr::new(skill.to_str().unwrap());
        GenerateTree { base, skill, config: config.to_str().unwrap().to_string() }
    }

    fn generate(&self) -> recipe_runtime::cli::Run {
        recipe_main(
            &args(&["generate", "--config", &self.config, "--recipe-id", "r1", "--image", "easy-vllm:0.32.0-cu133-aarch64-source"]),
            Some("80"),
            &self.skill,
        )
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.base.join("output/single").join(relative)).unwrap_or_default()
    }
}

impl Drop for GenerateTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// L4 · L10 (single): the yaml listens on the compose container port, the env keeps the host port, one serving container.
#[test]
fn generate_on_a_single_manifest_listens_on_the_container_port() {
    let tree = GenerateTree::new("single", "single", 0);
    let run = tree.generate();
    assert_eq!(run.status, 0, "{}", run.stderr.to_utf8().unwrap());
    let yaml = tree.read("configs/cell.yaml");
    let env = tree.read("envs/.env.cell");
    assert!(yaml.lines().any(|l| l == "port: 8000"), "{yaml}");
    assert!(!yaml.contains("tensor-parallel-size") && !yaml.contains("distributed-executor-backend"), "{yaml}");
    assert!(env.lines().any(|l| l == "SERVING_PORT=18080") && env.lines().any(|l| l == "CONTAINER_NAME=cell-serving-container"), "{env}");
    assert!(!env.contains("MASTER_CONTAINER_NAME") && !env.contains("TIKTOKEN_ENABLED"), "{env}");
}

/// L10 (multi): the manifest's topology (not the branch) gives TP/Ray, the host-network port and the multi env keys.
#[test]
fn generate_on_a_multi_manifest_emits_the_distributed_shape() {
    let tree = GenerateTree::new("multi", "multi", 2);
    let run = tree.generate();
    assert_eq!(run.status, 0, "{}", run.stderr.to_utf8().unwrap());
    let yaml = tree.read("configs/cell.yaml");
    let env = tree.read("envs/.env.cell");
    for line in ["port: 18080", "tensor-parallel-size: 2", "distributed-executor-backend: ray"] {
        assert!(yaml.lines().any(|l| l == line), "{line}\n{yaml}");
    }
    for line in [
        "SERVING_PORT=18080",
        "MASTER_CONTAINER_NAME=cell-master-container",
        "SLAVE_CONTAINER_NAME=cell-slave-container",
        "IMAGE_TAG=easy-vllm:0.32.0-cu133-aarch64-source",
        "BUILD_DOCKERFILE=Dockerfile.source-build",
        "VLLM_REF=v0.32.0",
    ] {
        assert!(env.lines().any(|l| l == line), "{line}\n{env}");
    }
    assert!(!env.contains("TIKTOKEN_ENABLED") && !env.lines().any(|l| l.starts_with("CONTAINER_NAME=")), "{env}");
}

/// L10 negatives: a topology the manifest does not name as single/multi, and a multi manifest whose TP is 1, stop with 5 and
/// write nothing (no silent single).
#[test]
fn generate_refuses_a_topology_it_cannot_shape() {
    for (label, topology, nodes) in [("unknown", "dual", 0), ("multi-tp1", "multi", 0)] {
        let tree = GenerateTree::new(label, topology, nodes);
        // The multi-tp1 snapshot must not carry a TP of its own: TP comes from the manifest (nodes × gpus_per_node).
        if label == "multi-tp1" {
            let snapshot = std::path::Path::new(tree.skill.to_utf8().unwrap().as_str()).join("feedback/.last_ranking.json");
            std::fs::write(snapshot, r#"{"model_id": "Org/Model", "result": {"ranked": [{"id": "r1", "quantization": null, "max_model_len": 4096, "gpu_memory_utilization": 0.9}]}}"#).unwrap();
        }
        let run = tree.generate();
        assert_eq!(run.status, 5, "{label}: {}", run.stderr.to_utf8().unwrap());
        assert!(tree.read("configs/cell.yaml").is_empty(), "{label}");
    }
}
