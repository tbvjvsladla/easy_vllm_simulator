//! hint_api bridge for `artifacts` (arms grouped by part).
use crate::api::{enc_path, Call, Res};
use crate::artifacts as a;
use crate::value::J;

fn opt(s: Option<String>) -> J {
    J::from(s)
}
fn bytes_arg(c: &Call, i: usize, name: &str) -> Vec<u8> {
    match c.arg(i, name) {
        Some(J::Str(s)) => s.as_bytes().to_vec(),
        Some(J::Dict(m)) => {
            let h = m.get("__bytes__").and_then(J::as_str).unwrap_or_default();
            (0..h.len() / 2).filter_map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).ok()).collect()
        }
        _ => Vec::new(),
    }
}
fn opt_bool(c: &Call, i: usize, name: &str) -> Option<bool> {
    c.arg(i, name).and_then(J::as_bool)
}

/// a1: the `_Ctx` argument 0 — the bridge form `{"__class__": "_Ctx", "__ctx__": {repo, ev, runner}}` made by the `_ctx` arm
/// (re-built here: the Python shim also builds a fresh `_Ctx` per request).
fn a1_ctx(c: &Call) -> Result<a::_Ctx, crate::core::HintError> {
    let v = c.arg(0, "c").cloned().unwrap_or(J::Null);
    let x = &v["__ctx__"];
    a::_ctx(&crate::api::decode_path(&x["repo"]), &x["ev"], &x["runner"], &J::Null, &J::Null)
}

/// a2: a `Path` (`{"__path__"}`) or a `_GitPath` (`{"__repr__": repr(obj)}` — the shim's fallback for a plain object).
fn a2_patch_path(p: &a::PatchPath) -> J {
    match p {
        a::PatchPath::Fs(f) => enc_path(f),
        a::PatchPath::Git(g) => crate::jd! {"__repr__" => format!("<hintlib.artifacts._GitPath object>{g}")},
    }
}

