//! The 107 P1 rows of `scan_node.py` against the saved raw of the accepted Original run: every row's arguments (as the original
//! actually received them), the value or exception it actually produced, and the local hour of each clock row's own recorded bracket.
//! The golden (`data/scan_node/p1_golden.json`) is derived from that saved raw by a pure format conversion (no original call, no new
//! capture); the checks here run (through `scan_node_selfcheck`, the same function the program's `--p1-self-check` runs) each row through the private bridge exactly as the Python adapter does (`serve`) over fixtures laid
//! out from the bytes the control declared. A clock row's hour is a literal taken from its bracket, never the current time.
//!
//! Identity is part of the contract: the importer numbers the caller's containers and NaN floats, the bridge hands back the *same*
//! shared objects, and the answer plan names them (`argref`). The golden's expected plan comes from the raw graph refs of the saved
//! run (the six `cross_validate` rows return `ic["hca_devices"]` itself); the two rows with only an argument NaN return no alias.
//!
//! Not checked here, stated plainly: traceback frames of raised exceptions; the fresh adapter capture (owned by the parent Root after
//! the source freeze).

use agent_runtime_support::pyvalue::PyValue;
use sweep_stop::scan_node_selfcheck::{
    get, items, parse, run_all, run_golden, text_of, OwnerUid, GOLDEN,
};

#[test]
fn rows_run_as_the_original_actually_answered() {
    // the unreadable-file rows need an ordinary user: root reads mode-000 files
    assert_ne!(
        std::fs::metadata("/proc/self")
            .expect("proc")
            .uid_of_owner(),
        0,
        "run these checks as an ordinary user (a mode-000 file is readable by root)"
    );
    let outcome = run_all();
    assert!(
        outcome.problems.is_empty(),
        "{} problem(s):\n{}",
        outcome.problems.len(),
        outcome.problems.join("\n")
    );
    assert_eq!(outcome.executed, 107);
    assert_eq!(outcome.returned, 105);
    assert_eq!(outcome.raised, 2);
    assert_eq!(outcome.by_plane.get("pure"), Some(&88));
    assert_eq!(outcome.by_plane.get("clock"), Some(&4));
    assert_eq!(outcome.by_plane.get("file-fixture"), Some(&15));
    // the six rows whose raw graph returns the caller's own `ic["hca_devices"]`; no other row returns a caller object
    assert_eq!(outcome.argref_rows, 6);
}

#[test]
fn golden_is_the_whole_closed_set_with_identity_classified() {
    let golden = parse(GOLDEN);
    let rows = items(get(&golden, "rows"));
    assert_eq!(rows.len(), 107);
    let ids: std::collections::BTreeSet<String> =
        rows.iter().map(|row| text_of(get(row, "id"))).collect();
    assert_eq!(ids.len(), 107, "row ids are unique");
    let count = |key: &str| {
        rows.iter()
            .filter(|row| match get(get(row, "identity"), key).peel() {
                PyValue::List(entries) => !entries.is_empty(),
                _ => false,
            })
            .count()
    };
    // raw refs of the saved run: six rows return the caller's own list; two rows only carry an argument NaN (no returned alias)
    assert_eq!(count("returned_argrefs"), 6);
    assert_eq!(count("arg_nan_identity"), 2);
    let aliasing: Vec<String> = rows
        .iter()
        .filter(|row| matches!(get(get(row, "identity"), "returned_argrefs").peel(), PyValue::List(e) if !e.is_empty()))
        .map(|row| text_of(get(row, "id")))
        .collect();
    assert_eq!(
        aliasing,
        ["S50N01", "S50N02", "S50N03", "S50N05", "S50N06", "S50N07"]
    );
}

/// Negative control of the harness itself: a golden whose expectations are changed must be reported, row by row (so a pass above
/// is a comparison, not a loop that cannot fail).
#[test]
fn harness_reports_a_changed_expectation() {
    let mutated = GOLDEN
        .replace("\"alpha\"", "\"alphX\"")
        .replacen("\"+d\"", "\"+e\"", 1);
    let outcome = run_golden(&mutated);
    assert!(outcome.problems.len() >= 2, "{:#?}", outcome.problems);
    assert!(
        outcome
            .problems
            .iter()
            .any(|p| p.contains("value or identity plan differs")),
        "{:#?}",
        outcome.problems
    );
    assert_eq!(
        outcome.executed, 107,
        "the mutated golden still runs every row"
    );
}
