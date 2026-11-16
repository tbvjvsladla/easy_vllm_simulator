//! hint_api bridge for `template` (arms grouped by part).
use crate::api::{decode_path, enc_path, Call, Res};
use crate::template as t;
use crate::value::J;

fn opt(s: Option<String>) -> J {
    J::from(s)
}
fn t3(v: (String, String, String)) -> J {
    J::List(vec![J::Str(v.0), J::Str(v.1), J::Str(v.2)])
}

/// Decode a `_Ctx` argument (`{"__class__": "_Ctx", "repo", "facts", "templates"}` — the encoding of `template._Ctx(...)`)
/// and run `f` with it. Only empty `templates` can be bridged (t1 part).
fn with_ctx(c: &Call, f: t::FactRenderer) -> Res {
    let a = c.j(0, "c");
    let repo = decode_path(&a["repo"]);
    let facts = a["facts"].clone();
    let ctx = t::_Ctx { repo: &repo, facts: &facts, templates: &[] };
    f(&ctx).map(J::Str)
}

pub fn call(c: &Call) -> Option<Res> {
    let name = c.func.as_str();
    // ── t1 ──
    if let Some(fid) = name.strip_prefix("_f_") {
        if let Some((_, f)) = t::FACT_RENDERERS().iter().find(|(k, _)| *k == fid) {
            return Some(with_ctx(c, *f));
        }
    }
    // ── selftest ──
    if name == "selftest" {
        return Some(Ok(J::from(t::selftest())));
    }
    Some(match name {
        "_Ctx" => Ok(crate::jd! {
            "__class__" => "_Ctx",
            "repo" => enc_path(&c.path(0, "repo")),
            "facts" => c.j(1, "facts"),
            "templates" => c.j(2, "templates"),
        }),
        "_measurement_table" => {
            let a = c.j(0, "c");
            let repo = decode_path(&a["repo"]);
            let facts = a["facts"].clone();
            t::_measurement_table(&t::_Ctx { repo: &repo, facts: &facts, templates: &[] }).map(J::from)
        }
        "_norm" => t::_norm(&c.str(0, "text")).map(J::Str),
        "normalize_source" => t::normalize_source(&c.str(0, "text")).map(J::Str),
        "read_source_raw" => {
            let e = c.kw("errors").and_then(J::as_str).unwrap_or("replace").to_string();
            t::read_source_raw(&c.path(0, "path"), &e).map(J::Str)
        }
        "_scalar" => t::_scalar(&c.j(0, "v")).map(J::Str),
        "_cell" => t::_cell(&c.j(0, "v"), &c.opt_str(1, "absent").unwrap_or_else(|| t::_MISSING_TEXT.into())).map(J::Str),
        "_code" => t::_code(&c.j(0, "v"), &c.opt_str(1, "absent").unwrap_or_else(|| t::_MISSING_TEXT.into())).map(J::Str),
        "_table" => {
            let h: Vec<String> = c.strs(0, "headers");
            let rows = J::List(crate::api::unwrap_seq(&c.j(1, "rows")).iter().map(|r| J::List(crate::api::unwrap_seq(r))).collect());
            t::_table(&h, &rows).map(J::from)
        }
        "_dict" => t::_dict(&c.j(0, "obj"), &c.str(1, "key")),
        "_axis" => t::_axis(&c.j(0, "v")),
        "_topology" => t::_topology(&c.j(0, "facts")).map(opt),
        "_task_class" => t::_task_class(&c.j(0, "facts")).map(opt),
        "is_lite_cell" => t::is_lite_cell(&c.j(0, "facts")).map(J::Bool),
        "cell_grade" => t::cell_grade(&c.j(0, "facts")).map(J::Str),
        "payload_docs" => t::payload_docs(&c.j(0, "facts")).map(J::from),
        "_cond_text" => t::_cond_text(&c.str(0, "line"), &c.j(1, "facts")).map(J::Str),
        "_value_text" => t::_value_text(&c.j(0, "v")).map(J::Str),
        "_camp_id" => t::_camp_id(&c.j(0, "camp")).map(J::Str),
        "_verdict_text" => t::_verdict_text(&c.j(0, "f")).map(J::Str),
        "_marker_lines" => t::_marker_lines(&c.j(0, "f")).map(J::from),
        "_tail_evidence_text" => t::_tail_evidence_text(&c.j(0, "ev")).map(J::Str),
        "_patch_records" => t::_patch_records(&c.j(0, "f")).map(J::List),
        "_required_patches_line" => t::_required_patches_line(&c.j(0, "f")).map(J::Str),
        "_vllm_observed_rows" => t::_vllm_observed_rows(&c.j(0, "nobs")).map(|v| J::List(v.into_iter().map(t3).collect())),
        "_approval_rows" => t::_approval_rows(&c.j(0, "appr"), &c.j(1, "prior"))
            .map(|v| J::List(v.into_iter().map(|(a, b)| J::List(vec![J::Str(a), J::Str(b)])).collect())),
        "_yes_no" => t::_yes_no(&c.j(0, "v")).map(J::Str),
        "_bench_definition_source" => t::_bench_definition_source(&c.j(0, "bd")).map(J::Str),
        "_bench_definition_verdict" => t::_bench_definition_verdict(&c.j(0, "bd")).map(J::Str),
        "_factcheck_line" => t::_factcheck_line(&c.j(0, "fc")).map(J::Str),
        "_missing_block" => t::_missing_block(&c.j(0, "missing"), &c.j(1, "meanings"), &c.j(2, "reasons")).map(J::Str),
        "_missing_codes" => t::_missing_codes(&c.j(0, "missing")).map(J::from),
        "_attestation_scope_line" => t::_attestation_scope_line(&c.j(0, "f")).map(opt),
        "_legend_md" => t::_legend_md().map(J::Str),
        "_slot_reason" => t::_slot_reason(&c.j(0, "s")).map(J::Str),
        "_revision_line" => t::_revision_line(&c.str(0, "slot"), &c.j(1, "r")).map(J::Str),
        "_excluded_lines" => t::_excluded_lines(&c.j(0, "slots")).map(J::from),
        "_file_marks_table" => t::_file_marks_table(&c.j(0, "slots")).map(J::from),
        "_missing_sentence" => t::_missing_sentence(&c.j(0, "f"), &c.str(1, "code"), &c.str(2, "absent_text")).map(J::Str),
        "_markers_text" => t::_markers_text(&c.j(0, "mk")).map(opt),
        "_patch_evidence_table" => t::_patch_evidence_table(&c.j(0, "patches")).map(J::from),
        "_candidates" => t::_candidates(&c.j(0, "f")).map(J::List),
        "_knob" => t::_knob(&c.j(0, "c")).map(J::Str),
        "_qualification_line" => t::_qualification_line(&c.j(0, "f")).map(J::Str),
        "_context_map_lines" => t::_context_map_lines(&c.j(0, "f")).map(J::from),
        "_derived_rule_text" => t::_derived_rule_text(&c.j(0, "d")).map(opt),
        "_derived_env_lines" => t::_derived_env_lines(&c.j(0, "f")).map(J::from),
        "_regenerated_env" => t::_regenerated_env(&c.j(0, "f")).map(J::from),
        "_env_value" => t::_env_value(&c.j(0, "v")).map(J::Str),
        "_row_span" => t::_row_span(&c.j(0, "rows"), &c.str(1, "field"), &c.str(2, "win")).map(J::Str),
        "_log_windows" => t::_log_windows(&c.j(0, "rows"), &c.opt_str(1, "field").unwrap_or_else(|| "log_window".into())),
        "_ray_folded" => t::_ray_folded(&c.j(0, "r")).map(J::Int),
        "_attempts" => t::_attempts(&c.j(0, "rows")).map(J::List),
        "_attempts_table" => t::_attempts_table(&c.j(0, "rows"), &c.j(1, "bnd")).map(J::from),
        "_ledger_span_lines" => t::_ledger_span_lines(&c.j(0, "f")).map(J::from),
        "_boundary_line" => t::_boundary_line(&c.j(0, "bnd")).map(J::from),
        // ── t2 ──
        _ => return t2_call(c),
    })
}

