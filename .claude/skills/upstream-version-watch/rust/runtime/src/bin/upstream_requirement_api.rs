//! `upstream_requirement_api evaluate-jsonl [--input-fd FD]` (see `upstream_runtime::requirement::service`): the normal program of the
//! requirement constructor projection.

use std::process::ExitCode;

fn main() -> ExitCode {
    upstream_runtime::jsonl_program::run(
        "upstream_requirement_api",
        upstream_runtime::requirement::service::evaluate_jsonl,
    )
}
