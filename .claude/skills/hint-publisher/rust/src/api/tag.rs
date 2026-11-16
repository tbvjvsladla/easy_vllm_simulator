//! hint_api bridge for `tag` (`tag.<fn>` requests → `hintlib::tag`). Value encoding = the capture shim's `to_json`.
//! `HintEvidenceBindingError` escapes as the shim reports a non-HintError exception: `{type, message = str(e)}`;
//! `CompletedProcess` = `{"__repr__": repr}`; `bytes` = `{"__bytes__": hex}`; a `set` = sorted list.
use crate::api::{unwrap_seq, Call, Res};
use crate::core::HintError;
use crate::tag as t;
use crate::value::J;

fn opt_s(v: Option<String>) -> J {
    J::from(v)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn snapshot(v: &J) -> Vec<J> {
    unwrap_seq(v)
}

fn inner(c: &Call) -> Option<Res> {
    let repo = || c.path(0, "repo");
    Some(match c.func.as_str() {
        "problem_codes" => t::problem_codes(&c.strs(0, "problems")).map(J::from),
        "bench_kind_of" => t::bench_kind_of(&c.j(0, "ref")).map(opt_s),
        "_checked_fields" => t::_checked_fields(&c.j(0, "fields")),
        "build_footer" => {
            let fields = if let Some(f) = c.args.first() { f.clone() } else { J::Dict(c.kwargs.clone()) };
            t::build_footer(&fields).map(J::Str)
        }
        "parse_footer" => {
            let retired = c.kw("retired_ok").map(|v| unwrap_seq(v).iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
            match c.arg(0, "text") {
                Some(J::Str(s)) => t::parse_footer(s, &retired),
                Some(o) => t::_malformed(&format!("footer 입력은 문자열이어야 한다: {}", o.type_name())),
                None => t::_malformed("footer 입력은 문자열이어야 한다: NoneType"),
            }
        }
        "bench_binding" => t::bench_binding(&c.j(0, "footer")),
        "_canonical_brief" => t::_canonical_brief(&c.j(0, "brief")).map(J::Str),
        "annotation" => t::annotation_j(&c.j(0, "brief"), &c.j(1, "fields")).map(J::Str),
        "parse_annotation" => t::parse_annotation(&c.str(0, "body")),
        "check_ref_format" => t::check_ref_format(&repo(), &c.str(1, "tag")).map(|_| J::Null),
        "tag_ref_kind" => t::tag_ref_kind(&repo(), &c.str(1, "tag")).map(opt_s),
        "_ref_sha" => t::_ref_sha(&repo(), &c.str(1, "ref")).map(opt_s),
        "read_tag" => t::read_tag(&repo(), &c.str(1, "tag")).map(J::from),
        "_local_hint_refs" => t::_local_hint_refs(&repo()).map(J::from),
        "_prefix_conflict" => t::_prefix_conflict(&c.str(0, "ref"), &c.strs(1, "refs")).map(opt_s),
        "name_collision" => t::name_collision(&repo(), &c.str(1, "tag"), c.opt_str(2, "remote").as_deref()).map(opt_s),
        "_tag_object_bytes" => t::_tag_object_bytes(&c.str(0, "anchor"), &c.str(1, "tag"), &c.str(2, "utc"), &c.str(3, "message"))
            .map(|b| crate::jd! {"__bytes__" => hex(&b)}),
        "_brief_problems" => t::_brief_problems(&repo(), &c.str(1, "anchor"), &c.str(2, "brief")).map(J::from),
        "_facts_problems" => t::_facts_problems(&repo(), &c.str(1, "anchor"), &c.str(2, "tag")).map(J::from),
        "_anchor_problems" => t::_anchor_problems(
            &repo(),
            &c.str(1, "tag"),
            &c.str(2, "anchor"),
            c.opt_str(3, "brief").as_deref(),
            &c.str(4, "body"),
            &c.strs(5, "terms"),
        )
        .map(J::from),
        "sealed_sources" => t::sealed_sources(&repo(), &c.str(1, "anchor")).map(|(a, b)| J::List(vec![J::from(a.map(J::List)), J::from(b)])),
        "changed_sealed_sources" => t::changed_sealed_sources(&repo(), &snapshot(&c.j(1, "snapshot"))).map(J::from),
        "seal" => t::seal(
            &repo(),
            &c.str(1, "tag"),
            &c.str(2, "anchor"),
            &c.str(3, "message"),
            &c.str(4, "generated_utc"),
            c.opt_str(5, "remote").as_deref(),
        )
        .map(J::Str),
        "_create_tag" => t::_create_tag(&repo(), &c.str(1, "tag"), &c.str(2, "anchor"), &c.str(3, "message"), &c.str(4, "utc")).map(J::Str),
        "verify_local" => {
            let src = c.opt_path(4, "source_repo");
            t::verify_local(&repo(), &c.str(1, "tag"), None, None, src.as_deref()).map(J::from)
        }
        "_downgrade_source_drift" => {
            let src = c.opt_path(4, "source_repo");
            t::_downgrade_source_drift(&repo(), &c.str(1, "anchor"), &c.strs(2, "findings"), None, src.as_deref()).map(J::from)
        }
        "_redact_url" => t::_redact_url(&c.str(0, "url")).map(J::Str),
        "_require_remote_arg" => match c.arg(0, "remote") {
            Some(J::Str(s)) => t::_require_remote_arg(s).map(|_| J::Null),
            _ => crate::core::fail("HINT_PUSH_REMOTE_UNRESOLVED", "원격 이름(또는 URL·경로)이 비었다.", None),
        },
        "_is_remote_nick" => t::_is_remote_nick(&c.str(0, "name")).map(J::Bool),
        "_declared_urls" => t::_declared_urls(&repo(), &c.str(1, "remote"), c.bool_or(2, "push", false)).map(J::from),
        "_transport_of" => t::_transport_of(&c.str(0, "url")).map(J::Str),
        "_remote_transports" => t::_remote_transports(&repo(), &c.str(1, "remote"), c.bool_or(2, "push", false)).map(J::from),
        "remote_needs_token" => t::remote_needs_token(&repo(), &c.str(1, "remote")).map(J::Bool),
        "read_push_token" => t::read_push_token(&repo(), &c.j(1, "environ")).map(opt_s),
        "push_credential_args" => t::push_credential_args().map(J::from),
        "_scrub" => t::_scrub(&c.str(0, "text"), c.opt_str(1, "token").as_deref()).map(J::Str),
        "_ls_remote_refs" => t::_ls_remote_refs(&repo(), &c.str(1, "remote"), &c.str(2, "pattern")),
        "remote_tag_object" => t::remote_tag_object(&repo(), &c.str(1, "remote"), &c.str(2, "tag")).map(opt_s),
        "remote_ref_object" => t::remote_ref_object(&repo(), &c.str(1, "remote"), &c.str(2, "ref")).map(opt_s),
        "fetch_ref_objects" => t::fetch_ref_objects(&repo(), &c.str(1, "remote"), &c.str(2, "ref")).map(opt_s),
        "_require_safe_refspec" => t::_require_safe_refspec(&repo(), &c.str(1, "refspec")).map(|_| J::Null),
        "git_push_authenticated" => t::git_push_authenticated(&repo(), &c.str(1, "remote"), &c.str(2, "refspec"), c.bool_or(3, "dry_run", false))
            .map(|p| crate::jd! {"__repr__" => t::completed_process_repr(&p)}),
        "push_tag" => {
            let src = c.opt_path(6, "source_repo");
            t::push_tag(&repo(), &c.str(1, "remote"), &c.str(2, "tag"), c.bool_or(3, "dry_run", false), None, None, src.as_deref())
        }
        "selftest" => Ok(J::from(t::selftest())),
        _ => return None,
    })
}

pub fn call(c: &Call) -> Option<Res> {
    let r = inner(c)?;
    Some(r.map_err(|e: HintError| {
        if t::is_evidence_binding_error(&e) {
            HintError::py("HintEvidenceBindingError", e.str())
        } else {
            e
        }
    }))
}
