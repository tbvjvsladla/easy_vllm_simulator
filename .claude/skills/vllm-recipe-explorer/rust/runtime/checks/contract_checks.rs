//! Contract checks of the recipe-runtime bins (plan 26111805 「보존 계약」, cell c21b): what their consumers read
//! (runtime-support/contract/consumers.tsv L1·L2 rows of this crate) —
//! - every consumer and document starts a bin through its launcher in `scripts/`; the launchers honour
//!   `EASY_VLLM_RECIPE_RUNTIME_ROOT` (smoke_clone.sh) and refuse an absent executable with exit 2;
//! - the skill documents name subcommands and long options (`recipe.sh {estimate,generate,simulate}`, …);
//! - policy-distribution `checks.rs` runs the self-tests (`--self-test`, `selftest_gmu_roles_batch.sh` without
//!   arguments, `gen_recipe_set.sh --check-parity`) and needs exit 0;
//! - policy-distribution, policy-predicates `claims_tf.rs` and upstream `smoke_clone.sh`: `recipe.sh estimate|generate|
//!   simulate` in an unterraformed tree is exit 4 (the Flag gate fires before the config is read);
//! - adversarial-benchmark `campaign_write.rs`: `escalation_predicate --symptom … --json` exits 0 or 1 (both are
//!   verdicts) with the predicate keys on stdout;
//! - upstream `smoke_model.rs`: `rope_scaling_translate --model-dir --target-len --json` keys.
//!
//! The bins run for real (through the launchers where the consumers do) in scratch directories. Values (L2) and exit
//! codes, option names and marker lines (L1) are fixed; message texts are not (L3).
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use json::Json;

/// `.claude/skills/vllm-recipe-explorer` (this crate is its `rust/runtime/`).
fn skill() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("recipe-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// The launchers of `scripts/` and the bin each one starts (`target/release/<bin>` of the runtime root).
const LAUNCHERS: [(&str, &str); 17] = [
    ("cleanup_docker.sh", "cleanup_docker"),
    ("crosscheck_model_card.sh", "crosscheck_model_card"),
    ("escalation_predicate.sh", "escalation_predicate"),
    ("estimate_vram.sh", "estimate_vram"),
    ("feedback_log.sh", "feedback_log"),
    ("functional_smoke.sh", "functional_smoke"),
    ("gen_recipe_set.sh", "gen_recipe_set"),
    ("parse_model_config.sh", "parse_model_config"),
    ("parse_vllm_log.sh", "parse_vllm_log"),
    ("preload_ram_gate.sh", "preload_ram_gate"),
    ("quant_table.sh", "quant_table"),
    ("rank_recipes.sh", "rank_recipes"),
    ("recipe.sh", "recipe"),
    ("rope_scaling_translate.sh", "rope_scaling_translate"),
    ("run_trial.sh", "run_trial"),
    ("selftest_gmu_roles_batch.sh", "selftest_gmu_roles_batch"),
    ("sim_classify.sh", "sim_classify"),
];

/// This build's executable of a launcher's bin.
fn exe(bin: &str) -> &'static str {
    match bin {
        "cleanup_docker" => env!("CARGO_BIN_EXE_cleanup_docker"),
        "crosscheck_model_card" => env!("CARGO_BIN_EXE_crosscheck_model_card"),
        "escalation_predicate" => env!("CARGO_BIN_EXE_escalation_predicate"),
        "estimate_vram" => env!("CARGO_BIN_EXE_estimate_vram"),
        "feedback_log" => env!("CARGO_BIN_EXE_feedback_log"),
        "functional_smoke" => env!("CARGO_BIN_EXE_functional_smoke"),
        "gen_recipe_set" => env!("CARGO_BIN_EXE_gen_recipe_set"),
        "parse_model_config" => env!("CARGO_BIN_EXE_parse_model_config"),
        "parse_vllm_log" => env!("CARGO_BIN_EXE_parse_vllm_log"),
        "preload_ram_gate" => env!("CARGO_BIN_EXE_preload_ram_gate"),
        "quant_table" => env!("CARGO_BIN_EXE_quant_table"),
        "rank_recipes" => env!("CARGO_BIN_EXE_rank_recipes"),
        "recipe" => env!("CARGO_BIN_EXE_recipe"),
        "rope_scaling_translate" => env!("CARGO_BIN_EXE_rope_scaling_translate"),
        "run_trial" => env!("CARGO_BIN_EXE_run_trial"),
        "selftest_gmu_roles_batch" => env!("CARGO_BIN_EXE_selftest_gmu_roles_batch"),
        "sim_classify" => env!("CARGO_BIN_EXE_sim_classify"),
        other => panic!("no bin {other}"),
    }
}

