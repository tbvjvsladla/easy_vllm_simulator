//! hintlib.template — port of `scripts/hintlib/template.py`, split by source line range into parts (one owner each).
//! Every part glob-imports its siblings through this module (`use super::*`), so a Python call `f(...)` inside the
//! module stays `f(...)` in any part.
mod selftest;
mod t1;
mod t2;
pub use t1::*;
pub use t2::*;

/// `template.selftest()` — `[]` = pass (port in `selftest.rs`).
pub fn selftest() -> Vec<String> {
    selftest::selftest()
}
