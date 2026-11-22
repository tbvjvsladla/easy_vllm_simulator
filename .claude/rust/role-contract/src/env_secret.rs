//! `HF_TOKEN` in the workspace secret file, judged by presence only (plan_26112012 #11; h57, h58 D3 (가-1)).
//!
//! HF_TOKEN, when needed, is the key `HF_TOKEN` of the untracked workspace file `envs/.env` at a fixed path (ENV_SECRET_KEY_LEDGER);
//! no manifest field points to it any more. The judgement answers a bool and nothing else: the file's bytes stay inside
//! [`hf_token_present`] -- no output, JSON or log line ever carries a line of the file, another key's name or the value.
//!
//! One rule on both sides, over bytes (h117): a line `HF_TOKEN=<value>` -- lines end at `\n`; blanks of the C locale's `[[:space:]]`
//! may come before the key, so a `#` comment line never counts -- whose value has an ASCII printable byte (0x21-0x7E) other than
//! `"` (0x22) and `'` (0x27). Nothing is decoded as UTF-8 and no locale takes part: a value of only blanks, quotes, a CR of CRLF,
//! control bytes or non-ASCII bytes (U+00A0, broken UTF-8) gives no value. The main reads its own workspace file
//! ([`hf_token_present`]); the sub is asked by the main's remote probe (`scan_node_probe::collect_peer_model_env`) with
//! [`remote_presence_test`], whose answer is an exit code only.

use crate::env_file_mode::ENV_FILE_REL;
use std::path::Path;

/// The `envs/.env` key that holds the HF token (ENV_SECRET_KEY_LEDGER C2).
pub const HF_TOKEN_KEY: &str = "HF_TOKEN";
/// The manifest field that pointed to a token file before plan_26112012 #11. A manifest issued earlier may still carry it; the scan
/// does not read it for any verdict and says so in one warning line.
pub const RETIRED_TOKEN_POINTER_FIELD: &str = "hf_token_env_file";

/// `[[:space:]]` under `LC_ALL=C`: space, tab, line feed, vertical tab, form feed, carriage return.
const C_LOCALE_SPACE: [u8; 6] = *b" \t\n\x0b\x0c\r";

/// A byte that gives the key a value: ASCII printable (0x21-0x7E) and not a quote -- the remote line's `[!#-&(-~]`.
fn is_value_byte(byte: u8) -> bool {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'\''
}

/// `HF_TOKEN=<value>` after leading C-locale blanks, with a value byte in the value.
fn assigns_a_value(line: &[u8]) -> bool {
    let key_at = line.iter().position(|byte| !C_LOCALE_SPACE.contains(byte)).unwrap_or(line.len());
    line[key_at..]
        .strip_prefix(HF_TOKEN_KEY.as_bytes())
        .and_then(|rest| rest.strip_prefix(b"="))
        .is_some_and(|value| value.iter().copied().any(is_value_byte))
}

/// Whether `<workspace>/envs/.env` has a line that gives `HF_TOKEN` a value. An absent or unreadable file is `false`.
pub fn hf_token_present(workspace: &Path) -> bool {
    match std::fs::read(workspace.join(ENV_FILE_REL)) {
        Ok(bytes) => bytes.split(|&byte| byte == b'\n').any(assigns_a_value),
        Err(_) => false,
    }
}

/// The remote shell line of the same rule over `<work dir>/envs/.env` (`quoted_work_dir` is already shell-quoted). It sees the
/// main's bytes: under `LC_ALL=C` every byte is one character and `[[:space:]]` is `C_LOCALE_SPACE`, `-a` reads a file with a NUL
/// byte as text (lines end at `\n` only), and `[!#-&(-~]` is 0x21-0x7E without `"` and `'`. `grep -q` answers by its exit code
/// only, and the line prints `TOK=1` or `TOK=0` -- never a line of the file (errors go to /dev/null).
pub fn remote_presence_test(quoted_work_dir: &str) -> String {
    format!(
        "LC_ALL=C grep -aEq '^[[:space:]]*{HF_TOKEN_KEY}=.*[!#-&(-~]' {quoted_work_dir}/{ENV_FILE_REL} 2>/dev/null && echo TOK=1 || echo TOK=0"
    )
}