// ── t2 ─────────────────────────────────────────────────────────────────────────────────────────
/// Resolve `{"__call__": ...}` nested anywhere (the shim's `from_json` is recursive) and `{"__tuple__"}` → list.
fn t2_deep(v: &J) -> Result<J, crate::core::HintError> {
    match v {
        J::Dict(m) if m.len() == 1 && m.0[0].0 == "__call__" => {
            let (r, o, e) = crate::api::dispatch(&m.0[0].1);
            crate::io::out(&o);
            crate::io::err(&e);
            r
        }
        J::Dict(m) if m.len() == 1 && (m.0[0].0 == "__tuple__" || m.0[0].0 == "__set__") => {
            Ok(J::List(m.0[0].1.items().iter().map(t2_deep).collect::<Result<_, _>>()?))
        }
        J::Dict(m) if m.len() == 1 && m.0[0].0 == "__path__" => Ok(v.clone()),
        J::Dict(m) => {
            let mut out = crate::value::Map::new();
            for (k, x) in m.iter() {
                out.insert(k, t2_deep(x)?);
            }
            Ok(J::Dict(out))
        }
        J::List(l) => Ok(J::List(l.iter().map(t2_deep).collect::<Result<_, _>>()?)),
        other => Ok(other.clone()),
    }
}

