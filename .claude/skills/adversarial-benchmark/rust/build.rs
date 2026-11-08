//! Reads the full-definition repeat floor from the campaign schema so the crate never hard-codes it.
//!
//! The Python original imports `repeat_axis.FULL_REPEATS_MIN`, which is cross-checked against this same schema
//! value (`definitions.budgets.properties.repeats.minimum`). The schema is read at build time and the value is
//! compiled in; nothing is copied into the build output.
//!
//! Source root: `EASY_VLLM_SOURCE_ROOT` when set (prototype builds), otherwise the fourth ancestor of this crate
//! (`.claude/skills/adversarial-benchmark/rust` -> repository root).

use std::env;
use std::fs;
use std::path::PathBuf;

const SCHEMA_RELATIVE_PATH: &str = "campaigns/_template/campaign.schema.json";
const SOURCE_ROOT_ENV: &str = "EASY_VLLM_SOURCE_ROOT";

fn main() {
    println!("cargo:rerun-if-env-changed={SOURCE_ROOT_ENV}");
    println!("cargo:rerun-if-changed=build.rs");

    let schema_path = source_root().join(SCHEMA_RELATIVE_PATH);
    println!("cargo:rerun-if-changed={}", schema_path.display());

    let text = fs::read_to_string(&schema_path).unwrap_or_else(|err| {
        panic!(
            "cannot read campaign schema {}: {err}",
            schema_path.display()
        )
    });
    let schema: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|err| {
        panic!(
            "campaign schema {} is not JSON: {err}",
            schema_path.display()
        )
    });
    let floor = schema
        .pointer("/definitions/budgets/properties/repeats/minimum")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| {
            panic!(
                "{} has no integer definitions.budgets.properties.repeats.minimum",
                schema_path.display()
            )
        });

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let generated = format!(
        "/// Full-definition repeat floor, read from `{SCHEMA_RELATIVE_PATH}` at build time.\n\
         pub const FULL_REPEATS_MIN: u32 = {floor};\n"
    );
    fs::write(out_dir.join("full_repeats_min.rs"), generated)
        .expect("write generated repeat floor");
}

fn source_root() -> PathBuf {
    if let Some(root) = env::var_os(SOURCE_ROOT_ENV) {
        return PathBuf::from(root);
    }
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(4)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "{} has no fourth ancestor; set {SOURCE_ROOT_ENV}",
                manifest_dir.display()
            )
        })
}