/// A runtime root (`EASY_VLLM_RECIPE_RUNTIME_ROOT`) whose `target/release/<bin>` are this build's executables, so the
/// launchers start the bins under test in the release and the fixture profile alike.
fn runtime_root(s: &Scratch) -> PathBuf {
    let rt = s.0.join("runtime");
    let rel = rt.join("target/release");
    std::fs::create_dir_all(&rel).unwrap();
    for (_, bin) in LAUNCHERS {
        std::os::unix::fs::symlink(exe(bin), rel.join(bin)).unwrap();
    }
    rt
}

/// `bash <launcher> <args>` with the runtime root override, stdin closed.
fn launch_from(launcher: &Path, rt: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(launcher)
        .args(args)
        .env("EASY_VLLM_RECIPE_RUNTIME_ROOT", rt)
        // a scratch tree never resolves to a git work tree above the temporary directory (the bins ask git for the root)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn launch(rt: &Path, cwd: &Path, launcher: &str, args: &[&str]) -> Output {
    launch_from(&skill().join("scripts").join(launcher), rt, cwd, args)
}

/// Every launcher of `scripts/` exists; an empty `EASY_VLLM_RECIPE_RUNTIME_ROOT` and a root without the executable are
/// exit 2 with nothing on stdout; a root holding the build starts the bin (`--help` is exit 0).
#[test]
fn launchers_and_the_runtime_root_override() {
    let s = scratch("launch");
    let rt = runtime_root(&s);
    let empty_root = s.0.join("empty-root");
    std::fs::create_dir_all(&empty_root).unwrap();
    for (launcher, _) in LAUNCHERS {
        assert!(skill().join("scripts").join(launcher).is_file(), "{launcher}");
        let out = launch(Path::new(""), &s.0, launcher, &["--help"]);
        assert_eq!(out.status.code(), Some(2), "{launcher}: empty runtime root");
        assert_eq!(text(&out.stdout), "", "{launcher}");
        let out = launch(&empty_root, &s.0, launcher, &["--help"]);
        assert_eq!(out.status.code(), Some(2), "{launcher}: no executable");
        assert_eq!(text(&out.stdout), "", "{launcher}");
        if launcher == "selftest_gmu_roles_batch.sh" {
            continue; // a self-test driver without options (run in consumer_self_tests_pass)
        }
        let out = launch(&rt, &s.0, launcher, &["--help"]);
        assert_eq!(out.status.code(), Some(0), "{launcher} --help: {}", text(&out.stderr));
    }
}

