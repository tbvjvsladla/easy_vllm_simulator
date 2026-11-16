//! hint_api bridge for `evidence` (arms grouped by part).
use std::path::{Component, Path, PathBuf};

use crate::api::{Call, Res};
use crate::core::HintError;
use crate::evidence as e;
use crate::value::{Map, J};
use crate::{jd, jl};

/// Python `str(Path(...))` of a path built from the request's (relative) repo argument: `.` segments collapse.
fn enc_py_path(p: &Path) -> J {
    let mut out = PathBuf::new();
    for c in p.components() {
        if c != Component::CurDir {
            out.push(c.as_os_str());
        }
    }
    let s = crate::core::posix(&out);
    jd! {"__path__" => if s.is_empty() { ".".to_string() } else { s }}
}

fn enc_paths(v: &[PathBuf]) -> J {
    J::List(v.iter().map(|p| enc_py_path(p)).collect())
}

fn opt_s(v: Option<String>) -> J {
    J::from(v)
}

fn strs(v: Vec<String>) -> J {
    J::from(v)
}

/// `CellEvidence(**kwargs)` — the dataclass as a J dict (fields in declaration order, defaults filled).
const CELL_EVIDENCE_FIELDS: [(&str, &str); 36] = [
    ("mode", "!"), ("campaign_id", "!"), ("cell", "!"), ("node", "!"), ("topology", "!"), ("declaration", "n"),
    ("cell_status", "n"), ("lockset", "n"), ("cell_config", "n"), ("triplet", "d"), ("sweep", "n"), ("certificate", "n"),
    ("certificate_path", "n"), ("bench_report_path", "n"), ("report_kind", "n"), ("pointers", "l"), ("attestation", "n"),
    ("image_ledger", "n"), ("build_identity", "d"), ("resolve", "n"), ("manifest", "n"), ("publication", "n"),
    ("lineage_seeds", "d"), ("missing", "l"), ("serve_proof", "n"), ("plane", "n"), ("native_install_path", "n"),
    ("pip_freeze_path", "n"), ("pip_freeze_paths", "d"), ("cleanup_attestation_path", "n"), ("cleanup_attestation", "n"),
    ("native_launch_path", "n"), ("native_launch_error", "n"), ("phases", "d"), ("sources", "d"), ("repo", "n"),
];

fn cell_evidence(c: &Call) -> Res {
    let mut m = Map::new();
    m.insert("__class__", "CellEvidence");
    let mut missing = Vec::new();
    for (i, (name, dflt)) in CELL_EVIDENCE_FIELDS.iter().enumerate() {
        let v = match c.arg(i, name) {
            Some(v) => v.clone(),
            None => match *dflt {
                "n" => J::Null,
                "d" => J::dict(),
                "l" => J::list(),
                _ => {
                    missing.push(format!("'{name}'"));
                    J::Null
                }
            },
        };
        m.insert(*name, v);
    }
    if !missing.is_empty() {
        let n = missing.len();
        // Python's wording: "'a' and 'b'" · "'a', 'b', and 'c'" (serial comma from three on).
        let list = match n {
            1 => missing[0].clone(),
            2 => format!("{} and {}", missing[0], missing[1]),
            _ => format!("{}, and {}", missing[..n - 1].join(", "), missing[n - 1]),
        };
        return Err(HintError::py(
            "TypeError",
            format!("CellEvidence.__init__() missing {n} required positional argument{}: {list}", if n == 1 { "" } else { "s" }),
        ));
    }
    Ok(J::Dict(m))
}

fn rows_j(rows: Vec<(String, i64, J)>) -> J {
    J::List(rows.into_iter().map(|(t, l, d)| jl![t, l, d]).collect())
}

// ── e3 helpers ──
/// A `{Path: str}` result re-keyed as Python `str(Path)` (`.` segments collapse).
fn e3_rekey(d: &J) -> J {
    let mut m = Map::new();
    if let Some(src) = d.as_dict() {
        for (k, v) in src.iter() {
            m.insert(enc_py_path(Path::new(k))["__path__"].display(), v.clone());
        }
    }
    J::Dict(m)
}

/// A digest-sweep dict whose `dir` is a Path in Python.
fn e3_dir_path(s: &J) -> J {
    let mut s = s.clone();
    if let Some(d) = s.get_str("dir").map(str::to_string) {
        s.set("dir", enc_py_path(Path::new(&d)));
    }
    s
}

/// A `CellEvidence` result in the dataclass encoding (`__class__` first).
fn e3_enc_ev(v: &J) -> J {
    let mut m = Map::new();
    m.insert("__class__", "CellEvidence");
    if let Some(src) = v.as_dict() {
        for (k, x) in src.iter() {
            if k != "__class__" {
                m.insert(k, x.clone());
            }
        }
    }
    J::Dict(m)
}

