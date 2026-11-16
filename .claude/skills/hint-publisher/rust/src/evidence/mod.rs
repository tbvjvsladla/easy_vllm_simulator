//! hintlib.evidence — port of `scripts/hintlib/evidence.py`, split by source line range into parts (one owner each).
//! Every part glob-imports its siblings through this module (`use super::*`), so a Python call `f(...)` inside the
//! module stays `f(...)` in any part.
mod e1;
mod e2;
mod e3;
mod e4;
mod selftest;
pub use e1::*;
pub use e2::*;
pub use e3::*;
pub use e4::*;

/// `evidence.selftest()` — ported by the selftest owner.
pub fn selftest() -> Vec<String> {
    selftest::selftest()
}
