//! Tripwire ④'s key = `completion_gate.certificate_run_key(parse_flat_certificate(text))` from its single owner.
use policy_gate::completion_gate::{certificate_run_key, parse_flat_certificate};

pub use policy_gate::completion_gate::CERTIFICATE_RUN_KEY_FIELDS;

/// The run key, or None when the flat parse fails or a key field is absent/blank (unidentifiable).
pub fn certificate_key(text: &str) -> Option<Vec<String>> {
    let (fields, ok) = parse_flat_certificate(text);
    if !ok {
        return None;
    }
    certificate_run_key(&fields)
}