/// A tuple/list argument (`()` default) as a J list.
fn e3_seq(v: Option<&J>) -> J {
    J::List(v.map(crate::api::unwrap_seq).unwrap_or_default())
}

pub fn call(c: &Call) -> Option<Res> {
    let repo = || c.path(0, "repo");
    Some(match c.func.as_str() {
        // ── shared ──
        "CellEvidence" => cell_evidence(c),
        // ── e1 ──
        "_unobserved" => e::_unobserved(&c.j(0, "value")).map(J::Bool),
        "_norm_value" => e::_norm_value(&c.j(0, "value")),
        "_as_int" => e::_as_int(&c.j(0, "value")).map(J::from),
        "_boolish" => e::_boolish(&c.j(0, "value")).map(J::from),
        "_check_campaign_id" => e::_check_campaign_id(&c.j(0, "campaign_id")).map(J::Str),
        "_check_cell" => e::_check_cell(&c.j(0, "cell")).map(J::Str),
        "_check_node" => e::_check_node(&c.j(0, "node")).map(J::Str),
        "_read_json_opt" => e::_read_json_opt(&c.path(0, "path"), &c.str(1, "what")),
        "_manifest" => e::_manifest(&repo(), &c.str(1, "topology")).map(|(d, s)| jl![J::from(d), s]),
        "parse_certificate" => {
            let p = c.arg(1, "path").map(|v| J::from(crate::core::posix(&crate::api::decode_path(v)))).unwrap_or(J::Null);
            e::parse_certificate(&repo(), &p)
        }
        "_identity_from_certificate" => e::_identity_from_certificate(&repo(), &c.j(1, "cert")),
        "authorize" => {
            let p = c.arg(1, "manifest_path").map(|v| J::from(crate::core::posix(&crate::api::decode_path(v)))).unwrap_or(J::Null);
            e::authorize(&repo(), &p, &c.str(2, "action"))
        }
        "no_cert_binding_source" => {
            let r = c.opt_path(1, "repo");
            e::no_cert_binding_source(&c.j(0, "manifest"), r.as_deref()).map(opt_s)
        }
        "binding_artifact_path" => {
            let r = c.opt_path(1, "repo");
            e::binding_artifact_path(&c.j(0, "manifest"), r.as_deref()).map(opt_s)
        }
        "promotion_binding_problems" => {
            e::promotion_binding_problems(&c.j(0, "manifest"), &c.str(1, "tag"), &c.str(2, "topology"), &c.str(3, "anchor"))
                .map(|v| J::List(v.into_iter().map(|(a, b)| jl![a, b]).collect()))
        }
        "publication_records" => e::publication_records(&repo()).map(J::List),
        "_sweep_doc" => e::_sweep_doc(&repo(), &c.path(1, "directory"), &c.str(2, "binding")).map(J::from),
        "_output_sweeps" => {
            let cell = c.opt_str(2, "cell");
            e::_output_sweeps(&repo(), &c.str(1, "topology"), cell.as_deref()).map(J::List)
        }
        "_bind_sweep" => {
            let mu = c.opt_str(3, "measured_utc");
            e::_bind_sweep(&repo(), &c.str(1, "topology"), &c.str(2, "cell"), mu.as_deref(), &c.j(4, "simlog_dirs"),
                &c.j(5, "sweep_dir"), &c.j(6, "sweep_config"), &c.j(7, "sweep_generated_utc"))
                .map(|(d, m, s)| jl![J::from(d), strs(m), s])
        }
        "_internal_doc_refs" => {
            let t = c.j(0, "text");
            e::_internal_doc_refs(&if t.truthy() { t.display() } else { String::new() }).map(strs)
        }
        "_docker" => e::_docker(&J::Null, &c.args.iter().skip(1).map(|a| a.display()).collect::<Vec<_>>()).map(opt_s),
        "_history_build_args" => {
            let t = c.opt_str(0, "text");
            e::_history_build_args(t.as_deref()).map(|(a, b)| jl![a, b])
        }
        "_image_facts" => e::_image_facts(&c.str(0, "ref"), &J::Null).map(J::from),
        "_env_first" => {
            let r = c.opt_str(1, "rel_path");
            e::_env_first(&repo(), r.as_deref())
        }
        "_dockerfile_args" => {
            let d = c.opt_str(2, "dockerfile");
            e::_dockerfile_args(&repo(), &c.str(1, "topology"), d.as_deref()).map(|(a, s)| jl![J::from(a), s])
        }
        "compose_build_dockerfile" => {
            let p = c.arg(0, "compose_path").map(|v| match v {
                J::Dict(_) => J::from(crate::core::posix(&crate::api::decode_path(v))),
                o => o.clone(),
            });
            e::compose_build_dockerfile(&p.unwrap_or(J::Null)).map(|(a, b)| jl![J::from(a), J::from(b)])
        }
        "_history_track_dockerfile" => e::_history_track_dockerfile(&c.j(0, "raw_args")).map(|(a, b)| jl![J::from(a), b]),
        "_build_identity" => {
            let er = c.kw("env_rel").and_then(J::as_str).map(str::to_string);
            e::_build_identity(&repo(), &c.kw("topology").and_then(J::as_str).unwrap_or_default().to_string(),
                &c.kw("env").cloned().unwrap_or(J::Null), er.as_deref(), &c.kw("measured").cloned().unwrap_or(J::Null),
                &c.kw("manifest").cloned().unwrap_or(J::Null), &J::Null)
                .map(|(o, m)| jl![o, strs(m)])
        }
        "_attested_host" => {
            let t = c.opt_str(2, "timing_note");
            e::_attested_host(&c.j(0, "att"), &c.str(1, "key"), t.as_deref()).map(|(v, s, b)| jl![J::from(v), J::from(s), b])
        }
        "_node_image_id_note" => e::_node_image_id_note(&c.j(0, "att"), &c.str(1, "iid")).map(J::Str),
        "_utc_z_soft" => e::_utc_z_soft(&c.j(0, "v")).map(opt_s),
        "_bind_resolve" => e::_bind_resolve(&repo(), &c.str(1, "topology"), &c.j(2, "build")).map(|(d, s)| jl![J::from(d), s]),
        "_bind_attestation" => e::_bind_attestation(&repo(), &c.str(1, "topology"), &c.str(2, "node"), &c.str(3, "cell"), &c.j(4, "build"))
            .map(|(d, m, s)| jl![J::from(d), strs(m), s]),
        "_attestation_has_rows" => e::_attestation_has_rows(&c.j(0, "doc")).map(J::Bool),
        "_triplet" => e::_triplet(&repo(), &c.str(1, "topology"), &c.str(2, "cell")),
        "_serve_proof" => e::_serve_proof(&repo(), &c.str(1, "topology"), &c.str(2, "cell")).map(|(d, s)| jl![J::from(d), J::from(s)]),
        "_preserved_native_file" => e::_preserved_native_file(&repo(), &c.j(1, "raw"), &c.str(2, "field"))
            .map(|(s, p)| jl![s, enc_py_path(&p)]),
        "_native_cleanup_attestation" => e::_native_cleanup_attestation(&repo(), &c.j(1, "proof")).map(|(s, d)| jl![s, d]),
        "_native_producer_evidence" => {
            let pr = c.opt_str(4, "proof_rel");
            e::_native_producer_evidence(&repo(), &c.str(1, "topology"), &c.str(2, "cell"), &c.j(3, "proof"), pr.as_deref())
        }
        "_measured_meta" => e::_measured_meta(&c.j(0, "ev_sweep"), &c.j(1, "cert")),
        "_report_born_utc" => {
            let r = c.opt_str(1, "rel_path");
            e::_report_born_utc(&repo(), r.as_deref()).map(opt_s)
        }
        "_require_report_joins_certificate" => {
            let r = c.opt_str(1, "report_rel");
            e::_require_report_joins_certificate(&repo(), r.as_deref(), &c.j(2, "cert")).map(|_| J::Null)
        }
        "_report_twin" => {
            let r = c.opt_str(0, "cert_rel");
            e::_report_twin(r.as_deref()).map(opt_s)
        }
        "_lite_snapshot" => {
            let r = c.opt_str(1, "rel_path");
            e::_lite_snapshot(&repo(), r.as_deref())
        }
        "_report_missing" => {
            let r = c.opt_str(2, "report_rel");
            e::_report_missing(&repo(), &c.j(1, "cert"), r.as_deref()).map(|(m, k)| jl![strs(m), J::from(k)])
        }
        "measurement_config" => e::measurement_config(&repo(), &c.j(1, "ev")),
        "_sanitize_dataset_value" => e::_sanitize_dataset_value(&c.j(0, "v")),
        "dataset_provenance" => e::dataset_provenance(&repo(), &c.j(1, "ev")),
        "_surface" => e::_surface(&c.j(0, "v")).map(opt_s),
        "_dig" => e::_dig(&c.j(0, "doc"), &c.str(1, "dotted")),
        "_report_verdict_table" => {
            let r = c.opt_str(1, "rel_path");
            e::_report_verdict_table(&repo(), r.as_deref())
        }
        "_lite_report_numbers" => {
            let r = c.opt_str(1, "rel_path");
            e::_lite_report_numbers(&repo(), r.as_deref())
        }
        "_judged" => e::_judged(&repo(), &c.j(1, "ev")).map(|(a, b, r)| jl![a, b, strs(r)]),
        "_certificate_not_due" => e::_certificate_not_due(&repo(), &c.j(1, "ev")).map(opt_s),
        "_num_equal" => e::_num_equal(&c.str(0, "a"), &c.str(1, "b")).map(J::Bool),
        "cert_is_lite" => e::cert_is_lite(&c.j(0, "cert")).map(J::Bool),
        "measurement" => e::measurement(&c.j(0, "ev")),
        "_kv_declared_note" => e::_kv_declared_note(&c.j(0, "v"), &c.j(1, "clamp"), &c.str(2, "rel")).map(opt_s),
        // ── e2 ──
        "_dur" => e::_dur(&c.j(0, "a"), &c.j(1, "b")).map(opt_s),
        "_label_is_cell" => e::_label_is_cell(&c.j(0, "label"), &c.str(1, "cell")).map(J::Bool),
        "_session_is_cell" => e::_session_is_cell(&c.j(0, "sid"), &c.str(1, "cell")).map(J::Bool),
        "_node_session_docs" => e::_node_session_docs(&repo(), &c.str(1, "node")),
        "_session_event_is_cell" => {
            e::_session_event_is_cell(&c.j(0, "d"), &c.str(1, "ts"), &c.j(2, "ev"), &c.j(3, "sessions")).map(J::Bool)
        }
        "_event_fields" => e::_event_fields(&c.j(0, "d")).map(J::Str),
        "_measured_key" => e::_measured_key(&repo(), &c.j(1, "ev")).map(opt_s),
        "_event_node_dirs" => e::_event_node_dirs(&repo(), &c.j(1, "ev")).map(strs),
        "_node_event_files" => e::_node_event_files(&repo(), &c.str(1, "node")).map(|(f, s)| jl![enc_paths(&f), s]),
        "_event_rows" => e::_event_rows(&c.path(0, "path")).map(|(r, b)| jl![rows_j(r), b]),
        "_node_streams" => e::_node_streams(&repo(), &c.str(1, "node"))
            .map(|v| J::List(v.into_iter().map(|(k, r, s, b)| jl![k, rows_j(r), strs(s), b]).collect())),
        "_node_opens" => e::_node_opens(&c.j(0, "streams")).map(|v| J::List(v.into_iter().map(|(t, l)| jl![t, J::from(l)]).collect())),
        "_event_windows" => e::_event_windows(&c.j(0, "rows"), &c.j(1, "node_opens")).map(J::List),
        "_ledger_timeline" => e::_ledger_timeline(&repo(), &c.j(1, "ev"), &J::Null)
            .map(|(r, f, w)| jl![J::List(r), strs(f), J::from(w)]),
        "foreign_label_note" => e::foreign_label_note(&c.j(0, "label"), &c.str(1, "cell")).map(J::Str),
        "_win_holds" => e::_win_holds(&c.j(0, "w"), &c.str(1, "t")).map(J::Bool),
        "_merge_stream_copies" => e::_merge_stream_copies(&c.j(0, "rows")).map(J::List),
        "_startup_markers" => e::_startup_markers(&repo(), &c.j(1, "ev"), &c.j(2, "win")).map(J::List),
        "_timeline_key" => e::_timeline_key(&c.j(0, "r")),
        "event_timeline" => e::event_timeline(&repo(), &c.j(1, "ev")).map(J::List),
        "_self_node" => e::_self_node(&repo()).map(|(n, s)| jl![J::from(n), s]),
        "measurement_campaign" => e::measurement_campaign(&c.j(0, "ev")),
        "campaign_instances_of" => e::campaign_instances_of(&repo(), &c.str(1, "cell")).map(J::List),
        "campaign_absence" => {
            let r = c.opt_path(1, "repo");
            e::campaign_absence(&c.j(0, "ev"), r.as_deref())
        }
        "event_ledger_spans" => e::event_ledger_spans(&repo(), &c.j(1, "ev")).map(J::List),
        "_smoke_ledger_echo" => e::_smoke_ledger_echo(&repo(), &c.j(1, "ev"), &c.str(2, "node")).map(J::from),
        "event_files" => e::event_files(&repo(), &c.j(1, "ev")).map(strs),
        "_campaign_boundary" => {
            let r = c.opt_path(2, "repo");
            e::_campaign_boundary(&c.j(0, "ev"), &c.j(1, "win"), r.as_deref())
        }
        "event_campaign_boundary" => e::event_campaign_boundary(&repo(), &c.j(1, "ev")),
        "_note_same_cell" => e::_note_same_cell(&c.j(0, "bnd"), &c.j(1, "same")),
        "_cell_simlog_logs" => e::_cell_simlog_logs(&repo(), &c.j(1, "ev")).map(|v| enc_paths(&v)),
        "_same_cell_pre_sessions" => {
            let b = c.opt_str(2, "b");
            e::_same_cell_pre_sessions(&repo(), &c.j(1, "ev"), b.as_deref()).map(J::List)
        }
        "_scope_rows" => e::_scope_rows(&c.j(0, "rows"), &c.j(1, "bnd"), &c.j(2, "same_cell")).map(J::List),
        "_timeline_parts" => e::_timeline_parts(&repo(), &c.j(1, "ev"))
            .map(|(r, f)| jl![J::List(r), J::List(f.into_iter().map(|(a, b)| jl![a, b]).collect())]),
        "_json_key_line" => e::_json_key_line(&c.path(0, "path"), &c.str(1, "key"), &c.j(2, "value")).map(J::from),
        "_other_measurements" => e::_other_measurements(&repo(), &c.j(1, "ev")).map(J::List),
        "_measurement_rows" => e::_measurement_rows(&repo(), &c.j(1, "ev"), &c.j(2, "ctx"))
            .map(|(r, f)| jl![J::List(r), J::List(f.into_iter().map(|(a, b)| jl![a, b]).collect())]),
        "bench_definition" => e::bench_definition(&repo(), &c.j(1, "ev")),
        "bench_command" => e::bench_command(&repo(), &c.j(1, "ev")),
        "_utc_of_epoch" => e::_utc_of_epoch(c.int_or(0, "ep", 0)).map(J::Str),
        "_git_repo" => e::_git_repo(&repo()).map(J::Bool),
        "_head_at" => {
            let u = c.opt_str(1, "utc");
            e::_head_at(&repo(), u.as_deref()).map(|(s, m)| jl![J::from(s), m])
        }
        "_git_text" => e::_git_text_bytes(&repo(), &c.str(1, "rev"), &c.str(2, "rel_path")).map(|b| match b {
            Some(b) => jd! {"__bytes__" => b.iter().map(|x| format!("{x:02x}")).collect::<String>()},
            None => J::Null,
        }),
        "_tool_revision" => e::_tool_revision(&repo(), &c.str(1, "at"), &c.str(2, "rel_path")).map(J::from),
        "_measured_bench_tool" => e::_measured_bench_tool(&c.j(0, "ev")).map(opt_s),
        "_tool_selection" => e::_tool_selection(&repo(), &c.j(1, "ev")),
        "tool_snapshots" => {
            let mut ev = c.j(1, "ev");
            let d = c.opt_path(2, "draft_dir");
            e::tool_snapshots_mut(&repo(), &mut ev, d.as_deref()).map(J::List)
        }
        "_sweep_engine_logs" => e::_sweep_engine_logs(&c.path(0, "base"), &c.j(1, "idx"), &c.j(2, "levels"), &c.str(3, "cell"))
            .map(|v| enc_paths(&v)),
        "_measured_engine_logs" => e::_measured_engine_logs(&repo(), &c.j(1, "ev")).map(|(v, w)| jl![enc_paths(&v), w]),
        // ── e3 ──
        "_run_marks" => e::_run_marks(&c.path(0, "path")),
        "_mmdd" => e::_mmdd(c.opt_str(0, "utc").as_deref()).map(opt_s),
        "_same_run_candidates" => e::_same_run_candidates(&repo(), &c.j(1, "ev"), &c.j(2, "have")).map(|v| enc_paths(&v)),
        "_same_run_copies" => e::_same_run_copies(&repo(), &c.j(1, "ev"), &c.j(2, "tails")).map(|d| e3_rekey(&d)),
        "_measured_engine_logs_ex" => e::_measured_engine_logs_ex(&repo(), &c.j(1, "ev")).map(|(v, w, d)| jl![enc_paths(&v), w, e3_rekey(&d)]),
        "_lite_join_key" => e::_lite_join_key(&repo(), &c.j(1, "ev")).map(|(a, b)| jl![opt_s(a), b]),
        "lite_join" => e::lite_join(&repo(), &c.j(1, "ev")).map(|(d, r, w)| jl![d.map(|p| enc_py_path(&p)).unwrap_or(J::Null), J::from(r), w]),
        "_measured_engine_logs_base" => e::_measured_engine_logs_base(&repo(), &c.j(1, "ev")).map(|(v, w)| jl![enc_paths(&v), w]),
        "_nonempty" => e::_nonempty(&c.path(0, "p")).map(J::Bool),
        "_native_preserved_engine_logs" => e::_native_preserved_engine_logs(&repo(), &c.j(1, "ev")).map(|v| enc_paths(&v)),
        "_log_lines" => e::_log_lines(&c.path(0, "path")).map(strs),
        "_node_of_line" => e::_node_of_line(&c.str(0, "line"), &c.j(1, "table")).map(|(a, b)| jl![opt_s(a), opt_s(b)]),
        "_log_tail_caps" => e::_log_tail_caps(&repo(), &c.j(1, "ev")),
        "_tail_windows" => e::_tail_windows(&repo(), &c.j(1, "ev"), &c.j(2, "logs"), &c.j(3, "copies")),
        "_head_windows" => e::_head_windows(&repo(), &c.j(1, "ev"), &c.j(2, "logs")),
        "measurement_env_observed" => e::measurement_env_observed(&repo(), &c.j(1, "ev")).map(J::List),
        "_banners_of" => e::_banners_of(&c.path(0, "path")).map(|v| J::List(v.into_iter().map(|(a, b, d)| jl![a, b, opt_s(d)]).collect())),
        "_banner_of" => e::_banner_of(&c.path(0, "path")).map(|v| v.map(|(a, b, d)| jl![a, b, opt_s(d)]).unwrap_or(J::Null)),
        "_banner_in_window" => e::_banner_in_window(c.opt_str(0, "stamp").as_deref(), &c.str(1, "start"), &c.str(2, "end")).map(opt_s),
        "_read_json_soft" => e::_read_json_soft(&c.path(0, "path")),
        "_digest_sweeps" => e::_digest_sweeps(&repo(), &c.j(1, "ev"), c.opt_str(2, "digest").as_deref(), c.opt_str(3, "tag").as_deref())
            .map(|v| J::List(v.iter().map(e3_dir_path).collect())),
        "_engine_banner" => e::_engine_banner(&repo(), &c.j(1, "ev")),
        "_bracketed" => e::_bracketed(&c.str(0, "mmdd_hms"), &c.j(1, "brackets"), c.opt_str(2, "measured").as_deref())
            .map(|v| v.map(|(t, a, b)| jl![t, a, b]).unwrap_or(J::Null)),
        "_pick_banner" => e::_pick_banner(&c.j(0, "cands"), c.opt_str(1, "measured").as_deref(), &c.str(2, "basis"), &c.str(3, "what")),
        "_lite_cert_vllm_producer" => e::_lite_cert_vllm_producer(&repo(), &c.j(1, "ev"), &c.str(2, "cert_v")).map(|v| v.map(|(a, b)| jl![a, b]).unwrap_or(J::Null)),
        "_cert_vllm_producer" => e::_cert_vllm_producer(&repo(), &c.j(1, "ev"), &c.j(2, "sel")).map(|(a, b)| jl![a, b]),
        "_runtime_observations" => e::_runtime_observations(&repo(), &c.j(1, "ev")).map(J::List),
        "_runtime_observed_field" => e::_runtime_observed_field(&c.j(0, "observations"), &c.str(1, "key")).map(|(a, b)| jl![opt_s(a), b]),
        "vllm_observed" => {
            let r = c.opt_path(1, "repo");
            e::vllm_observed(&c.j(0, "ev"), r.as_deref())
        }
        "bind_image_ledger" => {
            let mut ev = c.j(0, "ev");
            e::bind_image_ledger_mut(&mut ev, &c.j(1, "view")).map(|_| J::Null)
        }
        "_wheel_meta" => e::_wheel_meta(&repo(), &c.j(1, "ev")).map(|(a, b)| jl![opt_s(a), b]),
        "attestation_scope" => e::attestation_scope(&repo(), &c.j(1, "ev")).map(J::from),
        "_attestation_written" => e::_attestation_written(&repo(), &c.j(1, "att")).map(|(a, b)| jl![opt_s(a), b]),
        "_attestation_timing_note" => e::_attestation_timing_note(&c.j(0, "ev")).map(opt_s),
        "_smoke_serve_logs" => e::_smoke_serve_logs(&repo()).map(|v| enc_paths(&v)),
        "_api_server_start" => e::_api_server_start(&repo(), &c.j(1, "ev"), &c.str(2, "year_hint")).map(|(a, b)| jl![opt_s(a), opt_s(b)]),
        "attestation_same_boot" => {
            e::attestation_same_boot(&repo(), &c.j(1, "ev"), c.opt_str(2, "written").as_deref(), &c.j(3, "win")).map(J::from)
        }
        "lite_window" => e::lite_window(&repo(), &c.j(1, "ev")).map(J::from),
        "lite_tool_args" => e::lite_tool_args(&repo(), &c.j(1, "ev")).map(J::from),
        "_measure_window" => e::_measure_window(&repo(), &c.j(1, "ev")).map(|(a, b, d)| jl![opt_s(a), opt_s(b), d]),
        "_attestation_timing" => e::_attestation_timing(&repo(), &c.j(1, "ev"), c.opt_str(2, "written").as_deref())
            .map(|(a, b, d)| jl![opt_s(a), opt_s(b), J::from(d)]),
        "_assemble" => e::_assemble(&repo(), &c.j(1, "ev"), c.kw("measured_key").and_then(J::as_str), &e3_seq(c.kw("simlog_dirs")), &J::Null).map(|v| e3_enc_ev(&v)),
        "_release_of" => e::_release_of(&c.j(0, "ref")).map(opt_s),
        "_tp_signal" | "_auto_node" => {
            let (t, cell) = (c.str(1, "topology"), c.str(2, "cell"));
            let mk = c.kw("measured_key").and_then(J::as_str).map(str::to_string);
            let rr = c.kw("report_rel").and_then(J::as_str).map(str::to_string);
            let sd = e3_seq(c.kw("simlog_dirs"));
            let m = c.kw("measurement").cloned().unwrap_or(J::Null);
            let cert = c.kw("cert").cloned().unwrap_or(J::Null);
            if c.func == "_tp_signal" {
                e::_tp_signal(&repo(), &t, &cell, &cert, mk.as_deref(), rr.as_deref(), &sd, &m).map(|(a, b)| jl![J::from(a), b])
            } else {
                e::_auto_node(&repo(), &t, &cell, &cert, mk.as_deref(), rr.as_deref(), &sd, &m).map(|(a, b)| jl![opt_s(a), b])
            }
        }
        "_resolve_node" => {
            let assigned = c.kw("assigned").and_then(J::as_str).map(str::to_string);
            let declared: Vec<String> = c.kw("declared").map(|v| crate::api::unwrap_seq(v).iter().map(J::display).collect()).unwrap_or_default();
            e::_resolve_node(&c.j(0, "ev_node"), &c.kw("measured").cloned().unwrap_or(J::Null), &c.kw("topology").map(J::display).unwrap_or_default(),
                assigned.as_deref(), &declared, &c.kw("cert_node_source").map(J::display).unwrap_or_default(), &c.kw("auto").cloned().unwrap_or(J::Null))
                .map(|(a, b)| jl![a, b])
        }
        "_pointer_ok" => e::_pointer_ok(&c.j(0, "p"), &c.str(1, "node"), &c.strs(2, "declared"), &c.j(3, "val")).map(J::Bool),
        "_certificate_from_pointers" => e::_certificate_from_pointers(&repo(), &c.j(1, "ptrs")).map(|(a, b, d)| jl![opt_s(a), J::from(b), strs(d)]),
        "from_campaign" => e::from_campaign(&repo(), &c.j(1, "campaign_id"), &c.j(2, "cell"), &c.j(3, "node"), &J::Null).map(|v| e3_enc_ev(&v)),
        "_json_line" => e::_json_line(&c.str(0, "text")),
        "from_publication" => e::from_publication(&repo(), &c.j(1, "topic"), &c.j(2, "node"), &J::Null).map(|v| e3_enc_ev(&v)),
        "_cell_of_publication" => e::_cell_of_publication(&repo(), &c.str(1, "topology"), c.opt_str(2, "measured_key").as_deref(), &e3_seq(c.arg(3, "simlog_dirs")))
            .map(|(a, b)| jl![a, b]),
        // ── selftest ──
        "selftest" => Ok(J::from(e::selftest())),
        // ── e4 ──
        "_serve_proof_ok" => e::_serve_proof_ok(&c.j(0, "sp"), &c.str(1, "cell"), c.opt_str(2, "image_tag").as_deref()).map(|(a, b)| jl![a, b]),
        "serve_proof_scope" => e::serve_proof_scope(&repo(), &c.j(1, "ev")).map(J::from),
        "qualification" => e::qualification(&c.j(0, "ev")),
        "_measured_identity" => e::_measured_identity(&repo(), &c.j(1, "ev")).map(|(a, b, d)| jl![a, b, d]),
        "_identity_from_meta" => e::_identity_from_meta(&c.j(0, "meta")),
        "identity" => e::identity(&c.j(0, "ev")),
        "_measured_quant_none" => e::_measured_quant_none(&repo(), &c.j(1, "ev")).map(opt_s),
        "topology_label" => e::topology_label(&c.j(0, "identity")).map(J::Str),
        "_serving_yaml" => e::_serving_yaml(&repo(), &c.j(1, "ev")).map(|(a, b)| jl![J::from(a), b]),
        "_checkpoint" => e::_checkpoint(&repo(), &c.j(1, "ev")),
        "_hf_revision" => e::_hf_revision(&c.path(0, "host"), &c.str(1, "label"), c.opt_str(2, "measured").as_deref()).map(|(a, b)| jl![opt_s(a), b]),
        "_revision_at" => e::_revision_at(&c.path(0, "g"), &c.str(1, "sha"), &c.str(2, "desc"), c.opt_str(3, "measured").as_deref())
            .map(|(a, b)| jl![opt_s(a), b]),
        "_hf_repo" => e::_hf_repo(&c.path(0, "host"), &c.str(1, "label")).map(|(a, b)| jl![opt_s(a), b]),
        "_base_model" => e::_base_model(&c.path(0, "host"), &c.str(1, "label")).map(|(a, b)| jl![opt_s(a), b]),
        "_quant_raw" => e::_quant_raw(&c.j(0, "ck")),
        "_quant_composition" => e::_quant_composition(&c.j(0, "ck")).map(|(a, b)| jl![J::from(a.map(J::List)), b]),
        "_ple_present" => e::_ple_present(&c.j(0, "ck")).map(J::from),
        "_yaml_get" => {
            let keys: Vec<String> = c.args.iter().skip(1).filter_map(|v| v.as_str().map(str::to_string)).collect();
            let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            e::_yaml_get(&c.j(0, "y"), &refs).map(|(a, b)| jl![opt_s(a), b])
        }
        "_same" => e::_same(&c.j(0, "a"), &c.j(1, "b")).map(J::Bool),
        "_measured_knobs" => e::_measured_knobs(&repo(), &c.j(1, "ev")),
        "_drift_check" => e::_drift_check(&repo(), &c.j(1, "y"), &c.str(2, "yrel"), &c.j(3, "env"), &c.j(4, "ev")).map(|_| J::Null),
        "naming_facts" => e::naming_facts(&repo(), &c.j(1, "ev")),
        "_flat_str" => e::_flat_str(&c.j(0, "v")).map(J::Str),
        "_flat_env" => e::_flat_env(&c.str(0, "text")),
        "_flat_sh" => e::_flat_sh(&c.str(0, "text")),
        "tail_sources" => e::tail_sources(&repo(), &c.j(1, "ev")),
        "_ledger_vllm" => e::_ledger_vllm(&c.j(0, "att")),
        "_prev_release" => e::_prev_release(&c.j(0, "ev")).map(opt_s),
        "_arch_facts" => e::_arch_facts(&repo(), &c.j(1, "ev")),
        "_target_gpu" => {
            let r = c.opt_path(1, "repo");
            e::_target_gpu(&c.j(0, "ev"), r.as_deref()).map(|(a, b)| jl![opt_s(a), opt_s(b)])
        }
        "_sealed_payload" => e::_sealed_payload(&repo(), &c.j(1, "ev")).map(|(a, b)| jl![J::from(a), opt_s(b)]),
        "sealed_publication" => e::sealed_publication(&repo(), &c.j(1, "ev")).map(J::from),
        "_spec_fact" => e::_spec_fact(&repo(), &c.j(1, "ev"), &c.j(2, "y"), &c.str(3, "yrel")),
        "_graph_fact" => e::_graph_fact(&repo(), &c.j(1, "ev"), &c.j(2, "y"), &c.str(3, "yrel")),
        "_ple_fact" => e::_ple_fact(&c.j(0, "ev"), &c.j(1, "env"), &c.str(2, "env_rel"), &c.j(3, "ck")),
        "approval_for" => e::approval_for(&repo(), &c.j(1, "campaign_id"), &c.j(2, "cell"), &c.j(3, "node")).map(J::from),
        "phase_set_publish" => e::phase_set_publish(&repo(), &c.j(1, "campaign_id"), &c.j(2, "cell"), &c.j(3, "node"), &c.j(4, "tag"),
            &c.j(5, "remote_sha"), &c.j(6, "utc"), &J::Null).map(|_| J::Null),
        "topic_for" => e::topic_for(&c.j(0, "ev")).map(J::Str),
        "_publisher" => {
            let args: Vec<String> = c.args.iter().skip(2).filter_map(|v| v.as_str().map(str::to_string)).collect();
            let accept: Vec<i64> = c.kw("accept").map(|v| crate::api::unwrap_seq(v).iter().filter_map(J::as_i64).collect()).unwrap_or_else(|| vec![0]);
            e::_publisher(&repo(), &c.str(1, "sub"), &args, &accept).map(|(a, b)| jl![a, b])
        }
        "_narratives" => e::_narratives(&c.j(0, "ev")),
        "_runtime_identity" => e::_runtime_identity(&repo(), &c.j(1, "ev")).map(|(a, b)| jl![a, b]),
        "_pii_scan" => e::_pii_scan(&repo(), &c.j(1, "rels"), &c.strs(2, "terms")).map(J::from),
        "_record" => e::_record(&repo(), &c.str(1, "topic")),
        "drive_publisher" => e::drive_publisher(&repo(), &c.j(1, "ev"), &c.j(2, "topic"), &c.j(3, "generated_utc"), &c.path(4, "draft_dir"))
            .map(|p| crate::api::enc_path(&p)),
        "_finalize" => e::_finalize(&repo(), &c.str(1, "topic"), &c.path(2, "inputs")).map(|p| crate::api::enc_path(&p)),
        "set_promotion_target" => {
            let d = c.opt_path(6, "draft_dir");
            e::set_promotion_target(&repo(), &c.j(1, "topic"), &c.j(2, "tag"), &c.j(3, "topology"), &c.j(4, "anchor"), &c.j(5, "generated_utc"), d.as_deref())
                .map(|p| crate::api::enc_path(&p))
        }
        "rebind_campaign_narratives" => {
            e::rebind_campaign_narratives(&repo(), &c.j(1, "ev"), &c.j(2, "topic"), &c.j(3, "generated_utc"), &c.path(4, "draft_dir"))
        }
        "output_manifest" => e::output_manifest(&repo(), &c.str(1, "topology")).map(J::from),
        "publisher_inputs" => e::publisher_inputs(&repo(), &c.j(1, "ev"), &c.path(2, "draft_dir")).map(|p| crate::api::enc_path(&p)),
        _ => return None,
    })
}
