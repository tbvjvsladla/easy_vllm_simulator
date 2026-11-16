//! hint_api bridge for `lineage`.
use crate::api::{unwrap_seq, Call, Res};
use crate::core::HintError;
use crate::lineage as l;
use crate::value::J;

fn opt_j(k: &Call, i: usize, name: &str) -> J {
    k.arg(i, name).cloned().unwrap_or(J::Null)
}

/// A `re.Pattern` argument (`{"__re__": text}` from a `slug_pattern` call).
fn pat_arg(k: &Call, i: usize, name: &str) -> Result<l::Pat, HintError> {
    let text = k.arg(i, name).and_then(|v| v.get_str("__re__").map(str::to_string)).unwrap_or_default();
    l::pat_from_text(&text)
}

fn seq_j(k: &Call, i: usize, name: &str) -> Vec<J> {
    k.arg(i, name).map(unwrap_seq).unwrap_or_default()
}

pub fn call(k: &Call) -> Option<Res> {
    Some(match k.func.as_str() {
        "derive" => {
            let slug = k.str(2, "base_slug");
            let src = k.opt_str(99, "base_slug_source").unwrap_or_else(|| "PAYLOAD.identity.base_slug".to_string());
            let declared = k.arg(99, "declared").map(|v| J::List(unwrap_seq(v))).unwrap_or(J::list());
            let depth = k.arg(99, "depth").cloned().unwrap_or(J::Int(l::DEFAULT_DEPTH));
            l::derive_j(
                &k.path(0, "repo"),
                &opt_j(k, 1, "ev"),
                &slug,
                &opt_j(k, 99, "publish_kst"),
                &depth,
                &declared,
                &opt_j(k, 99, "records"),
                &opt_j(k, 99, "this_topic"),
                &opt_j(k, 99, "identity"),
                &opt_j(k, 99, "cell_key"),
                &seq_j(k, 99, "axis_tokens"),
                &src,
            )
        }
        "receiver_summary" => l::receiver_summary(&opt_j(k, 0, "full"), &opt_j(k, 99, "excerpt_counts"), &opt_j(k, 99, "sealed_sources")),
        "reading_list" => l::reading_list(&opt_j(k, 0, "lineage")).map(J::List),
        "resolve_stem" => match k.arg(1, "stem") {
            Some(J::Str(s)) => l::resolve_stem(&opt_j(k, 0, "lineage"), s).map(J::from),
            _ => Ok(J::Null),
        },
        "require_documents" => l::require_documents(&opt_j(k, 0, "lineage")).map(|_| J::Null),
        "sealed_sources" => l::sealed_sources(&k.path(0, "repo"), &J::List(seq_j(k, 1, "paths"))).map(J::List),
        "record_cell_match" => l::record_cell_match(&k.path(0, "repo"), &k.str(1, "rid"), &opt_j(k, 2, "rec"), &k.str(3, "cell_key"))
            .map(|(a, b)| J::List(vec![J::from(a), J::Bool(b)])),
        "seeds_from_publications" => l::seeds_from_publications(
            &k.path(0, "repo"),
            &J::List(seq_j(k, 1, "records")),
            k.opt_str(99, "this_topic").as_deref(),
            &opt_j(k, 99, "identity"),
            k.opt_str(99, "cell_key").as_deref(),
            k.opt_str(99, "publish_kst").as_deref(),
        ),
        "seeds_from_model_priors" => pat_arg(k, 99, "pat").and_then(|p| {
            l::seeds_from_model_priors(
                &k.path(0, "repo"),
                &J::List(seq_j(k, 1, "records")),
                &opt_j(k, 99, "this_topic"),
                &opt_j(k, 99, "identity"),
                &p,
                k.opt_str(99, "publish_kst").as_deref(),
                &k.strs(99, "exclude"),
            )
        }),
        "model_aliases" => pat_arg(k, 1, "pat").and_then(|p| l::model_aliases(&J::List(seq_j(k, 0, "records")), &p)),
        "slug_pattern" => l::slug_pattern(&k.str(0, "base_slug")).map(|p| crate::jd! {"__re__" => p.pattern}),
        "tool_snapshot_rel" => l::tool_snapshot_rel(&k.str(0, "name"), &k.str(1, "rev")).map(J::Str),
        "_tool_candidate_problem" => l::_tool_candidate_problem(&k.path(0, "repo"), &opt_j(k, 1, "c")).map(J::from),
        "identity_equal" | "_identity_equal" => l::identity_equal(&opt_j(k, 0, "a"), &opt_j(k, 1, "b")).map(J::Bool),
        "load_publication_records" => l::load_publication_records(&k.path(0, "repo")).map(J::List),
        "parse_doc_name" => {
            let repo = k.opt_path(99, "repo");
            l::parse_doc_name(&crate::core::posix(&k.path(0, "path")), repo.as_deref())
        }
        "mention_graph" => {
            let roots = if k.has(99, "roots") { k.strs(99, "roots") } else { l::DEFAULT_ROOTS.map(String::from).to_vec() };
            let exclude = if k.has(99, "exclude") { k.strs(99, "exclude") } else { l::EXCLUDED_ROOTS.map(String::from).to_vec() };
            l::mention_graph(&k.path(0, "repo"), &roots, &exclude)
        }
        "supersede_banners" => l::supersede_banners(&k.path(0, "path")).map(J::from),
        "_parse_declared" => l::_parse_declared(&k.path(0, "repo"), &J::List(seq_j(k, 1, "declared")))
            .map(|v| J::List(v.into_iter().map(|(a, b)| J::List(vec![J::Str(a), J::Str(b)])).collect())),
        "_norm_key" => Ok(J::Str(l::_norm_key(&k.j(0, "s").display()))),
        "_norm_pointer" => {
            let (a, b) = l::_norm_pointer(&k.path(0, "repo"), &k.str(1, "raw"), &k.str(2, "base"));
            Ok(J::List(vec![J::from(a), J::from(b)]))
        }
        "_strip_ext" => Ok(J::Str(l::_strip_ext(&k.str(0, "name")))),
        "selftest" => Ok(J::from(l::selftest())),
        _ => return None,
    })
}
