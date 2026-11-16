//! hint_api bridge for `pii` (`pii.<fn>` requests → `hintlib::pii`). Value encoding = the capture shim's `to_json`.
use std::path::PathBuf;

use crate::api::{decode_path, enc_path, unwrap_seq, Call, Res};
use crate::core::HintError;
use crate::pii as p;
use crate::value::J;

/// Resolve `{"__call__": ...}` nested anywhere inside an argument (the Python shim's `from_json` is recursive).
fn deep(v: &J) -> Result<J, HintError> {
    match v {
        J::Dict(m) if m.len() == 1 && m.0[0].0 == "__call__" => {
            let (r, o, e) = crate::api::dispatch(&m.0[0].1);
            crate::io::out(&o);
            crate::io::err(&e);
            r
        }
        J::Dict(m) if m.len() == 1 && m.0[0].0 == "__tuple__" => {
            let items: Result<Vec<J>, HintError> = m.0[0].1.items().iter().map(deep).collect();
            Ok(crate::jd! {"__tuple__" => items?})
        }
        J::List(l) => Ok(J::List(l.iter().map(deep).collect::<Result<_, _>>()?)),
        other => Ok(other.clone()),
    }
}

fn hit_from(v: &J) -> Option<p::Hit> {
    if v.get_str("__class__") != Some("Hit") {
        return None;
    }
    Some(p::Hit::new(v.get_str("pattern")?, v.get_str("match")?, v.get("line")?.as_i64()?))
}

fn hit_items(v: &J) -> Result<Vec<p::HitItem>, HintError> {
    let mut out = Vec::new();
    for it in unwrap_seq(&deep(v)?) {
        if let Some(h) = hit_from(&it) {
            out.push(p::HitItem::Bare(h));
            continue;
        }
        let pair = unwrap_seq(&it);
        match (pair.first().and_then(J::as_str), pair.get(1).and_then(hit_from)) {
            (Some(w), Some(h)) if pair.len() == 2 => out.push(p::HitItem::Located(w.to_string(), h)),
            _ => return Err(HintError::py("AttributeError", format!("{} has no attribute 'line'", it.repr()))),
        }
    }
    Ok(out)
}

/// `skip_generic` iterable (`frozenset(x)`: a str iterates its characters).
fn strs_of(v: Option<&J>) -> Vec<String> {
    match v {
        None | Some(J::Null) => Vec::new(),
        Some(J::Str(s)) => s.chars().map(|c| c.to_string()).collect(),
        Some(J::Dict(m)) if !matches!(p::py_type(&J::Dict(m.clone())), "tuple" | "set") => m.keys().map(str::to_string).collect(),
        Some(x) => unwrap_seq(x).iter().filter_map(|y| y.as_str().map(str::to_string)).collect(),
    }
}

/// Character index (Python) → byte offset.
fn byte_at(text: &str, cp: i64) -> usize {
    if cp <= 0 {
        return 0;
    }
    text.char_indices().nth(cp as usize).map(|(i, _)| i).unwrap_or(text.len())
}

fn hits_j(h: Vec<p::Hit>) -> J {
    J::List(h.iter().map(p::Hit::to_j).collect())
}

fn located_j(h: Vec<(String, p::Hit)>) -> J {
    J::List(h.into_iter().map(|(r, h)| J::List(vec![J::Str(r), h.to_j()])).collect())
}

fn paths_j(v: Vec<(String, PathBuf)>) -> J {
    J::List(v.into_iter().map(|(r, q)| J::List(vec![J::Str(r), enc_path(&q)])).collect())
}

fn pairs_j(v: Vec<(String, String)>) -> J {
    J::List(v.into_iter().map(|(a, b)| J::List(vec![J::Str(a), J::Str(b)])).collect())
}

/// `exempt_log` given (a list) → a local log (the caller's list is not observable through the bridge on either side);
/// `None`/absent → stderr.
fn log_given(k: &Call, i: usize) -> bool {
    k.arg(i, "exempt_log").is_some_and(|v| !v.is_null())
}

fn norm_path(v: &J) -> PathBuf {
    let s = crate::core::posix(&decode_path(v));
    let parts: Vec<&str> = s.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
    let mut out = if s.starts_with('/') { "/".to_string() } else { String::new() };
    out.push_str(&parts.join("/"));
    if out.is_empty() {
        out.push('.');
    }
    PathBuf::from(out)
}

