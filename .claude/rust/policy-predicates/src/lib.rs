//! `policy-predicates`: the Rust port of `.claude/policies/predicates/*.py`.
#![allow(non_snake_case)]
pub mod claims;
pub mod claims_lib;
pub mod claims_owner;
pub mod claims_text;
pub mod claims_hint;
pub mod claims_recipe;
pub mod claims_selftest;
pub mod claims_tf;
pub mod claims_upstream;
pub mod companion;
pub mod runtime_patch;
pub mod support;
pub mod unit;