/// The subcommands and long options the skill documents and the consumers name are part of the parser (`--help` of the
/// launcher or subcommand is exit 0 and lists them); an unknown option is a usage error, exit 2, nothing on stdout.
#[test]
fn documented_subcommands_and_long_options() {
    let s = scratch("argv");
    let rt = runtime_root(&s);
    let table: [(&str, &[&str], &[&str]); 19] = [
        ("cleanup_docker.sh", &[], &["--apply", "--keep", "--cache-age-hours"]),
        ("crosscheck_model_card.sh", &[], &["--hf-repo-id", "--ephemeral-estimate", "--json"]),
        ("escalation_predicate.sh", &[], &["--symptom", "--events", "--started-utc", "--ended-utc", "--json", "--self-test"]),
        ("estimate_vram.sh", &[], &["--self-test"]),
        ("feedback_log.sh", &[], &[]),
        ("functional_smoke.sh", &[], &[]),
        ("gen_recipe_set.sh", &[], &[]),
        ("parse_model_config.sh", &[], &["--nas-root", "--json", "--self-test"]),
        ("parse_vllm_log.sh", &[], &[]),
        ("preload_ram_gate.sh", &[], &["--self-test"]),
        ("quant_table.sh", &[], &[]),
        ("rank_recipes.sh", &[], &[]),
        ("recipe.sh", &[], &["estimate", "generate", "simulate"]),
        ("recipe.sh", &["estimate"], &["--config", "--auto", "--candidates"]),
        ("recipe.sh", &["generate"], &["--config", "--recipe-id"]),
        (
            "recipe.sh",
            &["simulate"],
            &["--config", "--candidate", "--lockset-out", "--dry-run", "--mock-profile", "--run-id", "--topic", "--cap", "--image"],
        ),
        ("rope_scaling_translate.sh", &[], &["--model-dir", "--target-len", "--json", "--self-test"]),
        ("run_trial.sh", &[], &["--self-test"]),
        ("sim_classify.sh", &[], &[]),
    ];
    for (launcher, sub, options) in table {
        let mut args: Vec<&str> = sub.to_vec();
        args.push("--help");
        let out = launch(&rt, &s.0, launcher, &args);
        let help = text(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{launcher} {sub:?}: {}", text(&out.stderr));
        for o in options {
            assert!(
                help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).any(|w| w == *o),
                "{launcher} {sub:?} lacks {o}:\n{help}"
            );
        }
    }
    for (launcher, args) in [
        ("estimate_vram.sh", &["--bogus-option"][..]),
        ("escalation_predicate.sh", &["--bogus-option"]),
        ("recipe.sh", &["estimate", "--bogus-option"]),
        ("rope_scaling_translate.sh", &["--bogus-option"]),
    ] {
        let out = launch(&rt, &s.0, launcher, args);
        assert_eq!(out.status.code(), Some(2), "{launcher} {args:?}");
        assert_eq!(text(&out.stdout), "", "{launcher} {args:?}");
    }
}

/// policy-distribution runs these through the launchers and needs exit 0.
#[test]
fn consumer_self_tests_pass() {
    let s = scratch("selftest");
    let rt = runtime_root(&s);
    for (launcher, args) in [
        ("escalation_predicate.sh", &["--self-test"][..]),
        ("estimate_vram.sh", &["--self-test"]),
        ("parse_model_config.sh", &["--self-test"]),
        ("preload_ram_gate.sh", &["--self-test"]),
        ("rope_scaling_translate.sh", &["--self-test"]),
        ("run_trial.sh", &["--self-test"]),
        ("selftest_gmu_roles_batch.sh", &[]),
        ("gen_recipe_set.sh", &["--check-parity"]),
    ] {
        let out = launch(&rt, &s.0, launcher, args);
        assert_eq!(out.status.code(), Some(0), "{launcher} {args:?}:\n{}{}", text(&out.stdout), text(&out.stderr));
    }
}

/// The Flag gate (policy:TERRAFORM_FLAG_GATE): in a tree without `output/<topology>/manifest.yaml` (a fresh clone, the
/// smoke_clone.sh form: the launcher copied into the tree, the runtime root pointing at the build) `recipe.sh estimate`,
/// `generate` and `simulate` stop with exit 4 before the config is read — the config path does not exist.
#[test]
fn recipe_flag_gate_is_exit_4_in_an_unterraformed_tree() {
    let s = scratch("flag");
    let rt = runtime_root(&s);
    let tree = s.0.join("tree");
    let launcher = tree.join(".claude/skills/vllm-recipe-explorer/scripts/recipe.sh");
    std::fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    std::fs::copy(skill().join("scripts/recipe.sh"), &launcher).unwrap();
    for args in [
        &["estimate", "--config", "/nonexistent/config.yaml", "--auto"][..],
        &["estimate", "--auto", "--config", "campaigns/_bootstrap/cells/freshclone-probe/config.yaml"],
        &["generate", "--config", "/nonexistent/config.yaml", "--recipe-id", "r1"],
        &["simulate", "--config", "/nonexistent/config.yaml", "--candidate", "/nonexistent/lockset.json"],
    ] {
        let out = launch_from(&launcher, &rt, &tree, args);
        assert_eq!(out.status.code(), Some(4), "{args:?}:\n{}{}", text(&out.stdout), text(&out.stderr));
    }
}

