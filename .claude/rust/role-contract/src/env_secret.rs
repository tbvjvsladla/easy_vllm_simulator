//! `HF_TOKEN` in the workspace secret file, judged by presence only (plan_26112012 #11; h57, h58 D3 (가-1)).
//!
//! HF_TOKEN, when needed, is the key `HF_TOKEN` of the untracked workspace file `envs/.env` at a fixed path (ENV_SECRET_KEY_LEDGER);
//! no manifest field points to it any more. The judgement answers a bool and nothing else: the file's bytes stay inside
//! [`hf_token_present`] -- no output, JSON or log line ever carries a line of the file, another key's name or the value.
//!
//! One rule on both sides: a line `HF_TOKEN=<value>` (leading whitespace allowed; a `#` comment line never counts) whose value has a
//! character other than whitespace and quotes. The main reads its own workspace file ([`hf_token_present`]); the sub is asked by the
//! main's remote probe (`scan_node_probe::collect_peer_model_env`) with [`remote_presence_test`], whose answer is an exit code only.

use crate::env_file_mode::ENV_FILE_REL;
use std::path::Path;

/// The `envs/.env` key that holds the HF token (ENV_SECRET_KEY_LEDGER C2).
pub const HF_TOKEN_KEY: &str = "HF_TOKEN";
/// The manifest field that pointed to a token file before plan_26112012 #11. A manifest issued earlier may still carry it; the scan
/// does not read it for any verdict and says so in one warning line.
pub const RETIRED_TOKEN_POINTER_FIELD: &str = "hf_token_env_file";

/// `HF_TOKEN=<value>` with a value that has a character other than whitespace and quotes.
fn assigns_a_value(line: &str) -> bool {
    line.trim_start()
        .strip_prefix(HF_TOKEN_KEY)
        .and_then(|rest| rest.strip_prefix('='))
        .is_some_and(|value| value.chars().any(|c| !c.is_whitespace() && c != '"' && c != '\''))
}

/// Whether `<workspace>/envs/.env` has a line that gives `HF_TOKEN` a value. An absent or unreadable file is `false`.
pub fn hf_token_present(workspace: &Path) -> bool {
    match std::fs::read(workspace.join(ENV_FILE_REL)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).lines().any(assigns_a_value),
        Err(_) => false,
    }
}

/// The remote shell line of the same rule over `<work dir>/envs/.env` (`quoted_work_dir` is already shell-quoted): `grep -Eq` answers
/// by its exit code only, and the line prints `TOK=1` or `TOK=0` -- never a line of the file (errors go to /dev/null).
pub fn remote_presence_test(quoted_work_dir: &str) -> String {
    format!(
        "grep -Eq \"^[[:space:]]*{HF_TOKEN_KEY}=.*[^[:space:]\\\"']\" {quoted_work_dir}/{ENV_FILE_REL} 2>/dev/null && echo TOK=1 || echo TOK=0"
    )
}