pub fn call(c: &Call) -> Option<Res> {
    Some(match c.func.as_str() {
        // ── a1 ──
        "_ctx" => {
            let runner = c.kw("runner").cloned().unwrap_or(J::Null);
            a::_ctx(&c.path(0, "repo"), &c.j(1, "ev"), &runner, &J::Null, &J::Null)
                .map(|_| crate::jd! {"__class__" => "_Ctx", "__ctx__" => crate::jd!{"repo" => c.j(0, "repo"), "ev" => c.j(1, "ev"), "runner" => runner}})
        }
        "_Ctx.docker" => a1_ctx(c).map(|cx| {
            let cache = c.kw("cache").and_then(J::as_bool).unwrap_or(true);
            let args: Vec<String> = c.args[1..].iter().map(J::display).collect();
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let (cls, p) = cx.docker(&refs, cache);
            let mut argv = vec!["docker".to_string()];
            argv.extend(args.iter().cloned());
            J::List(vec![J::Str(cls), p.map(|p| crate::jd! {"__repr__" => a::_proc_repr(&argv, &p)}).unwrap_or(J::Null)])
        }),
        "_get" => a::_get(&c.j(0, "obj"), &c.str(1, "name"), &c.j(2, "default")),
        "_read" => a::_read(&c.path(0, "path")).map(J::Str),
        "_json_file" => a::_json_file(&c.path(0, "path")).map(J::from),
        "_topology" => a::_topology(&c.j(0, "ev")).map(J::Str),
        "_cell" => a::_cell(&c.j(0, "ev")).map(J::Str),
        "_utc_z" => a::_utc_z(c.arg(0, "raw").and_then(J::as_str)).map(opt),
        "_secs_between" => a::_secs_between(c.arg(0, "a").and_then(J::as_str), c.arg(1, "b").and_then(J::as_str)).map(J::from),
        "_fmt_duration" => a::_fmt_duration(c.arg(0, "secs").and_then(J::as_i64), &c.str(1, "bound")).map(J::Str),
        "_forward_module" => a::_forward_module(&c.path(0, "repo"), None).map(|m| J::Str(crate::core::posix(&m.path))),
        "_render_module" => a::_render_module(&c.path(0, "repo"), None).map(|m| J::Str(m.SHARED_ASSET_DIR)),
        "_flag_value" => a::_flag_value(&c.strs(0, "args"), &c.str(1, "flag")).map(opt),
        "_image_ref" => a1_ctx(c).and_then(|x| a::_image_ref(&x)).map(|(r, s)| J::List(vec![opt(r), J::Str(s)])),
        "_image_created" => a1_ctx(c).and_then(|x| a::_image_created(&x)).map(opt),
        "_recipe_vs_image" => a1_ctx(c)
            .and_then(|x| a::_recipe_vs_image(&x, &c.strs(1, "rels"), c.arg(2, "created").and_then(J::as_str), &c.j(3, "selected")))
            .map(J::List),
        "_ws" => a::_ws(&c.str(0, "text")).map(J::Str),
        "_dockerfile_steps" => a::_dockerfile_steps(&c.str(0, "text")).map(J::from),
        "_history_step" => a::_history_step(&c.str(0, "created_by")).map(opt),
        "_history_layers" => a1_ctx(c).and_then(|x| a::_history_layers(&x)).map(|(r, s)| J::List(vec![r.map(J::List).unwrap_or(J::Null), J::Str(s)])),
        "_history_match" => a::_history_match(&c.strs(0, "steps"), &c.strs(1, "window")),
        "_git_rev_before" => a1_ctx(c).and_then(|x| a::_git_rev_before(&x, &c.str(1, "rel"), c.arg(2, "created").and_then(J::as_str))).map(opt),
        "_history_blobs" => a1_ctx(c).and_then(|x| a::_history_blobs(&x, &c.str(1, "name"))).map(J::List),
        "_prefer_same_path" => a::_prefer_same_path(&c.j(0, "cands"), &c.str(1, "rel")).map(J::List),
        "_better" => a::_better(&c.j(0, "a"), &c.j(1, "b")).map(J::Bool),
        "_select_dockerfile" => a1_ctx(c).and_then(|x| a::_select_dockerfile(&x, &c.str(1, "dockerfile"))),
        "_copy_dests" => a::_copy_dests(&c.str(0, "text")),
        "_dest_path" => a::_dest_path(&c.str(0, "src"), &c.str(1, "dst"), c.bool_or(2, "src_is_dir", false)).map(J::Str),
        "_select_copied" => a1_ctx(c).and_then(|x| {
            a::_select_copied(&x, &c.str(1, "src_rel"), &c.path(2, "path"), &c.j(3, "baked"), &c.str(4, "probe_note"), c.kw("ledger_sha256").and_then(J::as_str))
        }),
        "_measured_utc" => a1_ctx(c).and_then(|x| a::_measured_utc(&x)).map(|(r, s)| J::List(vec![opt(r), J::Str(s)])),
        "_lite_measured_utc" => a::_lite_measured_utc(&c.path(0, "repo"), &c.j(1, "ev")).map(|(r, s)| J::List(vec![opt(r), J::Str(s)])),
        "_regenerated" => a1_ctx(c).and_then(|x| a::_regenerated(&x, &c.path(1, "path"))).map(J::from),
        "_rerender_at_measurement" => a1_ctx(c).and_then(|x| a::_rerender_at_measurement(&x, &c.str(1, "kind"))),
        "_rerender_reason" => a::_rerender_reason(&c.j(0, "rr")).map(J::Str),
        "_select_tracked" => a1_ctx(c).and_then(|x| a::_select_tracked(&x, &c.path(1, "path"))),
        "_repo_rel" => a1_ctx(c).and_then(|x| a::_repo_rel(&x, &c.path(1, "path"))).map(opt),
        "_tracked_row" => a::_tracked_row(&c.j(0, "sel")),
        "_compose_text" => a1_ctx(c).and_then(|x| a::_compose_text(&x)).map(J::Str),
        "_compose_file" => a1_ctx(c).and_then(|x| a::_compose_file(&x)).map(|p| p.map(|p| enc_path(&p)).unwrap_or(J::Null)),
        "_renderer_revision" => a1_ctx(c).and_then(|x| a::_renderer_revision(&x)).map(J::from),
        "_renderer_drift_lines" => a::_renderer_drift_lines(&c.j(0, "rsel")).map(J::from),
        "_plane" => a1_ctx(c).and_then(|x| a::_plane(&x)).map(|(p, s)| J::List(vec![J::Str(p), J::Str(s)])),
        "plane_of" => a::plane_of(&c.path(0, "repo"), &c.j(1, "ev"), &J::Null).map(J::Str),
        "plane_and_source" => a::plane_and_source(&c.path(0, "repo"), &c.j(1, "ev"), &J::Null).map(|(p, s)| J::List(vec![J::Str(p), J::Str(s)])),
        "_dockerfile_args" => a1_ctx(c).and_then(|x| a::_dockerfile_args(&x, &c.str(1, "text"))).map(J::from),
        "_dockerfile_copies" => a::_dockerfile_copies(&c.str(0, "text")).map(J::from),
        "_image_tag" => a1_ctx(c).and_then(|x| a::_image_tag(&x)).map(|(r, s)| J::List(vec![opt(r), J::Str(s)])),
        "_build_selection" => a1_ctx(c).and_then(|x| a::_build_selection(&x, &c.j(1, "ledger"))),
        "_ledgers_in_attestation" => a::_ledgers_in_attestation(&c.j(0, "att")).map(|(m, n)| J::List(vec![m, J::Int(n)])),
        "_attestation_ref" => a1_ctx(c).and_then(|x| a::_attestation_ref(&x)).map(J::Str),
        "_ledger_vllm_view" => a1_ctx(c).and_then(|x| a::_ledger_vllm_view(&x)).map(J::from),
        "_observe_ledger" => a1_ctx(c).and_then(|x| a::_observe_ledger(&x)).map(|(l, s, p, pr)| J::List(vec![J::from(l), opt(s), p, J::List(pr)])),
        // ── a2 ──
        "_patch_base" => a1_ctx(c).and_then(|x| a::_patch_base(&x)).map(|p| a2_patch_path(&p)),
        "_sha256_of" => a::_sha256_of(&a::PatchPath::Fs(c.path(0, "p"))).map(J::Str),
        "_list_patch_dir" => a::_list_patch_dir(&a::PatchPath::Fs(c.path(0, "d")))
            .map(|(p, k, o)| J::List(vec![J::List(p.iter().map(a2_patch_path).collect()), J::from(k), J::from(o)])),
        "_patch_trigger" => a::_patch_trigger(&c.str(0, "text")).map(opt),
        "_self_gate" => a::_self_gate(&c.str(0, "text")).map(J::from),
        "_history_build_args" => a1_ctx(c).and_then(|x| a::_history_build_args(&x, &c.str(1, "dockerfile_text"))).map(|(a_, s)| J::List(vec![a_, J::Str(s)])),
        "_sh_literal" => a::_sh_literal(&c.str(0, "raw"), &c.j(1, "known")).map(opt),
        "_script_shape" => a::_script_shape(&c.str(0, "text")),
        "_fail_loud_problems" => a::_fail_loud_problems(&c.j(0, "shape"), &c.j(1, "exclude")).map(J::from),
        "_patch_loop_failloud" => a1_ctx(c)
            .and_then(|x| a::_patch_loop_failloud(&x, &c.str(1, "df_text"), &c.str(2, "phase")))
            .map(|(b, s)| J::List(vec![J::from(b), J::Str(s)])),
        "_probe_allowed" => a1_ctx(c).and_then(|x| a::_probe_allowed(&x)).map(|(b, s)| J::List(vec![J::Bool(b), J::Str(s)])),
        "_python_roots" => a1_ctx(c).and_then(|x| a::_python_roots(&x, &c.str(1, "image"))).map(J::from),
        "_cp_from_image" => a1_ctx(c)
            .and_then(|x| a::_cp_from_image(&x, &c.str(1, "image"), &c.str(2, "path")))
            .map(|(b, s)| J::List(vec![J::from(b), J::Str(s)])),
        "_local_image_owner" => a::_local_image_owner(&c.path(0, "repo"), &c.j(1, "ev")).map(J::Bool),
        "bind_image_git_release" => {
            let mut ev = c.j(1, "ev");
            a::bind_image_git_release(&c.path(0, "repo"), &mut ev, &c.kw("runner").cloned().unwrap_or(J::Null)).map(|_| J::Null)
        }
        "_image_probe" => a1_ctx(c).and_then(|x| a::_image_probe(&x, &c.str(1, "df_text"))).map(|p| p.to_memo_j()),
        "_marker_check" => a::_marker_check(&c.j(0, "shape"), &c.j(1, "probe")),
        "_reconstruct" => a::_reconstruct(&c.str(0, "name"), &c.str(1, "text"), &c.str(2, "phase"), &c.j(3, "args"), &c.j(4, "env")),
        "_ledger_rows" => a::_ledger_rows(&c.j(0, "ledger")).map(J::List),
        "_ledger_script_identity" => a1_ctx(c).and_then(|x| a::_ledger_script_identity(&x, &c.j(1, "row"))),
        "_post_label" => a::_post_label(&c.j(0, "row")),
        "_mtime_vs_created" => a1_ctx(c).and_then(|x| a::_mtime_vs_created(&x, c.opt_path(1, "p").as_deref())).map(J::Str),
        "_not_in_ledger_reason" => a1_ctx(c)
            .and_then(|x| a::_not_in_ledger_reason(&x, &c.j(1, "ledger"), &c.str(2, "ph"), &c.path(3, "p")))
            .map(J::Str),
        "_applied" => a1_ctx(c)
            .and_then(|x| a::_applied(&x, c.kw("probe").and_then(J::as_bool).unwrap_or(true)))
            .map(|(ap, sel, l, pn)| J::List(vec![ap, sel, J::from(l), pn])),
        "_node_asymmetry" => a1_ctx(c)
            .and_then(|x| a::_node_asymmetry(&x, c.arg(1, "ledger_source").and_then(J::as_str), &c.j(2, "per_node")))
            .map(J::from),
        "node_asymmetry_phase_text" => a::node_asymmetry_phase_text(&c.j(0, "asym"), &c.str(1, "ph")).map(opt),
        "applied_set" => a::applied_set(&c.path(0, "repo"), &c.j(1, "ev"), &c.kw("runner").cloned().unwrap_or(J::Null), &J::Null),
        "_shape_key_axis" => a::_shape_key_axis(&c.str(0, "key"), &c.str(1, "value")).map(J::Str),
        "_manifest_ph" => a::_manifest_ph(&c.str(0, "field_name")).map(J::Str),
        "_docker_triplet_env_shape" => a::_docker_triplet_env_shape(&c.str(0, "text")).map(|(t, b)| J::List(vec![J::Str(t), J::Bool(b)])),
        "_field_of" => a::_field_of(&c.j(0, "detail")).map(J::Str),
        "env_shape" => a::env_shape(&c.path(0, "repo"), &c.path(1, "path"), &c.str(2, "kind"), &J::Null, &J::Null, c.kw("measured_utc").and_then(J::as_str))
            .map(J::Str),
        "_compose_services" => a::_compose_services(&c.str(0, "text")),
        "_serving_blocks" => a::_serving_blocks(&c.str(0, "text"))
            .map(|v| J::List(v.into_iter().map(|(n, b)| J::List(vec![J::Str(n), J::Str(b)])).collect())),
        "_command_texts" => a::_command_texts(&c.str(0, "block")).map(J::from),
        "_runner_names" => a1_ctx(c).and_then(|x| a::_runner_names(&x)).map(|(n, m)| J::List(vec![J::from(n), J::from(m)])),
        "_arm_key_chain" => a1_ctx(c).and_then(|x| a::_arm_key_chain(&x)),
        "_arm_stem" => a::_arm_stem(&c.j(0, "chain"), &c.j(1, "values")).map(|(s, k)| J::List(vec![J::Str(s), J::Str(k)])),
        "_arm_roles" => a1_ctx(c).and_then(|x| a::_arm_roles(&x, &c.j(1, "forwards"))),
        "_arm_log_observations" => a1_ctx(c).and_then(|x| a::_arm_log_observations(&x)),
        "arm_log_text" => a::arm_log_text(&c.j(0, "obs")).map(J::Str),
        "_runtime_patch_armed" => a1_ctx(c).and_then(|x| a::_runtime_patch_armed(&x)).map(|(b, s)| J::List(vec![J::Bool(b), J::Str(s)])),
        "_runner_gpu_literal" => a::_runner_gpu_literal(&c.str(0, "text")),
        // ── a3 ──
        "_strip_comment" => a::_strip_comment(&c.str(0, "value")).map(|(v, cm)| J::List(vec![J::Str(v), opt(cm)])),
        "_serving_yaml_knobs" => a::_serving_yaml_knobs(&c.path(0, "path"))
            .map(|v| J::List(v.into_iter().map(|(k, x, cm)| J::List(vec![J::Str(k), J::Str(x), opt(cm)])).collect())),
        "value_status_candidates" => a::value_status_candidates(&c.path(0, "repo"), &c.j(1, "ev"), &J::Null).map(J::List),
        "current_serve_window" => a::current_serve_window(
            &c.path(0, "repo"),
            &c.j(1, "ev"),
            c.kw("measure_start").and_then(J::as_str),
            c.kw("measure_end").and_then(J::as_str),
        )
        .map(J::from),
        "_first_measure_utc" => a::_first_measure_utc(&c.path(0, "repo"), &c.j(1, "ev")).map(|(x, y)| J::List(vec![opt(x), opt(y)])),
        "_phase_span" => a::_phase_span(&c.path(0, "repo"), &c.j(1, "ev"), &c.str(2, "node"), &c.str(3, "phase"), &c.str(4, "cell"))
            .map(|(x, y, z)| J::List(vec![opt(x), opt(y), opt(z)])),
        "pre_sweep_note" => a::pre_sweep_note(&c.j(0, "pre")).map(J::Str),
        "reproduce_steps" => a::reproduce_steps(&c.path(0, "repo"), &c.j(1, "ev"), &J::Null, &J::Null).map(J::List),
        "sub_recipe" => a::sub_recipe(&c.path(0, "repo"), &c.j(1, "ev"), &J::Null, &J::Null),
        "_copy" => a::_copy(&c.path(0, "payload"), &c.path(1, "source"), &c.str(2, "slot"), c.arg(3, "name").and_then(J::as_str)).map(J::Str),
        "_write" => a::_write(&c.path(0, "payload"), &c.str(1, "slot"), &c.str(2, "name"), &c.str(3, "text")).map(J::Str),
        "_write_bytes" => a::_write_bytes(
            &c.path(0, "payload"),
            &c.str(1, "slot"),
            &c.str(2, "name"),
            &bytes_arg(c, 3, "data"),
            c.arg(4, "mode").and_then(J::as_i64),
        )
        .map(J::Str),
        "_require_registered" => a::_require_registered(&c.path(0, "repo"), &c.strs(1, "missing")).map(|_| J::Null),
        "_exempt_record" => a::_exempt_record(&c.str(0, "rec")).map(J::Str),
        "slot_confidence" => a::slot_confidence(&c.j(0, "row")).map(J::Str),
        "_slot" => a::_slot(&c.strs(0, "files"), &c.j(1, "evidence"), &c.j(2, "excluded")),
        "generated_by_ok" => a::generated_by_ok(&c.j(0, "value")).map(J::Bool),
        "unverified_header" => a::unverified_header(&c.str(0, "reason"), &c.str(1, "generated_by")).map(J::Str),
        "_insert_header" => a::_insert_header(&c.path(0, "payload"), &c.str(1, "rel"), &c.str(2, "reason"), &c.str(3, "generated_by")),
        "_agent_request" => a::_agent_request(&c.str(0, "slot"), &c.str(1, "name"), &c.str(2, "why"), &c.strs(3, "inputs")),
        "_log_signatures" => a::_log_signatures(&c.str(0, "text")).map(J::from),
        "_patch_declared" => a::_patch_declared(&c.str(0, "text"), c.arg(1, "trigger").and_then(J::as_str)),
        "_fired_text" => a::_fired_text(&c.strs(0, "sigs"), &c.j(1, "counts"), opt_bool(c, 2, "fired")).map(J::Str),
        "_relevance" => a::_relevance(
            &c.j(0, "declared"),
            &c.j(1, "model"),
            &c.strs(2, "sigs"),
            opt_bool(c, 3, "fired"),
            c.int_or(4, "n_logs", 0),
            &c.j(5, "counts"),
        )
        .map(|(x, y)| J::List(vec![J::Str(x), J::Str(y)])),
        "_default_basis" => a::_default_basis(&c.str(0, "slot"), &c.j(1, "row"), &c.str(2, "name")).map(J::Str),
        "_compose_volumes" => a::_compose_volumes(&c.str(0, "text")),
        "_marked_leaf_matches_frozen" => a::_marked_leaf_matches_frozen(
            &bytes_arg(c, 0, "shipped"),
            &bytes_arg(c, 1, "frozen"),
            &c.str(2, "target"),
            &c.j(3, "mark"),
        )
        .map(J::Bool),
        "collect" => a::collect(&c.path(0, "repo"), &c.j(1, "ev"), &c.path(2, "payload_dir"), &J::Null, &J::Null, &J::Null),
        "selftest" => Ok(J::from(a::selftest())),
        _ => {
            let _ = enc_path;
            return None;
        }
    })
}