/// `escalation_predicate --symptom KIND… --json` (adversarial-benchmark `campaign_write.rs` runs the ELF directly): exit 0
/// (predicate ① holds) or 1 (it does not) — both are verdicts —, stdout one JSON object with `predicate_1` (bool),
/// `model_plane_standing`·`model_plane_refuted`·`host_plane_ignored`·`unknown_symptoms`·`reasons` (lists of strings) and
/// `evidence_source` (string); stderr is not part of it.
#[test]
fn escalation_predicate_verdict_json() {
    let run = |symptoms: &[&str]| {
        let mut c = Command::new(exe("escalation_predicate"));
        for s in symptoms {
            c.arg("--symptom").arg(s);
        }
        let out = c.arg("--json").stdin(Stdio::null()).output().unwrap();
        let doc = Json::parse(&text(&out.stdout)).unwrap_or_else(|e| panic!("{symptoms:?}: {e}\n{}", text(&out.stdout)));
        for key in ["model_plane_standing", "model_plane_refuted", "host_plane_ignored", "unknown_symptoms", "reasons"] {
            doc.at(key).strs();
        }
        assert_eq!(doc.at("predicate_1").kind(), "bool", "{symptoms:?}");
        assert_eq!(doc.at("evidence_source").kind(), "string", "{symptoms:?}");
        (out.status.code(), doc)
    };
    let (code, doc) = run(&["config_arch_unsupported"]);
    assert_eq!(code, Some(0));
    assert_eq!(doc.at("predicate_1"), &Json::Bool(true));
    assert_eq!(doc.at("model_plane_standing").strs(), ["config_arch_unsupported"]);

    let (code, doc) = run(&["watchdog_trip"]);
    assert_eq!(code, Some(1));
    assert_eq!(doc.at("predicate_1"), &Json::Bool(false));
    assert_eq!(doc.at("host_plane_ignored").strs(), ["watchdog_trip"]);

    let (code, doc) = run(&["config_arch_unsupported", "not_a_symptom"]);
    assert_eq!(code, Some(1));
    assert_eq!(doc.at("predicate_1"), &Json::Bool(false));
    assert_eq!(doc.at("unknown_symptoms").strs(), ["not_a_symptom"]);
}

