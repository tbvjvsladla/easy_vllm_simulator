//! Clauses ported against the terraforming_node owners (see claims.rs for the mapping): scan_node / manifest_contract
//! (`role_contract`), campaign_init / campaign_template_validator (`sweep_stop::campaign_write` / `campaign_validator`, the
//! Rust owner behind `campaign_init.sh`), recipe.py (`recipe_runtime::recipe_cmd`), crosscheck_model_card
//! (`recipe_runtime::model_card`), render_dockerfile (its command line `render_dockerfile.sh`).
//!
//! The originals executed these owners in-process with controlled inputs; here the Rust owners are called the same way (their
//! library functions, or the native command line when the original called `main(argv)`). Where the original read an owner's
//! `.py` SOURCE text as evidence, the clause now reads the Rust source that carries the same property (the `.py` is no longer
//! the implementation), with the same fail-closed, non-vacuous shape.
#![allow(unused_imports)]
use crate::support::*;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- helpers

/// The body (from the opening `{` to its matching `}`) of the first `fn <name>(` in a Rust source: strings, raw strings, char
/// literals and comments are skipped, so braces inside `format!` texts never count. Absent = AssertionError (as
/// `_extract_python_function`).
pub fn rust_fn_body(src: &str, name: &str) -> R<String> {
    let head = re(&format!(r"(?m)^[ \t]*(?:pub(?:\([a-z]+\))?[ \t]+)?fn[ \t]+{}\b", regex_escape(name)));
    let start = head
        .find(src)
        .ok()
        .flatten()
        .map(|m| m.start())
        .ok_or_else(|| Fail::new("AssertionError", format!("function {} not found", repr(name))))?;
    let b = src.as_bytes();
    let mut i = start;
    let mut paren = 0i32;
    let mut depth = 0i32;
    let mut open: Option<usize> = None;
    while i < b.len() {
        let c = b[i];
        // comments
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        // raw strings r"..." / r#"..."#
        if c == b'r' && i + 1 < b.len() && (b[i + 1] == b'"' || b[i + 1] == b'#') && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < b.len() && b[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat(b'#').take(hashes)).collect();
                j += 1;
                while j < b.len() && !b[j..].starts_with(&close) {
                    j += 1;
                }
                i = j + close.len();
                continue;
            }
        }
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == b'\'' {
            // char literal: '{', '\'', '\\', '\u{..}' -- a lifetime ('a) has no closing quote right after.
            if i + 2 < b.len() && b[i + 1] != b'\\' && b[i + 2] == b'\'' {
                i += 3;
                continue;
            }
            if i + 1 < b.len() && b[i + 1] == b'\\' {
                let mut j = i + 2;
                while j < b.len() && b[j] != b'\'' {
                    j += 1;
                }
                i = j + 1;
                continue;
            }
        }
        match c {
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'{' if open.is_none() && paren == 0 => {
                open = Some(i);
                depth = 1;
            }
            b'{' if open.is_some() => depth += 1,
            b'}' if open.is_some() => {
                depth -= 1;
                if depth == 0 {
                    return Ok(src[start..=i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(Fail::new("AssertionError", format!("unterminated function {}", repr(name))))
}

/// `manifest_contract.main(argv)` = the Flag-gate command line (`manifest_contract.sh`, Rust `manifest_contract`).
fn manifest_contract_cli(args: &[&str]) -> R<Proc> {
    let l = repo_root().join(".claude/skills/terraforming_node/scripts/manifest_contract.sh").to_string_lossy().into_owned();
    let mut argv = vec!["bash", l.as_str()];
    argv.extend_from_slice(args);
    run(&argv, None, &[], None, Some(120))
}

fn ps(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// run_bench.sh's Flag gate (R48, now direct): `MC` names the launcher, the availability check precedes the gate call, an
/// invalid Flag stops with exit 4 right at the gate, before the workload env is loaded.
const FLAG_GATE_MC: &str = "MC=\"$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.sh\"";
const FLAG_GATE_LAUNCHER_COMMAND: &str = "bash \"$MC\" --topology \"$TOPO\" --repo \"$REPO\" --require-flag";
const FLAG_GATE_AVAILABILITY_COMMAND: &str = "MANIFEST_CONTRACT_VALIDATE_ONLY=1 bash \"$MC\"";

fn require_flag_gate_caller(src: &str, what: &str) -> R {
    req(src.contains(FLAG_GATE_MC), "run_bench.sh must bind MC to the manifest_contract launcher (the native Flag gate)")?;
    req(src.contains(FLAG_GATE_LAUNCHER_COMMAND), format!("run_bench.sh must invoke the Flag gate through the launcher with exactly {what}"))?;
    let gate = index(src, FLAG_GATE_LAUNCHER_COMMAND)?;
    req(index(src, FLAG_GATE_MC)? < gate, "MC must be bound before the Flag gate call")?;
    req(src.contains(FLAG_GATE_AVAILABILITY_COMMAND) && index(src, FLAG_GATE_AVAILABILITY_COMMAND)? < gate, "the launcher availability check must precede the Flag gate call")?;
    let end = (gate + 400).min(src.len());
    let window = src.get(gate..end).unwrap_or(&src[gate..]);
    req(window.contains("exit 4"), "an invalid Flag must stop the script with exit 4 right at the gate")?;
    req(gate < index(src, "EF=\"$REPO/output/$TOPO/envs/.env.$CONFIG\"")?, "the Flag gate must run before the workload env is loaded")
}

/// `recipe.main()` with `recipe.REPO_ROOT` pointed at a hermetic Flag-absent root: the Rust `recipe_main` derives the root
/// from the skill directory (git toplevel, else three levels up), so the skill directory is placed three levels below `tmp`.
fn recipe_dispatch(tmp: &Path, argv: &[String]) -> R<i32> {
    let skill = tmp.join(".claude/skills/vllm-recipe-explorer");
    mkdirs(&skill)?;
    let args: Vec<PyStr> = argv.iter().map(|a| PyStr::new(a)).collect();
    let run = recipe_runtime::recipe_cmd::recipe_main(&args, None, &PyStr::new(&ps(&skill)));
    Ok(run.status)
}

fn sweep(text: &str) -> R<sweep_stop::pyvalue::PyValue> {
    sweep_stop::pyjson::loads(text).map_err(|e| Fail::new("ValueError", format!("{e:?}")))
}

fn sweep_dict(text: &str) -> R<sweep_stop::pyvalue::PyDict> {
    match sweep(text)? {
        sweep_stop::pyvalue::PyValue::Dict(d) => Ok(d),
        _ => Err(Fail::new("TypeError", "expected a JSON object")),
    }
}

fn stop_text(s: sweep_stop::campaign_write::Stop) -> Fail {
    match s {
        sweep_stop::campaign_write::Stop::Refuse(t) => Fail::new("PurgeGateRefusal", t),
        sweep_stop::campaign_write::Stop::Fallback => Fail::new("NotImplementedError", "campaign_init: shape not modelled"),
    }
}

/// A hermetic checkout for the campaign owner: `campaigns/` (skeleton copied from the real tree so the built-in validator has
/// its template) plus the two files the evidence pointers below name.
fn campaign_fixture_root(tmp: &Path) -> R<PathBuf> {
    let root = tmp.join("repo");
    let real = repo_root();
    for rel in rglob_files(&real.join("campaigns/_template")) {
        let src = real.join("campaigns/_template").join(&rel);
        let dst = root.join("campaigns/_template").join(&rel);
        if let Some(p) = dst.parent() {
            mkdirs(p)?;
        }
        std::fs::copy(&src, &dst).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    write(&root.join("campaigns/README.md"), &read("campaigns/README.md")?)?;
    write(&root.join("CLAUDE.md"), "fixture\n")?;
    write(&root.join("README.md"), "fixture\n")?;
    Ok(root)
}

/// `_grounding_fixture(ci, camp, status, reason)`.
fn grounding_fixture(camp: &Path, status: &str, reason: Option<&str>) -> R {
    let name = camp.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let reason_field = reason.map(|r| format!(", \"reason\": {}", json_str(r))).unwrap_or_default();
    let gap = if status == "resolved" { "null".to_string() } else { format!("{{\"status\": {}, \"asked_utc\": \"t\"}}", json_str(status)) };
    let doc = format!(
        "{{\"schema_version\": 1, \"campaign_id\": {}, \"asked_utc\": \"2026-09-08T00:00:00Z\", \"request\": {{\"query\": {{\"terms\": [\"m1\"]}}}}, \"export\": {{\"resolution\": {{\"status\": {}, \"librarian\": \"wiki-desk\"{}}}, \"references\": []}}, \"attestation\": {{\"status\": {}, \"grounding_gap\": {}}}}}",
        json_str(&name), json_str(status), reason_field, json_str(status), gap
    );
    write(&camp.join("grounding/20260908T000000Z.json"), &doc)
}

/// The real `Files` boundary of the P1 scanner (what the production command line hands in).
struct OsFiles;
impl role_contract::scan_node_files::Files for OsFiles {
    fn exists(&self, path: &[u8]) -> bool {
        use std::os::unix::ffi::OsStrExt;
        std::fs::metadata(std::ffi::OsStr::from_bytes(path)).is_ok()
    }
    fn read(&self, path: &[u8]) -> Result<Vec<u8>, role_contract::scan_node_files::FileFault> {
        use std::os::unix::ffi::OsStrExt;
        std::fs::read(std::ffi::OsStr::from_bytes(path)).map_err(|e| role_contract::scan_node_files::FileFault {
            errno: e.raw_os_error().unwrap_or(0),
            strerror: e.to_string(),
        })
    }
}

const SCAN_NODE_CLI_RS: &str = ".claude/skills/adversarial-benchmark/rust/src/scan_node_cli.rs";
const SCAN_NODE_PROBE_RS: &str = ".claude/rust/role-contract/src/scan_node_probe.rs";
const SCAN_NODE_P1_T_SOURCES: [&str; 3] = [
    ".claude/rust/role-contract/src/scan_node.rs",
    ".claude/rust/role-contract/src/scan_node_manifest.rs",
    ".claude/rust/role-contract/src/scan_node_files.rs",
];
const CAMPAIGN_WRITE_RS: &str = ".claude/skills/adversarial-benchmark/rust/src/campaign_write.rs";

// ---------------------------------------------------------------- MODEL_ACQUISITION_TERNARY_GATE.C4

pub fn predicate_MODEL_ACQUISITION_TERNARY_GATE_C4() -> R {
    let tmpl = read("manifest.template.yaml")?;
    req(search("hf_token_env_file:\\s*\"\"", &tmpl).is_some(), "must default to an empty pointer field")?;
    let gitignore = read(".gitignore")?;
    let lines = splitlines(&gitignore);
    req(lines.contains(&".env") && lines.contains(&"envs/.env.*"), "env files (where a real hf token would live) must be gitignored")?;

    // Code-level: read_manifest_field (the Rust P1 owner) resolves the pointer -- a path string, never file content.
    let tmp = TempDir::new("pp-mat-c4-")?;
    let mpath = tmp.path().join("manifest.yaml");
    write(&mpath, "hf_token_env_file: /some/host/path/token.env\n")?;
    let got = role_contract::scan_node_files::read_manifest_field(&OsFiles, &PyValue::str(&ps(&mpath)), &PyValue::str("hf_token_env_file"))
        .map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    req(jstr(&got).as_deref() == Some("/some/host/path/token.env"), "predicate requirement failed at original line 1203")?;

    // Source-level (mutation-sensitive), retargeted from scan_node_original.py's main() to the Rust command line that now
    // carries it (`scan_node_cli.rs`): every use of the resolved pointer `token_file` is its definition, an existence check
    // (`local.is_file(path)` inside `match token_file.peel()`), or the remote probe `collect_peer_model_env(.., &token_file, ..)`.
    let cli = read(SCAN_NODE_CLI_RS)?;
    let def = "match files::read_manifest_field(files, &mani_path, &text(\"hf_token_env_file\"))";
    req(cli.contains(def), "hf_token_env_file must actually be consumed somewhere (not dead code)")?;
    let tok = re(r"\btoken_file\b");
    let mut sinks: Vec<String> = Vec::new();
    for m in tok.find_iter(&cli).flatten() {
        let (s, e) = (m.start(), m.end());
        let before = &cli[s.saturating_sub(4)..s];
        let after = &cli[e..(e + 8).min(cli.len())];
        let call_open = cli[..s].rfind("collect_peer_model_env(");
        let in_probe_call = call_open.map_or(false, |o| !cli[o..s].contains(")?") && before.ends_with('&'));
        let kind = if before.ends_with("let ") {
            "definition"
        } else if after.starts_with(".peel()") {
            "isfile"
        } else if in_probe_call {
            "collect_peer_model_env"
        } else {
            "other"
        };
        sinks.push(kind.to_string());
    }
    req(sinks.iter().any(|k| k != "definition"), "hf_token_env_file must actually be consumed somewhere (not dead code)")?;
    let allowed = ["definition", "isfile", "collect_peer_model_env"];
    req(sinks.iter().all(|k| allowed.contains(&k.as_str())), format!("hf_token_env_file may only be passed to existence checks (local or remote), found: {}", repr_list(&sinks)))?;
    let m = index(&cli, "match token_file.peel()")?;
    let arm_end = index_from(&cli, "};", m)?;
    let arm = &cli[m..arm_end];
    req(arm.contains("local.is_file(path)"), "hf_token_env_file may only be passed to existence checks (local or remote), found: a non-isfile local use")?;
    for banned in ["read_file", "read_to_string", "fs::read", "open("] {
        req(!arm.contains(banned), "the raw token file's content must never be read")?;
    }
    req(search(r"(?:read|open)[A-Za-z_]*\([^)]*token_file", &cli).is_none(), "the raw token file's content must never be read")?;
    let peer = rust_fn_body(&read(SCAN_NODE_PROBE_RS)?, "collect_peer_model_env")?;
    req(!peer.contains("open(") && !peer.contains("read_file") && !peer.contains("read_to_string") && !peer.contains("cat "), "the remote existence probe must never read the token file content")?;
    req(peer.contains("[ -f "), "the remote probe must be an existence test, not a content read")?;

    // The P1 parts (Rust T sources) never read the token file either.
    let mut p1: Vec<(String, String)> = Vec::new();
    for rel in SCAN_NODE_P1_T_SOURCES {
        p1.push((rel.to_string(), read(rel)?));
    }
    req(p1.iter().all(|(_, t)| ["std::fs", "File::open", "read_to_string"].iter().all(|n| !t.contains(n))), "the P1 Rust T sources must read files only through the Files boundary")?;
    let quoted = re(r".hf_token_env_file.");
    req(p1.iter().all(|(_, t)| quoted.find_iter(t).flatten().all(|m| m.as_str().starts_with('"') && m.as_str().ends_with('"'))), "hf_token_env_file may appear in the P1 Rust T sources only as a quoted manifest key")?;
    req(!p1.iter().find(|(r, _)| r.ends_with("scan_node_files.rs")).map(|(_, t)| t.contains("hf_token")).unwrap_or(true), "the P1 file reader must not know the token pointer at all")
}

// ---------------------------------------------------------------- TERRAFORM_FLAG_GATE.C1

#[allow(clippy::too_many_arguments)]
fn gate(declared: &str, ic: bool, peer_given: bool, peer_reachable: Option<bool>, bandwidth: Option<f64>, branch: &str, branch_topo: &str, mani_topo: &str, peer_ip: Option<&str>) -> R<(String, String, i64)> {
    use role_contract::scan_node::{evaluate_gate, GateArgs};
    let opt_b = |v: Option<bool>| v.map(PyValue::Bool).unwrap_or(PyValue::None);
    let args = GateArgs {
        declared: PyValue::str(declared),
        ic_present: PyValue::Bool(ic),
        peer_given: PyValue::Bool(peer_given),
        peer_reachable: opt_b(peer_reachable),
        bandwidth: bandwidth.map(PyValue::Float).unwrap_or(PyValue::None),
        bw_floor: PyValue::Float(180.0),
        per_port_gbps: PyValue::None,
        per_port_floor: PyValue::None,
        branch: PyValue::str(branch),
        branch_topo: PyValue::str(branch_topo),
        mani_topo: PyValue::str(mani_topo),
        mani_path: PyValue::str("manifest.yaml"),
        peer_ip: peer_ip.map(PyValue::str).unwrap_or(PyValue::None),
        egress_self: PyValue::None,
        egress_peer: PyValue::None,
        model_env_ok: PyValue::None,
        model_env_reason: PyValue::None,
        manifest_read_error: PyValue::None,
    };
    let out = evaluate_gate(&args).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    let items = match out.peel() {
        PyValue::Tuple(t) | PyValue::List(t) if t.len() == 3 => t.clone(),
        other => return Err(Fail::new("TypeError", format!("evaluate_gate returned {}", other.type_name()))),
    };
    let g = &items[1];
    let field = |k: &str| jget(g, k).ok().and_then(jstr).unwrap_or_default();
    let code = match items[2].peel() {
        PyValue::Int(i) => i.to_string().parse::<i64>().unwrap_or(-1),
        _ => -1,
    };
    Ok((field("branch"), field("status"), code))
}

fn contract(man: &str) -> R<(bool, i32)> {
    let res = crate::claims_owner::evaluate(man, "single")?;
    let flag = matches!(res.get("flag").map(PyValue::peel), Some(PyValue::Bool(true)));
    let code = match res.get("exit_code").map(PyValue::peel) {
        Some(PyValue::Int(i)) => i.to_string().parse::<i32>().unwrap_or(-1),
        _ => -1,
    };
    Ok((flag, code))
}

pub fn predicate_TERRAFORM_FLAG_GATE_C1() -> R {
    // The emit gate of main() now lives in the Rust command line (scan_node_cli.rs): the Flag-attestation block is emitted
    // only under `--emit-manifest` AND gate.status == "ok" AND a CONFIRMED (single|multi) topology, at exactly one call site.
    let cli = read(SCAN_NODE_CLI_RS)?;
    let cond = "if a.emit_manifest && gate_ok && (a.topology.equals(\"single\") || a.topology.equals(\"multi\")) {";
    let gate_ok = "let gate_ok = matches!(get(&get(&envelope, \"gate\"), \"status\").peel(), PyValue::Str(word) if word.equals(\"ok\"));";
    req(cli.contains(cond) && cli.contains(gate_ok) && count(&cli, "emit_manifest_block(") == 1
        && index(&cli, gate_ok)? < index(&cli, cond)? && index(&cli, cond)? < index(&cli, "emit_manifest_block(")?,
        "the Flag-attestation emit call must be gated on gate.status=='ok' AND a CONFIRMED (non-auto) topology -- a regression loosening this condition would let emit_manifest_block stamp complete:true/branch_verified:true on a blocked or speculative gate")?;

    // --- Layer A: single-topology matrix
    let single = |declared: &str, branch_topo: &str, mani_topo: &str| gate(declared, false, false, None, None, "single-node", branch_topo, mani_topo, None);
    let (b, s, e) = single("single", "single", "single")?;
    req(b == "alpha" && s == "ok" && e == 0, "the genuinely-consistent single baseline must pass -- control for the flips below")?;
    let (b, s, _) = single("auto", "single", "single")?;
    req(b == "auto" && s == "report-only", "an unconfirmed topology declaration must never produce an 'ok' gate")?;
    use role_contract::scan_node::emit_gate;
    req(emit_gate(&PyValue::Bool(true), &PyValue::str("auto")) != 0, "emit_gate must fail-closed on --emit-manifest while topology is still speculative")?;
    req(emit_gate(&PyValue::Bool(true), &PyValue::str("single")) == 0 && emit_gate(&PyValue::Bool(true), &PyValue::str("multi")) == 0, "control: emit_gate must NOT block once topology is genuinely confirmed")?;
    let (b, s, e) = single("single", "multi", "multi")?;
    req(b == "alpha" && s == "blocked" && e == 2, "declared topology disagreeing with the git branch must block, alone")?;
    let (b, s, e) = single("single", "single", "multi")?;
    req(b == "alpha" && s == "blocked" && e == 2, "an on-disk manifest topology disagreeing with branch/declared must block, alone")?;

    // --- Layer A: multi-topology matrix
    let multi = |ic: bool, reach: bool, bw: Option<f64>, ip: Option<&str>| gate("multi", ic, true, Some(reach), bw, "multi-node", "multi", "multi", ip);
    let (b, s, e) = multi(true, true, Some(208.2), None)?;
    req(b == "multi-ready" && s == "ok" && e == 0, "the genuinely-consistent multi baseline must pass -- control for the flips below")?;
    let (b, s, e) = multi(false, true, Some(208.2), None)?;
    req(b == "gamma" && s == "blocked" && e == 2, "missing interconnect alone must block multi")?;
    let (b, s, e) = multi(true, false, Some(208.2), Some("x"))?;
    req(b == "gamma" && s == "blocked" && e == 2, "an unreachable declared peer alone must block multi")?;
    let (b, s, e) = multi(true, true, Some(50.0), None)?;
    req(b == "gamma" && s == "blocked" && e == 2, "below-floor measured bandwidth alone must block multi")?;
    let (b, s, e) = multi(true, true, None, None)?;
    req(b == "multi-ready-candidate" && s == "pending-perf" && s != "ok" && e == 2,
        "an untaken bandwidth measurement must stay pending-perf AND exit non-zero -- exit 0 is read as success by `scan_node.sh || abort` runners")?;

    // --- Layer B: manifest_contract's read-gate
    use role_contract::manifest_gate::{EXIT_MISSING_FIELD, EXIT_NO_FLAG, EXIT_OK};
    let base = "{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"managed\", \"terraforming\": {\"complete\": true, \"branch_verified\": true}}";
    let (f, c) = contract(base)?;
    req(f && c == EXIT_OK, "the genuinely-complete baseline must pass -- control for the flips below")?;
    let (f, c) = contract("{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"managed\", \"terraforming\": {\"complete\": false, \"branch_verified\": true}}")?;
    req(!f && c == EXIT_NO_FLAG, "predicate requirement failed at original line 1979")?;
    let (f, c) = contract("{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"managed\", \"terraforming\": {\"complete\": true, \"branch_verified\": false}}")?;
    req(!f && c == EXIT_NO_FLAG, "predicate requirement failed at original line 1983")?;
    let (f, c) = contract("{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"managed\"}")?;
    req(!f && c == EXIT_NO_FLAG, "a wholly absent terraforming block must fail exactly like an explicit false -- never a silent pass by omission")?;
    let (f, c) = contract("{\"topology\": \"single\", \"gpus_per_node\": null, \"model_source\": \"managed\", \"terraforming\": {\"complete\": true, \"branch_verified\": true}}")?;
    req(!f && c == EXIT_MISSING_FIELD, "predicate requirement failed at original line 1994")?;
    let (f, c) = contract("{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"nas-bogus\", \"terraforming\": {\"complete\": true, \"branch_verified\": true}}")?;
    req(!f && c == EXIT_MISSING_FIELD, "predicate requirement failed at original line 1998")
}

// ---------------------------------------------------------------- TERRAFORM_FLAG_GATE.C2

/// The recipe deliverable dispatch: estimate/generate and simulate each pass the Flag gate after the command line parses.
fn require_recipe_dispatch_source() -> R {
    let src = read(".claude/skills/vllm-recipe-explorer/rust/runtime/src/recipe_cmd.rs")?;
    let main = rust_fn_body(&src, "recipe_main")?;
    // estimate/generate fall through recipe_main into estimate_or_generate, which carries their single gate call;
    // simulate gates inside recipe_main (pass-3 refactor split the bodies — the gated set is unchanged).
    let eg = rust_fn_body(&src, "estimate_or_generate")?;
    req(main.contains("Some(\"estimate\") | Some(\"generate\") => {}") && main.contains("Some(\"simulate\") => {")
        && count(&main, "require_terraform_flag(&root)") == 1 && count(&eg, "require_terraform_flag(&root)") == 1
        && src.contains("fn require_terraform_flag("),
        "the live dispatch source must gate exactly {estimate, generate, simulate} -- not a wider or narrower set -- through the real _require_terraform_flag call")
}

const FAKE_DOCKER: &str = r#"#!/bin/bash
# hermetic fake docker: `ps`/`logs` answer, `exec <ctr> bash -lc <payload>` runs the REAL inner payload with a route nonce,
# `exec <ctr> cat <path>` reads the fake container's /tmp (an owned directory of the hermetic root).
ctmp() { printf '%s/%s' "$FAKE_CONTAINER_TMP" "${1#/tmp/}"; }
case "${1-}" in
  ps) echo fixture-container-id ;;
  logs) : ;;
  exec)
    if [ "${3-}" = bash ] && [ "${4-}" = -lc ]; then
      nonce="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
      printf 'ROUTE\t%s\t%s\t%s %s\n' "$nonce" "$2" "$3" "$4" >> "$VLLM_DOCKER_ROUTE_CAPTURE"
      case "$5" in
        "tail -5 /tmp/"*)
          name="$(printf '%s' "$5" | awk '{print $3}')"; t="$(ctmp "$name")"
          if [ -f "$t" ]; then tail -5 "$t"; fi ;;
        *) VLLM_FAKE_DOCKER_ROUTE="$nonce" /bin/bash -c "$5" || exit 1 ;;
      esac
    elif [ "${3-}" = cat ]; then
      cat "$(ctmp "$4")"
    else
      echo "unexpected docker argv: $*" >&2; exit 1
    fi ;;
  *) echo "unexpected docker argv: $*" >&2; exit 1 ;;
