//! hint_api bridge for `catalog`.
use crate::api::{enc_path, Call, Res};
use crate::catalog as c;
use crate::value::J;

fn pair((a, b): (String, String)) -> J {
    J::List(vec![J::Str(a), J::Str(b)])
}

pub fn call(k: &Call) -> Option<Res> {
    Some(match k.func.as_str() {
        "extract_brief" => c::extract_brief(&k.str(0, "body")).map(J::Str),
        "annotation_brief" => c::annotation_brief(&k.str(0, "body")).map(J::Str),
        "md_cell" => c::md_cell(&k.j(0, "s")).map(J::Str),
        "bench_mode_cell" => c::bench_mode_cell(&k.j(0, "doc")).map(pair),
        "verdict_cell" => c::verdict_cell(&k.j(0, "doc")).map(pair),
        "tagger_utc" => c::tagger_utc(&k.str(0, "header")).map(J::from),
        "query_slug" => c::query_slug(k.arg(0, "s").and_then(J::as_str)).map(J::Str),
        "_remote_label" => c::_remote_label(&k.str(0, "remote")).map(J::Str),
        "remote_hint_tags" => c::remote_hint_tags(&k.path(0, "repo"), &k.str(1, "remote"), k.bool_or(2, "allow_empty", false)),
        "payload_doc" => c::payload_doc(&k.path(0, "repo"), &k.str(1, "obj")).map(J::from),
        "main_worktree" => c::main_worktree(&k.path(0, "repo")).map(|p| enc_path(&p)),
        "fetch_hint" => c::fetch_hint(&k.path(0, "repo"), k.arg(1, "remote").and_then(J::as_str)).map(J::Str),
        "parse_name" => c::parse_name(&k.str(0, "name")),
        "_payload_grammars" => c::_payload_grammars().map(J::from),
        "_base_row" => c::_base_row(&k.str(0, "name"), &k.j(1, "parsed"), &k.strs(2, "payload_grammars")),
        "derive_entries" => c::derive_entries(
            &k.path(0, "repo"),
            &k.j(1, "remote_tags"),
            k.arg(2, "remote").and_then(J::as_str),
            k.bool_or(3, "record_missing", false),
        )
        .map(|(e, n)| J::List(vec![J::List(e), n])),
        "build_index" => c::build_index(&k.j(0, "entries"), &k.str(1, "remote"), &k.str(2, "generated_kst")),
        "_missing_cell" => c::_missing_cell(&k.j(0, "e")).map(J::Str),
        "_verdict_display" => c::_verdict_display(&k.j(0, "e")).map(J::Str),
        "render_rows" => c::render_rows(&k.j(0, "entries")).map(J::Str),
        "_group_key" => c::_group_key(&k.j(0, "e")).map(J::Str),
        "order_entries" => c::order_entries(&k.j(0, "entries"))
            .map(|v| J::List(v.into_iter().map(|(g, rows)| J::List(vec![J::Str(g), J::List(rows)])).collect())),
        "render_derived_block" => c::render_derived_block(&k.j(0, "index")).map(J::Str),
        "locate_derived_block" => c::locate_derived_block(&k.str(0, "text")).map(|(a, b)| J::List(vec![J::Int(a), J::Int(b)])),
        "splice_hints_md" => c::splice_hints_md(&k.str(0, "text"), &k.str(1, "block")).map(J::Str),
        "derive" => c::derive(
            &k.path(0, "repo"),
            &k.str(1, "remote"),
            &k.str(2, "generated_kst"),
            k.bool_or(3, "dry_run", false),
            k.bool_or(4, "record_missing", false),
            k.bool_or(5, "allow_empty", false),
        )
        .map(J::Int),
        "load_index" => c::load_index(&k.path(0, "repo")),
        "_hw_token" => c::_hw_token(&k.str(0, "arch")).map(J::Str),
        "_arch_match" => c::_arch_match(&k.str(0, "entry_arch"), k.arg(1, "query").and_then(J::as_str)).map(J::Str),
        "_vnorm" => c::_vnorm(k.arg(0, "v").and_then(J::as_str)).map(J::Str),
        "match" => c::r#match(
            &k.j(0, "index"),
            &k.str(1, "vllm"),
            &k.str(2, "model"),
            k.arg(3, "arch").and_then(J::as_str),
            k.bool_or(4, "include_other", false),
        )
        .map(J::List),
        "format_match" => {
            c::format_match(&k.j(0, "index"), &k.j(1, "results"), &k.str(2, "model"), k.bool_or(3, "include_other", false)).map(J::Str)
        }
        "selftest" => Ok(J::from(c::selftest())),
        _ => return None,
    })
}