/// `rope_scaling_translate --model-dir D --target-len N --json` (upstream `smoke_model.rs` runs the ELF directly and reads
/// the object): `needed` (bool), `hf_overrides` (object when needed), `native_max_position_embeddings` (the config's
/// value), `native_source`, `unknowns` and `reasons`. A config without a rope dictionary still answers (no override).
#[test]
fn rope_scaling_translate_json_keys() {
    let s = scratch("rope");
    write(
        &s.0.join("model/config.json"),
        r#"{"architectures": ["LlamaForCausalLM"], "model_type": "llama", "max_position_embeddings": 4096, "rope_theta": 10000.0, "rope_scaling": {"rope_type": "default"}, "hidden_size": 4096, "num_attention_heads": 32}"#,
    );
    let model = s.0.join("model");
    let run = |target: &str| {
        let out = Command::new(exe("rope_scaling_translate"))
            .args(["--model-dir", model.to_str().unwrap(), "--target-len", target, "--json"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let doc = Json::parse(&text(&out.stdout)).unwrap_or_else(|e| panic!("{target}: {e}\n{}{}", text(&out.stdout), text(&out.stderr)));
        for key in ["needed", "hf_overrides", "native_max_position_embeddings", "native_source", "unknowns", "reasons"] {
            doc.at(key);
        }
        assert_eq!(doc.at("needed").kind(), "bool");
        assert_eq!(doc.at("native_max_position_embeddings"), &Json::Num("4096".into()));
        doc
    };
    let doc = run("16384");
    assert_eq!(doc.at("needed"), &Json::Bool(true), "{doc:?}");
    assert_eq!(doc.at("hf_overrides").kind(), "object", "{doc:?}");
    let doc = run("2048");
    assert_eq!(doc.at("needed"), &Json::Bool(false), "{doc:?}");
    write(
        &s.0.join("model/config.json"),
        r#"{"architectures": ["LlamaForCausalLM"], "model_type": "llama", "max_position_embeddings": 4096}"#,
    );
    let doc = run("16384");
    assert_eq!(doc.at("needed"), &Json::Bool(true), "{doc:?}");
    assert_eq!(doc.at("hf_overrides"), &Json::Null, "{doc:?}");
}

/// A small RFC 8259 reader for the contract checks: the fixed surface is the parsed value (L2), so the checks read the
/// output as JSON instead of comparing bytes. Numbers keep their source text; no dependency of the crate is used.
mod json {
    // Each crate's checks use a different part of this shared reader.
    #![allow(dead_code)]

    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(String),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        pub fn parse(text: &str) -> Result<Json, String> {
            let mut r = Reader { b: text.as_bytes(), i: 0 };
            let v = r.value()?;
            r.ws();
            if r.i != r.b.len() {
                return Err(format!("text after the JSON value at byte {}", r.i));
            }
            Ok(v)
        }
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        /// `self[key]`, failing the check when the key is absent.
        pub fn at(&self, key: &str) -> &Json {
            self.get(key).unwrap_or_else(|| panic!("key {key:?} missing in {self:?}"))
        }
        pub fn keys(&self) -> Vec<&str> {
            match self {
                Json::Obj(m) => m.iter().map(|(k, _)| k.as_str()).collect(),
                _ => Vec::new(),
            }
        }
        pub fn str(&self) -> Option<&str> {
            match self {
                Json::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn arr(&self) -> Option<&[Json]> {
            match self {
                Json::Arr(a) => Some(a),
                _ => None,
            }
        }
        /// The JSON type name: null, bool, number, string, array, object.
        pub fn kind(&self) -> &'static str {
            match self {
                Json::Null => "null",
                Json::Bool(_) => "bool",
                Json::Num(_) => "number",
                Json::Str(_) => "string",
                Json::Arr(_) => "array",
                Json::Obj(_) => "object",
            }
        }
        /// A list of strings (e.g. `reason_codes`), failing the check on any other shape.
        pub fn strs(&self) -> Vec<String> {
            let items = self.arr().unwrap_or_else(|| panic!("not an array: {self:?}"));
            items.iter().map(|v| v.str().unwrap_or_else(|| panic!("not a string: {v:?}")).to_string()).collect()
        }
    }

    struct Reader<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Reader<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            }
        }
        fn eat(&mut self, lit: &str) -> Result<(), String> {
            if self.b[self.i..].starts_with(lit.as_bytes()) {
                self.i += lit.len();
                Ok(())
            } else {
                Err(format!("expected {lit:?} at byte {}", self.i))
            }
        }
        fn value(&mut self) -> Result<Json, String> {
            self.ws();
            match self.b.get(self.i) {
                None => Err("unexpected end of input".into()),
                Some(b'{') => {
                    self.i += 1;
                    let mut m = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(m));
                    }
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.ws();
                        self.eat(":")?;
                        let v = self.value()?;
                        m.push((k, v));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Json::Obj(m));
                            }
                            _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(a));
                    }
                    loop {
                        a.push(self.value()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Json::Arr(a));
                            }
                            _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Json::Str(self.string()?)),
                Some(b't') => self.eat("true").map(|_| Json::Bool(true)),
                Some(b'f') => self.eat("false").map(|_| Json::Bool(false)),
                Some(b'n') => self.eat("null").map(|_| Json::Null),
                Some(c) if *c == b'-' || c.is_ascii_digit() => {
                    let start = self.i;
                    while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                        self.i += 1;
                    }
                    let t = std::str::from_utf8(&self.b[start..self.i]).unwrap().to_string();
                    t.parse::<f64>().map_err(|e| format!("bad number {t:?}: {e}"))?;
                    Ok(Json::Num(t))
                }
                Some(c) => Err(format!("unexpected byte {c:#x} at {}", self.i)),
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            let v = u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())?;
            self.i += 4;
            Ok(v)
        }
        fn string(&mut self) -> Result<String, String> {
            self.eat("\"")?;
            let mut out = Vec::new();
            loop {
                let c = *self.b.get(self.i).ok_or("unterminated string")?;
                self.i += 1;
                match c {
                    b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                    b'\\' => {
                        let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                        self.i += 1;
                        let ch = match e {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'/' => '/',
                            b'b' => '\u{8}',
                            b'f' => '\u{c}',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'u' => {
                                let hi = self.hex4()?;
                                let code = if (0xD800..0xDC00).contains(&hi) {
                                    self.eat("\\u")?;
                                    let lo = self.hex4()?;
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    hi
                                };
                                char::from_u32(code).ok_or("lone surrogate")?
                            }
                            _ => return Err(format!("bad escape at byte {}", self.i)),
                        };
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    c if c < 0x20 => return Err(format!("control byte in string at {}", self.i)),
                    c => out.push(c),
                }
            }
        }
    }
}
