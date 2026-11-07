//! `upstream_http_direct_api evaluate-jsonl [--input-fd FD]` (see `upstream_runtime::http_direct::jsonl`): the normal program of the direct-HTTP
//! fetch; the command line is the shared one of `upstream_runtime::jsonl_program`.

use std::process::ExitCode;

fn main() -> ExitCode {
    upstream_runtime::jsonl_program::run(
        "upstream_http_direct_api",
        upstream_runtime::http_direct::jsonl::evaluate_jsonl,
    )
}
