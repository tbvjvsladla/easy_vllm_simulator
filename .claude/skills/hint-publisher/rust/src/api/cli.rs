//! hint_api bridge for `cli` (module name in requests: `hint`, the original `scripts/hint.py`).
use crate::api::{Call, Res};
use crate::cli;
use crate::value::J;

fn opt(c: &Call, i: usize, name: &str) -> Option<String> {
    c.opt_str(i, name)
}

pub fn call(c: &Call) -> Option<Res> {
    Some(match c.func.as_str() {
        "_factcheck_problems" => cli::_factcheck_problems(&c.j(0, "doc"), &c.j(1, "st")).map(J::from),
        "_one_line_utterance" => cli::_one_line_utterance(opt(c, 0, "text").as_deref(), &c.str(1, "code"), &c.str(2, "what")).map(J::from),
        "_with_bench_command" => Ok(cli::_with_bench_command(&c.j(0, "steps"), &c.j(1, "bc"))),
        "_build_doc" => Ok(cli::_build_doc(&c.j(0, "ev"), &c.j(1, "art"))),
        "_applied_summary" => Ok(cli::_applied_summary(&c.j(0, "aset"))),
        "_check_selectors" => cli::_check_selectors(
            opt(c, 99, "campaign").as_deref(),
            opt(c, 99, "cell").as_deref(),
            opt(c, 99, "publication").as_deref(),
            c.bool_or(99, "replay", false),
            c.bool_or(99, "require_replay", false),
        )
        .map(|_| J::Null),
        "_require_central" => cli::_require_central(&c.path(0, "repo"), &c.str(1, "action")).map(|_| J::Null),
        "_read_tail" => cli::_read_tail(&c.path(0, "draft")),
        "_tail_issues" => cli::_tail_issues(&c.path(0, "draft")).map(J::List),
        "_lineage_full" => cli::_lineage_full(&c.path(0, "draft"), &c.path(1, "payload")),
        "_load_state" => cli::_load_state(&c.path(0, "draft")),
        "_step" => Ok(J::from(cli::_step(&c.j(0, "st"), &c.str(1, "name")))),
        "self_test" => Ok(J::from(cli::self_test() as i64)),
        _ => return None,
    })
}