pub fn call(k: &Call) -> Option<Res> {
    Some(match k.func.as_str() {
        "Hit" => Ok(p::Hit::new(&k.str(0, "pattern"), &k.str(1, "match"), k.int_or(2, "line", 0)).to_j()),
        "Hit.__str__" => match deep(&k.j(0, "self")).ok().as_ref().and_then(hit_from) {
            Some(h) => Ok(J::Str(h.to_string())),
            None => Err(HintError::py("TypeError", "descriptor '__str__' requires a 'Hit' object")),
        },
        "render_hits" => hit_items(&k.j(0, "hits")).map(|items| J::Str(p::_render_items(&items, k.int_or(1, "limit", 20)))),
        "load_pii_terms" => p::load_pii_terms(&k.path(0, "repo")).map(J::from),
        "require_terms" => p::require_terms(&k.path(0, "repo")).map(J::from),
        "_patterns_for" => p::_patterns_for(&k.str(0, "profile"), &strs_of(k.arg(1, "skip_generic"))).map(J::from),
        "_line_prefix" => {
            let t = k.str(0, "text");
            Ok(J::Str(p::_line_prefix(&t, byte_at(&t, k.int_or(1, "start", 0)))))
        }
        "_is_section_anchored" => {
            let t = k.str(0, "text");
            Ok(J::Bool(p::_is_section_anchored(&t, byte_at(&t, k.int_or(1, "start", 0)))))
        }
        "_is_shell_default" => {
            let t = k.str(0, "text");
            Ok(J::Bool(p::_is_shell_default(&t, byte_at(&t, k.int_or(1, "start", 0)))))
        }
        "_is_comment_echo_of_default" => {
            let t = k.str(0, "text");
            Ok(J::Bool(p::_is_comment_echo_of_default(&t, byte_at(&t, k.int_or(1, "start", 0)), &k.str(2, "literal"))))
        }
        "_dedupe" => deep(&k.j(0, "terms")).and_then(|t| p::_dedupe(&p::_terms_from_j(&t))).map(J::from),
        "scan_text" => (|| {
            let text = deep(&k.j(0, "text"))?;
            let text = p::_text_arg(&text, "scan_text")?;
            let terms = p::_terms_from_j(&deep(&k.j(1, "terms"))?);
            let profile = k.kw("profile").and_then(J::as_str).unwrap_or("deploy").to_string();
            let skip = strs_of(k.kw("skip_generic"));
            let mut local: Vec<J> = Vec::new();
            let log = if log_given(k, usize::MAX) { Some(&mut local) } else { None };
            p::_scan_text(
                text,
                &terms,
                &profile,
                &skip,
                k.kw("shell_default_exempt").and_then(J::as_bool).unwrap_or(false),
                k.kw("section_anchor_exempt").and_then(J::as_bool).unwrap_or(true),
                log,
            )
            .map(hits_j)
        })(),
        "_read_scannable" => p::_read_scannable(&decode_path(&k.j(0, "path"))).map(J::from),
        "_walk" => p::_walk(&norm_path(&k.j(0, "root")), &norm_path(&k.j(1, "base"))).map(paths_j),
        "_selected" => p::_selected(&norm_path(&k.j(0, "root")), &unwrap_seq(&k.j(1, "rels"))).map(paths_j),
        "scan_tree" => (|| {
            let terms = p::_terms_from_j(&deep(&k.j(1, "terms"))?);
            let profile = k.kw("profile").and_then(J::as_str).unwrap_or("deploy").to_string();
            let skip = strs_of(k.kw("skip_generic"));
            let rels: Option<Vec<J>> = k.kw("rels").filter(|v| !v.is_null()).map(unwrap_seq);
            let mut local: Vec<J> = Vec::new();
            let log = if log_given(k, usize::MAX) { Some(&mut local) } else { None };
            p::_scan_tree(
                &decode_path(&k.j(0, "root")),
                &terms,
                &profile,
                &skip,
                k.kw("shell_default_exempt").and_then(J::as_bool).unwrap_or(true),
                k.kw("section_anchor_exempt").and_then(J::as_bool).unwrap_or(true),
                rels.as_deref(),
                log,
            )
            .map(located_j)
        })(),
        "node_placeholder" => p::node_placeholder(&k.str(0, "label")).map(J::Str),
        "manifest_placeholder" => p::manifest_placeholder(&k.str(0, "field")).map(J::Str),
        "nic_placeholder" => p::nic_placeholder(&k.str(0, "label")).map(J::Str),
        "_is_nic_key" => Ok(J::Bool(p::_is_nic_key(&k.arg(0, "key").map(J::display).unwrap_or_default()))),
        "_nic_values" => Ok(J::from(p::_nic_values(&k.j(0, "val")))),
        "_is_ipv4" => Ok(J::Bool(p::_is_ipv4(&k.str(0, "v")))),
        "_is_host_key" => Ok(J::Bool(p::_is_host_key(&k.str(0, "key")))),
        "_is_abs_path" => Ok(J::Bool(p::_is_abs_path(&k.str(0, "v")))),
        "_home_of" => Ok(J::from(p::_home_of(&k.str(0, "path")))),
        "_node_labels" => Ok(J::from(p::_node_labels(k.j(0, "nodes").items()))),
        "_walk_strings" => Ok(pairs_j(p::_walk_strings(&k.j(0, "obj"), &k.str(1, "prefix")))),
        "substitution_table" => p::substitution_table(&k.path(0, "repo"), &k.j(1, "manifest")).map(pairs_j),
        "_literal_regex" => {
            let ph = k.arg(1, "placeholder").and_then(J::as_str).map(str::to_string);
            Ok(J::Str(p::_literal_regex(&k.str(0, "literal"), ph.as_deref())))
        }
        "_table_rows" => p::_table_rows_j(&k.j(0, "table")).map(pairs_j),
        "_compiled" => p::_table_rows_j(&k.j(0, "table")).and_then(|rows| p::_compiled(&rows)).map(|(text, _, repl)| {
            let mut m = crate::value::Map::new();
            for (a, b) in repl {
                m.insert(a, b);
            }
            J::List(vec![crate::jd! {"__re__" => text}, J::Dict(m)])
        }),
        "_mask_residual" => Ok(J::Str(p::_mask_residual(&k.str(0, "text"), &k.str(1, "name"), &k.str(2, "placeholder")))),
        "substitute" => (|| {
            let text = deep(&k.j(0, "text"))?;
            let text = p::_text_arg(&text, "substitute")?;
            let rows = p::_table_rows_j(&deep(&k.j(1, "table"))?)?;
            p::substitute(text, &rows, k.kw("residual").and_then(J::as_bool).unwrap_or(true)).map(J::Str)
        })(),
        "_ip" => Ok(J::Str(p::_ip(&k.args.iter().filter_map(J::as_i64).collect::<Vec<_>>()))),
        "selftest" => Ok(J::from(p::selftest())),
        _ => return None,
    })
}
