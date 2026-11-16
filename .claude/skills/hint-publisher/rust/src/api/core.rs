//! hint_api bridge for `core`.
use crate::api::{enc_path, Call, Res};
use crate::core as c;
use crate::value::J;

pub fn call(k: &Call) -> Option<Res> {
    Some(match k.func.as_str() {
        "kst_iso" => c::kst_iso(&k.str(0, "utc")).map(J::Str),
        "kst_token" => c::kst_token(&k.str(0, "utc")).map(J::Str),
        "git_date" => c::git_date(&k.str(0, "utc")).map(J::Str),
        "require_utc" => {
            let what = k.opt_str(1, "what").unwrap_or_else(|| "--generated-utc".to_string());
            c::require_utc(k.arg(0, "value").and_then(J::as_str), &what).map(J::Str)
        }
        "dumps" => Ok(J::Str(c::dumps(&k.j(0, "obj")))),
        "sha256_bytes" => Ok(J::Str(c::sha256_bytes(k.str(0, "data").as_bytes()))),
        "sha256_file" => c::sha256_file(&k.path(0, "path")).map(J::Str),
        "read_json" => c::read_json(&k.path(0, "path")),
        "write_json" => c::write_json(&k.path(0, "path"), &k.j(1, "obj")).map(|_| J::Null),
        "rel" => c::rel(&k.path(0, "repo"), &k.path(1, "path")).map(J::Str),
        "resolve_repo" => {
            let e = k.arg(0, "explicit").filter(|v| !v.is_null()).map(|v| crate::core::posix(&crate::api::decode_path(v)));
            c::resolve_repo(e.as_deref(), k.bool_or(1, "require_git", true)).map(|p| enc_path(&p))
        }
        "level_raw_is_measured_tool" => Ok(J::Bool(c::level_raw_is_measured_tool(&k.path(0, "path"), &k.str(1, "cell")))),
        "selftest" => Ok(J::from(c::selftest())),
        _ => return None,
    })
}
