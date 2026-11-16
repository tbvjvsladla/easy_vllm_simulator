//! hint_api bridge for `branch` (`branch.<fn>` requests → `hintlib::branch`). Value encoding = the capture shim's `to_json`.
use crate::api::{enc_path, unwrap_seq, Call, Res};
use crate::branch as b;
use crate::value::J;

fn opt(v: Option<String>) -> J {
    J::from(v)
}
fn pair(a: Option<String>, c: Option<String>) -> J {
    J::List(vec![J::from(a), J::from(c)])
}
fn strs_of(v: &J) -> Vec<String> {
    unwrap_seq(v).iter().map(J::display).collect()
}
fn ostr(c: &Call, i: usize, name: &str) -> Option<String> {
    c.arg(i, name).filter(|v| !v.is_null()).map(J::display)
}
fn ostr_kw(c: &Call, name: &str) -> Option<String> {
    c.kw(name).filter(|v| !v.is_null()).map(J::display)
}
/// `extra_files` dict → pairs.
fn pairs_of(v: Option<&J>) -> Option<Vec<(String, String)>> {
    v.and_then(J::as_dict).map(|m| m.iter().map(|(k, x)| (k.to_string(), x.display())).collect())
}

pub fn call(c: &Call) -> Option<Res> {
    let repo = || c.path(0, "repo");
    Some(match c.func.as_str() {
        "required_top" => b::required_top(&c.j(0, "payload_doc")).map(J::from),
        "synthetic_identity" => b::synthetic_identity().map(J::from),
        "identity_env" => b::identity_env(&c.str(0, "generated_utc")),
        "parse_ident_headers" => b::parse_ident_headers(&c.str(0, "raw")),
        "read_object_text" => b::read_object_text(&repo(), &c.str(1, "kind"), &c.str(2, "rev")).map(opt),
        "commit_identity" => b::commit_identity(&repo(), &c.str(1, "commit")),
        "scan_blobs" => {
            if let Some(bad) = unwrap_seq(&c.j(0, "files")).iter().find_map(|p| unwrap_seq(p).get(1).filter(|d| !d.is_dict()).cloned()) {
                return Some(Err(crate::core::HintError::py(
                    "AttributeError",
                    format!("'{}' object has no attribute 'decode'", if bad.is_null() { "NoneType" } else { bad.type_name() }),
                )));
            }
            let files: Vec<(String, Vec<u8>)> = unwrap_seq(&c.j(0, "files"))
                .iter()
                .map(|p| {
                    let it = unwrap_seq(p);
                    (it.first().map(J::display).unwrap_or_default(), it.get(1).and_then(b::j_bytes).unwrap_or_default())
                })
                .collect();
            let terms = c.strs(1, "terms");
            if c.kw("exempt_log").map(|v| !v.is_null()).unwrap_or(false) {
                let mut log = Vec::new();
                b::scan_blobs(&files, &terms, Some(&mut log)).map(J::from)
            } else {
                b::scan_blobs(&files, &terms, None).map(J::from)
            }
        }
        "identity_pii_hits" => b::identity_pii_hits(&c.strs(0, "terms"), ostr(c, 1, "ident").as_deref()).map(J::from),
        "allowlist_violation" => match c.arg(0, "rel") {
            Some(J::Str(s)) => b::allowlist_violation(s).map(opt),
            _ => Ok(J::from("빈 경로")),
        },
        "payload_files" => b::payload_files(&c.path(0, "payload_dir")).map(J::from),
        "_mode_of" => b::_mode_of(&c.path(0, "p")).map(J::from),
        "build_tree" => b::build_tree(&repo(), &c.path(1, "payload_dir"), &c.strs(2, "rels")).map(J::from),
        "tree_entries" => b::tree_entries(&repo(), &c.str(1, "rev")).map(J::List),
        "tree_paths" => b::tree_paths(&repo(), &c.str(1, "rev")).map(J::from),
        "tree_violations" => b::tree_violations(&repo(), &c.str(1, "rev")).map(J::from),
        "commit_message" => b::commit_message(&c.path(0, "payload_dir"), &c.str(1, "generated_utc")).map(J::from),
        "payload_doc_problems" => b::payload_doc_problems(
            &c.j(0, "payload"),
            &c.j(1, "prov"),
            &J::List(unwrap_seq(&c.j(2, "paths"))),
            &c.j(3, "message"),
            &c.j(4, "docs"),
            ostr_kw(c, "expect_tag").as_deref(),
        )
        .map(J::from),
        "doc_residue_problems" => b::doc_residue_problems(&c.j(0, "docs")).map(J::from),
        "hint_tip" => b::hint_tip(&repo()).map(opt),
        "_require_hint_not_checked_out" => b::_require_hint_not_checked_out(&repo()).map(|_| J::Null),
        "_update_hint_ref" => b::_update_hint_ref(&repo(), &c.str(1, "new"), ostr(c, 2, "expected_old").as_deref()).map(|_| J::Null),
        "guide_marker" => match c.arg(0, "fmt") {
            None => b::guide_marker(b::BRANCH_FORMAT).map(J::from),
            Some(J::Str(s)) => b::guide_marker(s).map(J::from),
            Some(other) => crate::core::fail("HINT_BRANCH_FORMAT_SHAPE", format!("형식 버전 모양이 아니다: {}", other.repr()), None),
        },
        "readme_format" => b::readme_format(&c.j(0, "data")).map(opt),
        "_format_num" => b::_format_num(ostr(c, 0, "fmt").as_deref()).map(J::from),
        "_commit_parents" => b::_commit_parents(&c.str(0, "raw")).map(J::from),
        "guide_info" => b::guide_info(&repo(), &c.str(1, "sha")).map(|(f, w)| pair(f, w)),
        "require_guide" => b::require_guide(&repo(), &c.str(1, "sha"), &ostr_kw(c, "where").unwrap_or_else(|| "안내 커밋".into())).map(J::from),
        "guide_commit" => b::guide_commit(&repo(), &c.str(1, "remote"), ostr_kw(c, "pinned").as_deref()).map(|(s, f)| J::List(vec![s.into(), f.into()])),
        "_transition_template_bytes" => b::_transition_template_bytes(&repo()).map(|d| b::bytes_j(&d)),
        "_transition_tree" => {
            let readme = c.arg(1, "readme").and_then(b::j_bytes).unwrap_or_default();
            b::_transition_tree(&repo(), &readme, c.kw("write").and_then(J::as_bool).unwrap_or(true)).map(J::from)
        }
        "_transition_commit" => b::_transition_commit(&repo(), &c.str(1, "parent"), &c.str(2, "tree"), &c.str(3, "utc")).map(J::from),
        "_existing_transition" => {
            b::_existing_transition(&repo(), &c.str(1, "commit"), ostr(c, 2, "parent").as_deref(), &c.str(3, "expected_tree")).map(J::from)
        }
        "_remote_hint_tip" => b::_remote_hint_tip(&repo(), &c.str(1, "remote")).map(opt),
        "_orphan_payloads" => b::_orphan_payloads(&repo(), &c.str(1, "local"), &c.str(2, "keep")).map(J::from),
        "_require_approval" => b::_require_approval(c.opt_str(0, "approved_by").as_deref(), ostr(c, 1, "approved_utc").as_deref()).map(J::from),
        "branch_transition" => b::branch_transition(
            &repo(),
            &c.kw("remote").map(J::display).unwrap_or_default(),
            &c.kw("generated_utc").map(J::display).unwrap_or_default(),
            c.kw("approved_by").and_then(J::as_str),
            ostr_kw(c, "approved_utc").as_deref(),
        ),
        "commit_payload" => b::commit_payload(
            &repo(),
            &c.path(1, "payload_dir"),
            &c.kw("message").map(J::display).unwrap_or_default(),
            &c.kw("generated_utc").map(J::display).unwrap_or_default(),
            ostr_kw(c, "guide").as_deref(),
            ostr_kw(c, "remote").as_deref(),
        )
        .map(J::from),
        "read_blob" => b::read_blob(&repo(), &c.str(1, "rev"), &c.str(2, "path")).map(J::from),
        "read_json_blob" => b::read_json_blob(&repo(), &c.str(1, "rev"), &c.str(2, "path")).map(|(d, e)| J::List(vec![J::from(d), J::from(e)])),
        "require_payload_anchor" => b::require_payload_anchor(&repo(), &c.str(1, "tag"), &c.str(2, "anchor")).map(|_| J::Null),
        "read_blobs" => b::read_blobs(&repo(), &strs_of(&c.j(1, "shas"))).map(|m| {
            let mut out = crate::value::Map::new();
            for (k, v) in m {
                out.insert(k, b::bytes_j(&v));
            }
            J::Dict(out)
        }),
        "export_tree" => b::export_tree(&repo(), &c.str(1, "rev"), &c.path(2, "dest")).map(J::from),
        "commit_violations" => b::commit_violations(&repo(), &c.str(1, "commit"), &c.str(2, "tag"), &c.strs(3, "terms")).map(J::from),
        "selftest_repo" => b::selftest_repo(&c.path(0, "tmp"), &ostr(c, 1, "name").unwrap_or_else(|| "repo".into())).map(|p| enc_path(&p)),
        "selftest_guide" => b::selftest_guide(
            &repo(),
            match c.kw("fmt") {
                None => Some(b::BRANCH_FORMAT.to_string()),
                Some(v) => v.as_str().map(str::to_string),
            }
            .as_deref(),
            ostr_kw(c, "parent").as_deref(),
            &ostr_kw(c, "body").unwrap_or_else(|| b::FX_GUIDE_BODY.into()),
            ostr_kw(c, "utc").as_deref(),
        )
        .map(J::from),
        "fixture_payload_doc" => b::fixture_payload_doc(&c.str(0, "tag")),
        "fixture_hint_doc" => b::fixture_hint_doc(&ostr(c, 0, "brief").unwrap_or_else(|| b::FX_BRIEF.into())).map(J::from),
        "write_fixture_payload" => {
            let extra = pairs_of(c.kw("extra_files"));
            let pd = c.kw("payload_doc").filter(|v| !v.is_null()).cloned();
            b::write_fixture_payload(
                &c.path(0, "root"),
                &c.str(1, "tag"),
                ostr_kw(c, "source_anchor").as_deref(),
                extra.as_deref(),
                pd.as_ref(),
                &ostr_kw(c, "brief").unwrap_or_else(|| b::FX_BRIEF.into()),
            )
            .map(|_| J::Null)
        }
        "selftest" => Ok(J::from(b::selftest())),
        _ => return None,
    })
}
