//! `upstream_https_api evaluate-jsonl [--input-fd FD]` (see `upstream_runtime::fetch_https::jsonl`): the normal program of the HTTPS fetch.

use std::process::ExitCode;

fn main() -> ExitCode {
    upstream_runtime::jsonl_program::run(
        "upstream_https_api",
        upstream_runtime::fetch_https::jsonl::evaluate_jsonl,
    )
}