esac
"#;

const FAKE_VLLM: &str = r#"#!/bin/bash
# hermetic fake vllm: records the argv exactly as received (one ARG line per argument) and the docker route it ran under,
# then writes the result file into the fake container's /tmp.
{ printf 'CALL\t%s\n' "${VLLM_FAKE_DOCKER_ROUTE-}"; for a in "$@"; do printf 'ARG\t%s\n' "$a"; done; printf 'END\n'; } >> "$VLLM_ARGV_CAPTURE"
dir=""; file=""; prev=""; seen_dir=0; seen_file=0
for a in "$@"; do
  case "$prev" in --result-dir) dir="$a"; seen_dir=1 ;; --result-filename) file="$a"; seen_file=1 ;; esac
  prev="$a"
done
[ "$seen_dir" = 1 ] && [ "$seen_file" = 1 ] || { echo "fake vllm: --result-dir/--result-filename missing" >&2; exit 1; }
case "$dir" in /tmp|/tmp/*) ;; *) echo "fake vllm: result dir outside /tmp: $dir" >&2; exit 1 ;; esac
rel="${dir#/tmp}"; rel="${rel#/}"
out="$FAKE_CONTAINER_TMP${rel:+/$rel}/$file"
mkdir -p "$(dirname "$out")"
printf '{}' > "$out"
"#;

/// The fake vllm capture: `(docker_route, argv)` per invocation.
fn parse_vllm_capture(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut cur: Option<(String, Vec<String>)> = None;
    for line in text.split('\n') {
        if let Some(r) = line.strip_prefix("CALL\t") {
            cur = Some((r.to_string(), Vec::new()));
        } else if let Some(a) = line.strip_prefix("ARG\t") {
            if let Some(c) = cur.as_mut() {
                c.1.push(a.to_string());
            }
        } else if line == "END" {
            if let Some(c) = cur.take() {
                out.push(c);
            }
        }
    }
    out
}

fn run_bench_scenario() -> R {
    let real = repo_root();
    let tmp = TempDir::new("pp-flag-c2-bench-")?;
    let root = tmp.path();
    let script_dir = root.join(".claude/skills/adversarial-benchmark/scripts");
    mkdirs(&script_dir)?;
    for name in ["run_bench.sh", "bench_shell.sh"] {
        std::fs::copy(real.join(".claude/skills/adversarial-benchmark/scripts").join(name), script_dir.join(name)).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    let bench_shell_bin = real.join(".claude/skills/adversarial-benchmark/rust/target/release/bench_shell");
    use std::os::unix::fs::PermissionsExt;
    let exec_ok = std::fs::metadata(&bench_shell_bin).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false);
    req(exec_ok, format!("the real run_bench.sh needs the node-local native bench_shell product: {}", bench_shell_bin.display()))?;
    write(&root.join("output/single/envs/.env.fixture"), "SERVING_PORT=18000\nSERVING_MODEL_NAME=fixture-model\nCONTAINER_NAME=fixture-container\nCONFIG_FILE=fixture\n")?;
    write(&root.join("output/single/configs/fixture.yaml"), "model: /fixture/model\n")?;
    write(&root.join("output/single/manifest.yaml"), "topology: single\n")?;
    // The Flag gate ANSWER is faked (as the original's fake python3 did): the launcher stand-in answers success. The real
    // launcher and the native gate are proven by layer B above / C3 and the owner's shell checks.
    let mc = root.join(".claude/skills/terraforming_node/scripts/manifest_contract.sh");
    write(&mc, "#!/bin/bash\n# hermetic gate fixture launcher: availability check and gate both succeed\nexit 0\n")?;
    let fake_bin = root.join("fake-bin");
    write(&fake_bin.join("python3"), "#!/bin/sh\ncase \"${1-}\" in\n  -c|-) cat >/dev/null ;;\nesac\nexit 0\n")?;
    write(&fake_bin.join("curl"), "#!/bin/sh\nprintf 200\n")?;
    write(&fake_bin.join("vllm"), FAKE_VLLM)?;
    write(&fake_bin.join("docker"), FAKE_DOCKER)?;
    for f in [mc.clone(), fake_bin.join("python3"), fake_bin.join("curl"), fake_bin.join("vllm"), fake_bin.join("docker")] {
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    let capture = root.join("vllm-argv.txt");
    let route_capture = root.join("docker-route.txt");
    let container_tmp = root.join("container-tmp");
    mkdirs(&container_tmp)?;
    let path = format!("{}:{}", ps(&fake_bin), std::env::var("PATH").unwrap_or_default());
    let (cap, rcap, bsb, ctmp) = (ps(&capture), ps(&route_capture), ps(&bench_shell_bin), ps(&container_tmp));
    let script = ps(&script_dir.join("run_bench.sh"));
    let done = run(
        &["bash", &script, "fixture", "--topology", "single", "--backend", "openai-chat", "--concurrency", "1", "--input-len", "16", "--output-len", "8", "--num-prompts", "1", "--warmups", "0"],
        Some(root),
        &[("PATH", &path), ("VLLM_ARGV_CAPTURE", &cap), ("VLLM_DOCKER_ROUTE_CAPTURE", &rcap), ("BENCH_SHELL_BIN", &bsb), ("FAKE_CONTAINER_TMP", &ctmp)],
        None,
        Some(600),
    )?;
    req(done.code == 0, format!("{}{}", done.stdout, done.stderr))?;
    let captures = parse_vllm_capture(&std::fs::read_to_string(&capture).unwrap_or_default());
    let routes: Vec<Vec<String>> = std::fs::read_to_string(&route_capture).unwrap_or_default().lines()
        .filter_map(|l| l.strip_prefix("ROUTE\t")).map(|l| l.split('\t').map(str::to_string).collect()).collect();
    req(routes.len() == 1 && routes[0].get(1).map(String::as_str) == Some("fixture-container") && routes[0].get(2).map(String::as_str) == Some("bash -lc"), format!("{routes:?}"))?;
    req(captures.len() == 1 && !routes[0][0].is_empty() && captures[0].0 == routes[0][0], format!("exactly one vllm invocation must execute through docker exec bash -lc; routes={routes:?} captures={captures:?}"))?;
    let argv = &captures[0].1;
    req(argv.len() >= 2 && argv[0] == "bench" && argv[1] == "serve", format!("{argv:?}"))?;
    let temps: Vec<usize> = argv.iter().enumerate().filter(|(_, a)| *a == "--temperature").map(|(i, _)| i).collect();
    req(temps.len() == 1 && argv.get(temps[0] + 1).map(String::as_str) == Some("0"), format!("the actually executed vllm bench serve argv must contain exactly one greedy temperature pin; argv={argv:?}"))?;
    let after = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).map(String::as_str);
    req(after("--result-dir") == Some("/tmp") && after("--result-filename") == Some("ab_bench_fixture.json"), format!("run_bench.sh must still name the container /tmp result path; argv={argv:?}"))?;
    req(std::fs::read_to_string(container_tmp.join("ab_bench_fixture.json")).ok().as_deref() == Some("{}"), "the fake container result must be written into the owned container-tmp, not the host /tmp")
}

pub fn predicate_TERRAFORM_FLAG_GATE_C2() -> R {
    use role_contract::manifest_gate::EXIT_NO_MANIFEST;
    let tmp = TempDir::new("pp-flag-c2-")?;
    let t = tmp.path();
    for (cmd, extra) in [("estimate", vec!["--auto".to_string()]), ("generate", vec!["--recipe-id".into(), "r1".into()]), ("simulate", vec!["--candidate".into(), ps(&t.join("nope.json"))])] {
        let mut argv = vec![cmd.to_string(), "--config".into(), ps(&t.join("config.yaml"))];
        argv.extend(extra);
        let code = recipe_dispatch(t, &argv)?;
        req(code == 4, format!("recipe.sh {cmd} must die() with exit 4, got {code}"))?;
    }
    require_recipe_dispatch_source()?;

    // adversarial's second layer: the SAME production entrypoint run_bench.sh shells out to, executed here for real.
    let rc = manifest_contract_cli(&["--topology", "single", "--repo", &ps(t), "--require-flag"])?.code;
    req(rc == EXIT_NO_MANIFEST, "manifest_contract.sh --require-flag must be non-zero against a Flag-absent repo")?;
    let run_bench_src = read(".claude/skills/adversarial-benchmark/scripts/run_bench.sh")?;
    require_flag_gate_caller(&run_bench_src, "the argv executed in-process above (reference), not a lookalike")?;
    req(run_bench_src.contains("테라포밍-완수 Flag 미발급 — info-only"), "run_bench.sh must fall back to an info-only refusal, never a fabricated deliverable")?;
    run_bench_scenario()?;

    // upstream: same documented backstop entrypoint, executed for real in a fresh hermetic Flag-absent repo.
    let tmp2 = TempDir::new("pp-flag-c2b-")?;
    let rc2 = manifest_contract_cli(&["--topology", "single", "--repo", &ps(tmp2.path()), "--require-flag"])?.code;
    req(rc2 == EXIT_NO_MANIFEST, "predicate requirement failed at original line 2157")?;
    let upstream_skill = read(".claude/skills/upstream-version-watch/SKILL.md")?;
    req(upstream_skill.contains("manifest_contract.sh --topology") && upstream_skill.contains("--require-flag") && upstream_skill.contains("결정론 백스톱"), "upstream's SKILL.md must document this exact executed command as its deterministic backstop")?;
    req(upstream_skill.contains("렌더·빌드·bump ✗") && upstream_skill.contains("vLLM GitHub 릴리즈 조회·버전해소 *설명* OK"), "upstream must be documented as blocking render/build/bump specifically, while release lookups and version-resolution explanation stay allowed")?;

    // upstream render deliverable: bake-in Flag gate (the Rust command line behind render_dockerfile.sh).
    let render = ps(&repo_root().join(".claude/skills/upstream-version-watch/scripts/render_dockerfile.sh"));
    let tmp3 = TempDir::new("pp-flag-c2c-")?;
    let man = tmp3.path().join("m.yaml");
    let m = ps(&man);
    let absent = run(&["bash", &render, "--canonical-kind", "compose", "--topology", "single", "--manifest", &m], None, &[], None, Some(120))?;
    req(absent.code == 4, format!("upstream render Flag-absent must exit 4, got {}", absent.code))?;
    write(&man, "topology: single\ncpu_arch: aarch64\ngpus_per_node: 1\nmodel_source: managed\nnas_model_path: /models\nterraforming:\n  complete: true\n  branch_verified: true\n")?;
    let valid = run(&["bash", &render, "--canonical-kind", "compose", "--topology", "single", "--manifest", &m], None, &[], None, Some(120))?;
    req(valid.code == 0, format!("upstream render with a Flag-valid manifest must not be refused: rc={} {}", valid.code, tail_chars(&valid.stderr, 400)))?;
    let render_src = read(".claude/skills/upstream-version-watch/rust/runtime/src/render_template.rs")?;
    req(render_src.contains("fn require_terraform_flag("), "upstream render must define the Flag gate function")?;
    let rr = rust_fn_body(&render_src, "run_render")?;
    let gate_call = "if let Some(status) = require_terraform_flag(manifest_path)? {";
    req(rr.contains("if kind.is_none() && template.is_none() {") && rr.contains(gate_call)
        && index(&rr, "if kind.is_none() && template.is_none() {")? < index(&rr, gate_call)?
        && index(&rr, gate_call)? < index(&rr, "render_shared(")? && index(&rr, gate_call)? < index(&rr, "render(&template")?,
        "the render deliverable paths (canonical-kind/template) must invoke the Flag gate before rendering")?;

    // "no fabricated HW": the deliverable assembly (`build_context`) raises KeyError on a missing manifest HW fact.
    let resolved = tmp3.path().join("resolved.json");
    write(&resolved, "{\"vllm_version\": \"0.25.1\", \"torch\": {\"pin\": \"2.9.0\"}, \"ngc_base\": {\"tag\": \"26.05-py3\", \"cuda_version\": \"13.2\"}, \"build_track\": {\"decision\": \"wheel\"}, \"wheel\": {\"cuda\": \"132\", \"manylinux\": \"2_31\"}}")?;
    let template = tmp3.path().join("probe.template");
    write(&template, "ARCH={{CPU_ARCH}}\n")?;
    let (r, tp) = (ps(&resolved), ps(&template));
    let man2 = tmp3.path().join("nohw.yaml");
    write(&man2, "topology: single\ngpus_per_node: 1\nmodel_source: managed\nterraforming:\n  complete: true\n  branch_verified: true\n")?;
    let nohw = run(&["bash", &render, "--template", &tp, "--resolved", &r, "--manifest", &ps(&man2)], None, &[], None, Some(120))?;
    req(nohw.code != 0 && nohw.stderr.contains("KeyError") && nohw.stderr.contains("cpu_arch"), format!("build_context must raise, never fabricate a default cpu_arch: rc={} {}", nohw.code, tail_chars(&nohw.stderr, 400)))?;
    let ok = run(&["bash", &render, "--template", &tp, "--resolved", &r, "--manifest", &m], None, &[], None, Some(120))?;
    req(ok.code == 0 && ok.stdout.contains("ARCH=aarch64"), format!("with a real manifest fact present, the real value is used: rc={} {}{}", ok.code, ok.stdout, tail_chars(&ok.stderr, 400)))?;

    // info-only/advisory/HF-lookup capability survives Flag absence entirely.
    let cfg = json_loads("{\"quantization_config\": {\"quant_method\": \"fp8\"}}")?;
    let cprec = recipe_runtime::model_card::config_precision(&cfg).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    req(cprec.to_utf8().unwrap_or_default().starts_with("fp8"), "informational config-precision cross-check needs zero HW/Flag context")?;
    let cardp = recipe_runtime::model_card::card_precision(&PyValue::str("| DeepSeek-V4-Flash | FP4 + FP8 Mixed (experts FP4) |\n"), &PyValue::str("DeepSeek-V4-Flash"))
        .map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    let truthy = |k: &str| cardp.get(k).map_or(false, PyValue::is_truthy);
    req(truthy("mixed_note") || truthy("table_row"), "predicate requirement failed at original line 2184")
}

// ---------------------------------------------------------------- TERRAFORM_FLAG_GATE.C3

pub fn predicate_TERRAFORM_FLAG_GATE_C3() -> R {
    use role_contract::manifest_gate::{EXIT_NO_MANIFEST, REDIRECT_TEMPLATE};
    let tmp = TempDir::new("pp-flag-c3-")?;
    let t = tmp.path();
    let code = recipe_dispatch(t, &["estimate".into(), "--config".into(), ps(&t.join("config.yaml")), "--auto".into()])?;
    req(code == 4, "predicate requirement failed at original line 2209")?;
    require_recipe_dispatch_source()?;
    let rc = manifest_contract_cli(&["--topology", "single", "--repo", &ps(t), "--require-flag"])?.code;
    req(rc == EXIT_NO_MANIFEST, "predicate requirement failed at original line 2220")?;
    let out = manifest_contract_cli(&["--topology", "single", "--repo", &ps(t), "--json"])?;
    let emitted = json_loads(&out.stdout)?;
    let redirect = jget(&emitted, "redirect").ok().and_then(jstr).unwrap_or_default();
    req(matches!(jget(&emitted, "flag")?.peel(), PyValue::Bool(false)) && redirect == REDIRECT_TEMPLATE, "the real --json CLI path must emit the exact production REDIRECT_TEMPLATE constant, not a re-typed lookalike string")?;
    let run_bench_src = read(".claude/skills/adversarial-benchmark/scripts/run_bench.sh")?;
    require_flag_gate_caller(&run_bench_src, "the --require-flag argv executed as layer 1b above (reference), not a lookalike/rewritten invocation")?;
    let workflow_md = read(".claude/rules/workflow.md")?;
    let shared = "① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다.";
    req(workflow_md.contains(shared), "predicate requirement failed at original line 2246")?;
    req(REDIRECT_TEMPLATE.contains(shared), "predicate requirement failed at original line 2247")?;
    req(redirect.contains(shared), "the fragment proven live in workflow.md must be the SAME text the live CLI run actually emitted above, not merely present in the static constant somewhere else")
}

// ---------------------------------------------------------------- ROOT_SURFACE_REGISTRY.C2 / C3

pub fn predicate_ROOT_SURFACE_REGISTRY_C2() -> R {
    use sweep_stop::campaign_write::{do_purge, purge_gate_reasons, Ctx, BOOTSTRAP, JOURNEY_NAME};
    req(BOOTSTRAP == "_bootstrap", format!("예약 인스턴스 id 가 바뀌었다: {BOOTSTRAP}"))?;
    // The lifetime split itself: the skeleton is re-included (tracked), every other campaigns/ entry is ignored (volatile).
    let gi = read(".gitignore")?;
    let gl = splitlines(&gi);
    req(gl.contains(&"/campaigns/*") && gl.contains(&"!/campaigns/_template/") && gl.contains(&"!/campaigns/README.md"),
        "뼈대(_template/ · README.md)만 추적하고 인스턴스는 휘발이어야 한다(.gitignore)")?;
    let tmp = TempDir::new("pp-rsr-c2-")?;
    let root = campaign_fixture_root(tmp.path())?;
    let ctx = Ctx { root: root.clone() };
    let prev = root.join("campaigns/old");
    mkdirs(&prev)?;
    let ptr = prev.join("evidence_pointers.json");
    let reasons = || purge_gate_reasons(&ctx, "old").map_err(stop_text);
    req(reasons()?.iter().any(|r| r.contains("--abandon")), "이름 없는 잔재(orphan)는 abandoned 축소 게이트로만 정리돼야 한다")?;
    write(&prev.join(JOURNEY_NAME), "{\"utc\": \"2026-01-01T00:00:00Z\", \"kind\": \"campaign_closed\"}\n")?;
    req(!reasons()?.is_empty(), "포인터 파일이 없으면 게이트는 닫혀 있어야 한다")?;
    write(&ptr, "{\"pointers\": []}")?;
    req(reasons()?.iter().any(|r| r.contains("0건")), "빈 목록으로 게이트를 통과시키면 안 된다(공허통과 금지)")?;
    write(&ptr, "{\"pointers\": [{\"kind\": \"certificate\", \"path\": \"nope/x.yaml\"}]}")?;
    req(reasons()?.iter().any(|r| r.contains("실재하지 않는다")), "포인터가 가리키는 증거가 없으면 RED")?;
    write(&ptr, "{\"pointers\": [{\"kind\": \"certificate\", \"path\": \"CLAUDE.md\"}]}")?;
    req(!reasons()?.iter().any(|r| r.contains("relay_summary")), "원장이 없으면 요약을 요구하지 않는다(잃을 것이 없다 — 교착 금지)")?;
    write(&prev.join("relay/ctx.json"), "{}")?;
    req(reasons()?.iter().any(|r| r.contains("relay_summary")), "원장이 실재하면 그 서사(요약 testlog)를 요구해야 한다 — 원문은 휘발이다")?;
    write(&ptr, "{\"pointers\": [{\"kind\": \"certificate\", \"path\": \"CLAUDE.md\"}, {\"kind\": \"relay_summary\", \"path\": \"README.md\"}]}")?;
    let open = reasons()?;
    req(open.is_empty(), format!("전수 실재 + 요약이면 게이트가 열려야 한다: {}", repr_list(&open)))?;
    do_purge(&ctx, "old", false).map_err(stop_text)?;
    req(prev.is_dir(), "dry-run 이 지우면 안 된다")?;
    do_purge(&ctx, "old", true).map_err(stop_text)?;
    req(!prev.is_dir(), "apply 는 실제로 지워야 한다")?;
    // PURGE_KEEPS (the original's constant) as behaviour: the purge never removes the tracked skeleton.
    req(root.join("campaigns/_template/campaign.yaml").is_file() && root.join("campaigns/README.md").is_file(), "purge 가 뼈대를 지우면 안 된다: ('_template', 'README.md')")
}

pub fn predicate_ROOT_SURFACE_REGISTRY_C3() -> R {
    use role_contract::campaign::{derive_path, Derived, ModuleSite, BOOTSTRAP};
    use sweep_stop::campaign_validator::{fill_rows, strip_annotations, validate_instance, validate_template};
    use sweep_stop::campaign_write::Ctx;
    let real = repo_root();
    let root_b = ps(&real).into_bytes();
    let file_b = ps(&real.join(".claude/skills/terraforming_node/scripts/campaign_init.sh")).into_bytes();
    let site = ModuleSite { root: &root_b, file: &file_b };
    let boot = PyStr::new(BOOTSTRAP);
    for (kind, cell, sweep) in [("config", Some("c1"), None), ("lockset", Some("c1"), None), ("sweep", None, Some("s1")), ("relay-root", None, None), ("evidence", None, None)] {
        let got = derive_path(kind, cell, sweep, None, Some(&boot), Some(&root_b), site).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
        let derived = match got {
            Derived::Path(p) => String::from_utf8_lossy(&p).into_owned(),
            Derived::Refusal(r) => return Err(Fail::new("PurgeGateRefusal", r.0.to_utf8().unwrap_or_default())),
        };
        req(derived.contains("/campaigns/"), format!("파생 경로가 campaigns 밖이다({kind}): {derived}"))?;
        req(!derived.trim_end_matches('/').ends_with("/tasks"), format!("폐지된 루트 원장 자리가 살아 있다({kind}): {derived}"))?;
    }
    let x = PyStr::new("x");
    let refused = derive_path("config", None, None, None, Some(&x), Some(&root_b), site).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    req(matches!(refused, Derived::Refusal(_)), "셀 없는 셀 입력 파생은 fail-closed 여야 한다")?;

    let ctx = Ctx { root: real.clone() };
    let tv = validate_template(&ctx);
    req(tv.is_empty(), format!("뼈대가 무너졌다: {}", repr_list(&tv)))?;
    req(fill_rows("a: <<FILL>>\nb: 1\n") == vec![1], "빈칸 검출기가 발화해야 한다")?;

    let tmp = TempDir::new("pp-rsr-c3-")?;
    let camp = tmp.path().join("camp-x");
    mkdirs(&camp.join("phases/main"))?;
    write(&camp.join("cells/cell-a/config.yaml"), "cell_id: cell-a\n")?;
    let mut good = strip_annotations(&sweep_dict(&read("campaigns/_template/campaign.yaml")?)?);
    for (k, v) in [
        ("id", "\"camp-x\""),
        ("plan_ref", "\"docs/plan/p.md\""),
        ("declared_utc", "\"2026-09-06T00:00:00Z\""),
        ("nodes", "[{\"node_id\": \"main\", \"role\": \"main\", \"topology\": \"single\", \"hw\": \"gb10\"}]"),
        ("matrix", "{\"versions\": [\"0.18.0\"], \"models\": [\"m\"]}"),
        ("assignments", "{\"main\": [{\"cell\": \"cell-a\"}]}"),
        ("budgets", "{\"smoke_budget_overhead_mib\": 1, \"ready_max_seconds\": 1}"),
        ("control_variables", "{\"model\": \"m\", \"vllm_version\": \"0.18.0\", \"topology\": \"single\", \"target_gpu\": \"H100\"}"),
        ("hint_targets", "[{\"node_id\": \"main\", \"cells\": [\"cell-a\"], \"approval\": {\"approved_by\": \"fixture transcription\", \"approved_utc\": \"2026-09-06T00:00:00Z\", \"source\": \"declaration-popup\"}}]"),
    ] {
        good.set(k, sweep(v)?);
    }
    let text = sweep_stop::pyvalue::PyValue::Dict(good).dumps_unicode().map_err(|e| Fail::new("TypeError", format!("{e:?}")))?;
    write(&camp.join("campaign.yaml"), &text.to_utf8().unwrap_or_default())?;
    let st = camp.join("phases/main/serve.status.json");
    let body = |source: &str| format!("{{\"schema_version\": 1, \"node_id\": \"main\", \"phase\": \"serve\", \"cell_id\": \"cell-a\", \"state\": \"done\", \"proof\": {{\"predicate\": \"health200\", \"ok\": true, \"source\": {}}}}}", json_str(source));
    let vi = |c: &Path| validate_instance(&ctx, c).map_err(|e| match e { sweep_stop::campaign_validator::Failure::Contract(t) => Fail::new("CampaignContractFailure", t), sweep_stop::campaign_validator::Failure::Owner => Fail::new("CampaignContractFailure", "owner-rendered failure (schema unreadable)") });
    write(&st, &body(""))?;
    req(vi(&camp)?.iter().any(|p| p.contains("source 가 비었다")), "출처 없는 proof.ok 는 차단해야 한다 — 단언이 검증을 대체하는 자리")?;
    write(&st, &body("docs/testlog/t.md"))?;
    let left = vi(&camp)?;
    req(left.is_empty(), format!("출처가 붙으면 통과해야 한다: {}", repr_list(&left)))
}

// ---------------------------------------------------------------- LIBRARY_GROUNDING_FAIL_CLOSED.C1..C4

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C1() -> R {
    use sweep_stop::campaign_write::{grounding_terms, FILL};
    let long = "x".repeat(200);
    let decl = sweep_dict(&format!("{{\"control_variables\": {{\"model\": \"Qwen3-4B\", \"vllm_version\": \"0.26.0\", \"_note\": \"사람 주석\", \"big\": {}}}}}", json_str(&long)))?;
    let terms = grounding_terms(&decl);
    req(terms.iter().any(|t| t == "Qwen3-4B") && terms.iter().any(|t| t == "0.26.0"), format!("통제변인이 질의어로 파생되지 않았다: {}", repr_list(&terms)))?;
    req(!terms.iter().any(|t| t.starts_with("사람")), "사람 주석(`_` 키)이 질의어로 샜다")?;
    req(!terms.iter().any(|t| t.chars().count() > 80), "긴 산문이 질의어로 샜다(질의가 아니라 문서다)")?;
    req(grounding_terms(&sweep_dict(&format!("{{\"control_variables\": {{\"model\": {}}}}}", json_str(FILL)))?).is_empty(), "빈칸이 질의어가 됐다 — 모르는 값으로 도서관에 묻지 않는다")?;
    // `ground_campaign` of the original = the Rust `op_ground`: the record carries the three messages.
    let body = rust_fn_body(&read(CAMPAIGN_WRITE_RS)?, "op_ground")?;
    for token in ["request", "export", "attestation"] {
        req(body.contains(&format!("doc.set(\"{token}\"")), format!("그라운딩 기록에 {token} 메시지가 없다 — 3메시지 모양이 정본이다"))?;
    }
    Ok(())
}

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C2() -> R {
    use sweep_stop::campaign_write::{grounding_reasons, Ctx};
    let tmp = TempDir::new("pp-lg-c2-")?;
    let camp = tmp.path().join("camp-g");
    mkdirs(&camp)?;
    let ctx = Ctx { root: tmp.path().to_path_buf() };
    req(grounding_reasons(&ctx, &camp).iter().any(|r| r.contains("그라운딩 기록이 없다")), "기록 부재가 통과했다 — 백스톱이 fail-closed 가 아니다")?;
    grounding_fixture(&camp, "resolved", None)?;
    req(grounding_reasons(&ctx, &camp).is_empty(), "기록이 있는데 막았다(과잉차단)")?;
    grounding_fixture(&camp, "unresolved", Some("정직한 공백"))?;
    req(grounding_reasons(&ctx, &camp).is_empty(), "사서가 못 찾은 것을 차단했다 — 공백으로 막으면 새 주제를 영영 못 돈다")?;
    grounding_fixture(&camp, "refused", None)?;
    req(grounding_reasons(&ctx, &camp).iter().any(|r| r.contains("거절")), "거절을 통과시켰다 — 공백과 거절은 다른 사실이다")?;
    // The executors are real. single_serve_up.sh calls the backstop; run_trial (the Rust session) runs the campaign writer's
    // `--grounding-check` before any docker session and stops when it refuses (plan_26111615: the writer is campaign_init.sh).
    let rel = ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh";
    req(read(rel)?.contains("--grounding-check"), format!("{rel} 에 진입 백스톱 호출이 없다 — 실행자 없는 게이트는 교착도 아니다"))?;
    use recipe_runtime::run_trial_session::{pre_session_gates, SessionError, SessionOpts, CAMPAIGN_WRITER_REL};
    let opts = SessionOpts { no_budget: true, ..Default::default() };
    let with_writer = |rc: i32, tag: &str| -> R<Result<(), SessionError>> {
        let rt = TempDir::new(tag)?;
        let w = rt.path().join(CAMPAIGN_WRITER_REL);
        mkdirs(w.parent().unwrap())?;
        // stand-in writer: records the argv it got, answers the grounding check with `rc`
        write(&w, &format!("#!/bin/bash\nprintf '%s\\n' \"$*\" > \"$(dirname \"$0\")/argv\"\nexit {rc}\n"))?;
        let got = pre_session_gates(rt.path(), &opts);
        let argv = std::fs::read_to_string(w.parent().unwrap().join("argv")).unwrap_or_default();
        req(argv.trim() == "--grounding-check", format!("run_trial 이 writer 를 --grounding-check 로 부르지 않았다: {argv:?}"))?;
        Ok(got)
    };
    req(matches!(with_writer(1, "pp-lg-c2-refuse-")?, Err(SessionError::Stop { .. })),
        ".claude/skills/vllm-recipe-explorer/scripts/run_trial.sh 에 진입 백스톱 호출이 없다 — 그라운딩 거부에도 트라이얼에 진입했다")?;
    req(with_writer(0, "pp-lg-c2-pass-")?.is_ok(), "그라운딩 백스톱이 통과를 막았다(과잉차단)")
}

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C3() -> R {
    use sweep_stop::campaign_write::{write_cell, Args, Ctx};
    // Source witness of the writer (the behaviour is executed below): the absence of a record is written as `absent`.
    let wc = rust_fn_body(&read(CAMPAIGN_WRITE_RS)?, "write_cell")?;
    req(wc.contains("doc.set(\"grounding\"") && wc.contains("grounding.set(\"status\", py(\"absent\"))"),
        "셀 상태 writer 가 그라운딩 자기표시(부재 = absent)를 쓰지 않는다")?;
    let tmp = TempDir::new("pp-lg-c3-")?;
    let camp = tmp.path().join("camp-g");
    mkdirs(&camp.join("cells"))?;
    write(&camp.join("campaign.yaml"), "{\"schema_version\": 1, \"id\": \"camp-g\", \"assignments\": {\"main\": [{\"cell\": \"c1\"}]}}")?;
    let ctx = Ctx { root: tmp.path().to_path_buf() };
    let mut values: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    values.insert("--node", vec!["main".into()]);
    values.insert("--utc", vec!["t".into()]);
    let args = Args::from_values(values);
    let status = camp.join("cells/c1/cell.status.json");
    let grounding = || -> R<PyValue> {
        let doc = json_loads(&read_path(&status)?)?;
        Ok(jget(&doc, "grounding").map(Clone::clone).unwrap_or(PyValue::None))
    };
    write_cell(&ctx, &camp, &args, "c1", "pending").map_err(stop_text)?;
    let g = grounding()?;
    req(jget(&g, "status").ok().and_then(jstr).as_deref() == Some("absent"), "기록이 없는데 산출물이 침묵했다 — 부재와 '참조했다'가 데이터에서 갈리지 않는다")?;
    grounding_fixture(&camp, "unresolved", Some("정직한 공백"))?;
    write_cell(&ctx, &camp, &args, "c1", "pending").map_err(stop_text)?;
    let g = grounding()?;
    let truthy = |k: &str| jget(&g, k).map_or(false, PyValue::is_truthy);
    req(jget(&g, "status").ok().and_then(jstr).as_deref() == Some("unresolved") && truthy("source") && truthy("gap"), format!("그라운딩 자기표시가 불완전하다: {}", g.repr()))
}

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C4() -> R {
    let src = read(CAMPAIGN_WRITE_RS)?;
    req(src.contains("fn warm_start_library("), "입고 실행자가 없다")?;
    let pubsrc = read(".claude/rust/policy-publisher/src/publisher.rs")?;
    let fin = rust_fn_body(&pubsrc, "cmd_finalize")?;
    req(fin.contains(".claude/skills/wiki-desk/src/cli.ts") && fin.contains("\"sync\", \"--apply\""), "발행기 종료부가 입고를 부르지 않는다 — 발행과 입고가 갈라지면 서가는 늘 한 캠페인 늦다")?;
    req(src.contains("서가 입고(C4)"), "publish 위상 종료부의 입고 호출이 사라졌다")?;
    let body = rust_fn_body(&src, "warm_start_library")?;
    req(body.contains("\"sync\".to_string(), \"--apply\".to_string()") && body.contains("WIKI_CLI"), "publish 위상의 입고가 사서의 입고(sync --apply) 계약을 부르지 않는다")?;
    // incremental intake, proven by execution (wiki-desk's real CLI).
    let cli = ps(&repo_root().join(".claude/skills/wiki-desk/src/cli.ts"));
    let mut contract = json_loads(&read(".claude/skills/wiki-desk/assets/project-contract.example.json")?)?;
    if let PyValue::Dict(d) = &mut contract {
        d.set("source_roots", PyValue::List(vec![PyValue::str("docs")]));
        d.set("excluded_roots", PyValue::List(Vec::new()));
    }
    let tmp = TempDir::new("pp-lg-c4-")?;
    let repo = tmp.path();
    write(&repo.join("docs/plan_26100101_a.md"), "# a\n")?;
    write(&repo.join("contract.json"), &contract.dumps_ascii().map_err(|e| Fail::new("TypeError", format!("{e:?}")))?)?;
    let r = ps(repo);
    let wiki = |a: &[&str]| -> R<Proc> {
        let mut argv = vec!["node", cli.as_str()];
        argv.extend_from_slice(a);
        argv.extend(["--root", r.as_str(), "--local"]);
        run(&argv, None, &[], None, Some(300))
    };
    let c = ps(&repo.join("contract.json"));
    req(wiki(&["init", "--contract", &c, "--apply"])?.code == 0, "init 실패")?;
    let (first, second) = (wiki(&["sync", "--apply"])?, wiki(&["sync", "--apply"])?);
    req(first.code == 0 && second.code == 0, format!("입고 실패: {}{}", first.stderr, second.stderr))?;
    let created = jget(&json_loads(&first.stdout)?, "created").map_or(false, PyValue::is_truthy);
    let writes = jget(&json_loads(&second.stdout)?, "writes").map_or(false, PyValue::is_truthy);
    req(created && !writes, "입고가 증분이 아니다(같은 원문 재입고가 다시 쓴다)")
}