fn t2_pair(v: (Option<String>, Option<String>)) -> J {
    J::List(vec![J::from(v.0), J::from(v.1)])
}

fn t2_call(c: &Call) -> Option<Res> {
    let dj = |i: usize, n: &str| t2_deep(&c.j(i, n));
    let p = |i: usize, n: &str| c.path(i, n);
    let path_of = |v: &J| -> Option<std::path::PathBuf> { if v.is_null() { None } else { Some(decode_path(v)) } };
    Some((|| -> Res {
        Ok(match c.func.as_str() {
            "_Doc.parse" => t::_Doc::parse(&c.str(0, "name"), &c.str(1, "text")).to_j(),
            "_prompt_fields" => t::_prompt_fields(&c.strs(0, "body"))?,
            "_question" => J::Str(t::_question(&dj(0, "fields")?)?),
            "_directives" => t::_directives(&dj(0, "fields")?)?,
            "_condition_holds" => {
                let (b, s) = t::_condition_holds(&dj(0, "cond")?, &dj(1, "facts")?)?;
                J::List(vec![J::Bool(b), J::Str(s)])
            }
            "_na_line" => J::Str(t::_na_line(&dj(0, "cond")?, &c.str(1, "actual"))?),
            "_templates_dir" => enc_path(&t::_templates_dir(&p(0, "repo"))?),
            "_load_templates" => {
                let mut m = crate::value::Map::new();
                for (k, d) in t::_load_templates(&p(0, "repo"))? {
                    m.insert(k, d.to_j());
                }
                J::Dict(m)
            }
            "_strip_comment" => J::Str(t::_strip_comment(&c.str(0, "s"))?),
            "_unquote" => t2_pair(t::_unquote(&c.str(0, "item"))?),
            "_parse_value" => {
                let (v, e) = t::_parse_value(&c.str(0, "raw"))?;
                J::List(vec![v, J::from(e)])
            }
            "_parse_event_body" => {
                let (ev, errs) = t::_parse_event_body(&c.strs(0, "body"))?;
                let e = errs.into_iter().map(|(a, b, m)| J::List(vec![J::Str(a), J::Int(b), J::Str(m)])).collect();
                J::List(vec![ev, J::List(e)])
            }
            "_validate_event" => J::List(
                t::_validate_event(&dj(0, "ev")?)?.into_iter().map(|(a, b)| J::List(vec![J::Str(a), J::Str(b)])).collect(),
            ),
            "_sources_of" => J::from(t::_sources_of(&dj(0, "ev")?)?),
            "_source_token" => J::Str(t::_source_token(&c.str(0, "entry"))?),
            "_is_requirement_artifact" => J::Bool(t::_is_requirement_artifact(&dj(0, "loc")?)?),
            "parse_hint_events" => J::List(t::parse_hint_events(&c.str(0, "md"))?),
            "excerpts" => J::List(t::excerpts(&c.str(0, "md"))?),
            "fact_blocks" => t::fact_blocks(&dj(0, "md")?.display())?,
            "_valid_events" => J::List(t::_valid_events(&c.str(0, "md"))?),
            "_derive_from_texts" => t::_derive_from_texts(&c.str(0, "narrative"), &c.str(1, "artifacts_md"))?,
            "derive_blocks" => t::derive_blocks(&p(0, "payload_dir"))?,
            "_replace_fact_bodies" => J::Str(t::_replace_fact_bodies(&c.str(0, "text"), &dj(1, "bodies")?)?),
            "refresh" => J::from(t::refresh(&p(0, "payload_dir"))?),
            "_normalize_facts" => t::_normalize_facts(&dj(0, "facts")?)?,
            "name_guide_md" => J::Str(t::name_guide_md(&dj(0, "f")?)?),
            "_context_map_summary" => J::Str(t::_context_map_summary(&dj(0, "f")?)?),
            "default_snapshot_path" => enc_path(&t::default_snapshot_path(&p(0, "payload_dir"))?),
            "load_snapshot" => t::load_snapshot(&p(0, "path"))?,
            "render_scaffold" => {
                let snap = c.kw("snapshot_path").and_then(path_of);
                J::List(t::render_scaffold(&p(0, "repo"), &dj(1, "facts")?, &p(2, "payload_dir"), snap.as_deref())?)
            }
            "prompts" => J::List(t::prompts(&p(0, "payload_dir"))?),
            "seal_prompts" => {
                let docs: Vec<String> = match c.kw("docs") {
                    Some(v) => crate::api::unwrap_seq(v).iter().map(J::display).collect(),
                    None => t::PAYLOAD_DOCS.iter().map(|s| s.to_string()).collect(),
                };
                t::seal_prompts(&p(0, "payload_dir"), &docs)?;
                J::Null
            }
            "subsections" => J::List(t::subsections(&p(0, "payload_dir"), &c.str(1, "doc"), &c.str(2, "chapter"))?),
            "brief" => J::Str(t::brief(&p(0, "payload_dir"))?),
            "substituted_source" => {
                let draft = c.kw("draft_dir").and_then(path_of);
                J::Str(t::substituted_source_with(
                    &p(0, "repo"),
                    &p(1, "payload_dir"),
                    &c.str(2, "token"),
                    &t2_deep(c.kw("lineage").unwrap_or(&J::Null))?,
                    &t2_deep(c.kw("subst_table").unwrap_or(&J::Null))?,
                    draft.as_deref(),
                    &t::TemplateHooks::default(),
                )?)
            }
            "_heading_key" => J::Str(t::_heading_key(&c.str(0, "text"))?),
            "_section_in_headings" => J::Bool(t::_section_in_headings(&c.str(0, "label"), &c.str(1, "text"), &J::Null)?),
            "lint" => {
                let kw = |n: &str| -> Result<J, crate::core::HintError> { t2_deep(c.kw(n).unwrap_or(&J::Null)) };
                let bench = c.kw("bench_report").and_then(path_of);
                let draft = c.kw("draft_dir").and_then(path_of);
                let terms_j = c.kw("pii_terms").cloned();
                let terms_v: Option<Vec<String>> = terms_j.as_ref().and_then(|v| v.as_list().map(|l| l.iter().map(J::display).collect()));
                let terms: Option<Option<&[String]>> = match &terms_j {
                    None => None,
                    Some(J::Null) => Some(None),
                    Some(_) => Some(terms_v.as_deref()),
                };
                J::List(t::lint_with(
                    &p(0, "repo"),
                    &p(1, "payload_dir"),
                    &kw("lineage")?,
                    &kw("subst_table")?,
                    &kw("manifest")?,
                    bench.as_deref(),
                    &kw("facts")?,
                    c.kw("sealed").and_then(J::as_bool).unwrap_or(false),
                    c.kw("historical_verify").and_then(J::as_bool).unwrap_or(false),
                    terms,
                    draft.as_deref(),
                    &t::TemplateHooks::default(),
                )?)
            }
            "_mark_problems" => J::from(t::_mark_problems(&p(0, "payload_dir"), &dj(1, "rec")?)?),
            "_comment_line" => J::Str(t::_comment_line(&c.str(0, "name"), &c.str(1, "text"))?),
            "agent_request_header_line" => J::Str(t::agent_request_header_line(&dj(0, "req")?)?),
            "agent_request_scaffold" => J::Str(t::agent_request_scaffold(&dj(0, "req")?)?),
            "write_agent_requests" => J::from(t::write_agent_requests(&p(0, "payload_dir"), &dj(1, "reqs")?)?),
            "agent_request_problems" => J::List(
                t::agent_request_problems(&p(0, "payload_dir"), &dj(1, "req")?)?
                    .into_iter()
                    .map(|(a, b)| J::List(vec![J::Str(a), J::Str(b)]))
                    .collect(),
            ),
            "cited_sources" => {
                let draft = c.kw("draft_dir").and_then(path_of);
                t::cited_sources(&p(0, "repo"), &p(1, "payload_dir"), &t2_deep(c.kw("lineage").unwrap_or(&J::Null))?, draft.as_deref())?
            }
            "_num_key" => J::Str(t::_num_key(&c.str(0, "tok"))?),
            "_brief_numbers" => J::from(t::_brief_numbers(&c.str(0, "text"))?),
            "render_bench_source" => J::Str(t::render_bench_source(&p(0, "repo"), &dj(1, "source")?)?),
            "_render_bench_source" => J::Str(t::_render_bench_source(&p(0, "repo"), &dj(1, "source")?)?),
            "slot_intro" => {
                let (i, s) = t::slot_intro(&c.str(0, "text"))?;
                J::List(vec![J::Int(i), J::Str(s)])
            }
            "update_facts" => {
                let snap = c.kw("snapshot_path").and_then(path_of);
                J::from(t::update_facts(&p(0, "repo"), &p(1, "payload_dir"), &dj(2, "changes")?, snap.as_deref())?)
            }
            _ => return Err(crate::core::HintError::py("AttributeError", "__t2_unbridged__")),
        })
    })())
    .filter(|r| !matches!(r, Err(e) if e.message == "__t2_unbridged__"))
}
