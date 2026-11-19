//! Every CLI case under checks/data/cli run through the Rust launchers: stdout, stderr and exit code must be
//! byte-identical to the recording (a capture of the Python original, or a Rust launcher recording for the cases changed
//! or added since the date rules were removed). The launchers are pointed at the
//! binaries cargo built for this check (`POLICY_REGISTRY_BIN`, `HARNESS_VERIFY_BIN`). No Python runs here.

use std::path::Path;
use std::process::Command;

#[test]
fn cli_cases_match_the_original() {
    let checks = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks");
    let data = checks.join("data/cli");
    let mut ids: Vec<String> = std::fs::read_dir(&data).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    ids.sort();
    let only = std::env::var("PA_CASE").ok();
    let scratch = std::env::temp_dir().join(format!("policy-audit-cli-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let mut failures = Vec::new();
    let mut ran = 0;
    for id in &ids {
        if let Some(o) = &only {
            if !id.contains(o.as_str()) {
                continue;
            }
        }
        let case = data.join(id);
        let out = scratch.join(id);
        std::fs::create_dir_all(&out).unwrap();
        let st = Command::new("bash")
            .arg(checks.join("run_case.sh"))
            .arg(&case)
            .arg(&out)
            .arg("rust")
            .env("POLICY_REGISTRY_BIN", env!("CARGO_BIN_EXE_policy_registry"))
            .env("HARNESS_VERIFY_BIN", env!("CARGO_BIN_EXE_harness_verify"))
            .status()
            .unwrap();
        assert!(st.success(), "runner failed for {id}");
        ran += 1;
        for ch in ["stdout", "stderr", "exit_code"] {
            let want = std::fs::read(case.join(ch)).unwrap_or_default();
            let got = std::fs::read(out.join(ch)).unwrap_or_default();
            if want != got {
                failures.push(format!(
                    "{id}/{ch}:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want),
                    String::from_utf8_lossy(&got)
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(ran > 0);
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}
