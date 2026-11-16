//! hintlib.artifacts — port of `scripts/hintlib/artifacts.py`, split by source line range into parts (one owner each).
//! Every part glob-imports its siblings through this module (`use super::*`), so a Python call `f(...)` inside the
//! module stays `f(...)` in any part.
mod a1;
mod a2;
mod a3;
mod selftest;
pub use a1::*;
pub use a2::*;
pub use a3::*;

/// `artifacts.selftest()` — ported by the selftest owner.
pub fn selftest() -> Vec<String> {
    selftest::selftest()
}
