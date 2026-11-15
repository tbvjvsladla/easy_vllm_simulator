//! `agent-runtime-support`: the Python-compatible value, text, number, JSON and YAML semantics that several owner
//! packages need, as plain typed data and functions. Nothing here knows a skill, a node, a topology or a serving rule,
//! starts a process, opens a network connection or reads a credential; the YAML reader takes the bytes its caller read.
//!
//! The owner packages re-export these modules under their old paths (`pystr`, `pyvalue`, ...) so that no business type
//! path changed and every type stays one nominal type.

pub mod failure;
pub mod pycall;
pub mod pydyn;
pub mod pyformat;
pub mod pyjson;
pub mod pylower;
pub mod pynumber;
pub mod pyops;
pub mod pysort;
pub mod pystr;
pub mod schema;
pub mod pyvalue;
pub mod pyyaml;
mod unicode_alnum_table;
mod unicode_lower_table;
mod unicode_tables;
