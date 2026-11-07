//! `upstream_toml_api evaluate-jsonl [--input-fd FD]` (see `upstream_runtime::tomllib::service`): the normal program of the default
//! `tomllib.loads(text)` parser.

use std::process::ExitCode;

fn main() -> ExitCode {
    upstream_runtime::jsonl_program::run(
        "upstream_toml_api",
        upstream_runtime::tomllib::service::evaluate_jsonl,
    )
}
