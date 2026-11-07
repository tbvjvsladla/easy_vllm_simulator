//! The business function of the component: `Requirement(value)` as `resolve_torch_pin.main` uses it, observed as the projection
//! `{accepted, name, specifier_text, specifiers}` plus what the constructor wrote to standard error.

use super::grammar::parse_requirement;
use super::specifier::SpecifierSet;
use super::{Rejection, UnsupportedDomain};
use crate::pyhash::HashSecret;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The scope string of every record this component writes.
pub const SCOPE: &str = "requirement-main-projection-packaging24";

/// The hash profiles supported here: `PYTHONHASHSEED=0` and `PYTHONHASHSEED=17`, the two explicit test profiles of the saved originals.
pub const SUPPORTED_HASH_SEEDS: [u32; 2] = [0, 17];

/// What `main` reads from an accepted requirement.
pub struct AcceptedRequirement {
    /// `req.name`, as written.
    pub name: PyStr,
    /// `str(req.specifier)`.
    pub specifier_text: PyStr,
    /// `[(s.operator, s.version) for s in req.specifier]`: in the iteration order of the specifier set.
    pub specifiers: Vec<(PyStr, PyStr)>,
}

/// The projection of one constructor call.
pub enum Projection {
    Accepted(AcceptedRequirement),
    /// The constructor raised an `Exception`.
    Rejected,
}

/// The projection and the text written to standard error while constructing.
pub struct Observation {
    pub projection: Projection,
    /// One `SyntaxWarning` line per marker literal with an invalid escape, in parse order; empty for a silent construction.
    pub stderr: String,
}

fn text(points: Vec<u32>) -> PyStr {
    PyStr::from_code_points(points)
}

/// The stack of the thread that runs the construction. The pattern matcher recurses once per iteration of a repeated group (`(\.[0-9]+)*`), so the
/// length of the input, not the stack of the caller, must be what bounds it; Python's `re` has no such limit.
const CONSTRUCTION_STACK_BYTES: usize = 1 << 30;

/// `Requirement(value)` under the hash profile of `secret`. `Err` is a domain this component does not model (a refusal, not a rejection).
pub fn project_requirement(
    value: &PyValue,
    secret: &HashSecret,
) -> Result<Observation, UnsupportedDomain> {
    let PyValue::Str(source) = value.peel() else {
        // the argument is passed to `re.match`, which raises `TypeError` for anything but a str: an Exception `main` skips
        return Ok(Observation {
            projection: Projection::Rejected,
            stderr: String::new(),
        });
    };
    let points = source.code_points();
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(CONSTRUCTION_STACK_BYTES)
            .spawn_scoped(scope, || construct(points, secret))
            .map_err(|_| UnsupportedDomain::ConstructionStackUnavailable)?
            .join()
            .map_err(|_| UnsupportedDomain::ConstructionStackUnavailable)?
    })
}

/// The constructor on the source text of a `str`.
fn construct(source: &[u32], secret: &HashSecret) -> Result<Observation, UnsupportedDomain> {
    let rejected = |stderr: String| {
        Ok(Observation {
            projection: Projection::Rejected,
            stderr,
        })
    };
    let outcome = parse_requirement(source);
    let stderr: String = outcome.warnings.concat();
    let parsed = match outcome.result {
        Ok(parsed) => parsed,
        Err(Rejection::Unsupported(domain)) => return Err(domain),
        Err(_) => return rejected(stderr),
    };
    let set = match SpecifierSet::parse(&parsed.specifier, secret) {
        Ok(set) => set,
        Err(Rejection::Unsupported(domain)) => return Err(domain),
        Err(_) => return rejected(stderr),
    };
    let specifiers = set
        .iter()
        .map(|specifier| {
            (
                text(specifier.operator().to_vec()),
                text(specifier.version().to_vec()),
            )
        })
        .collect();
    Ok(Observation {
        projection: Projection::Accepted(AcceptedRequirement {
            name: text(parsed.name),
            specifier_text: text(set.text()),
            specifiers,
        }),
        stderr,
    })
}

impl Projection {
    /// The captured typed value: `{accepted: False}`, or `{accepted: True, name, specifier_text, specifiers: [(operator, version), ...]}`.
    pub fn to_value(&self) -> PyValue {
        let mut fields = PyDict::new();
        match self {
            Projection::Rejected => fields.set("accepted", PyValue::Bool(false)),
            Projection::Accepted(accepted) => {
                fields.set("accepted", PyValue::Bool(true));
                fields.set("name", PyValue::Str(accepted.name.clone()));
                fields.set(
                    "specifier_text",
                    PyValue::Str(accepted.specifier_text.clone()),
                );
                fields.set(
                    "specifiers",
                    PyValue::List(
                        accepted
                            .specifiers
                            .iter()
                            .map(|(operator, version)| {
                                PyValue::Tuple(vec![
                                    PyValue::Str(operator.clone()),
                                    PyValue::Str(version.clone()),
                                ])
                            })
                            .collect(),
                    ),
                );
            }
        }
        PyValue::Dict(fields)
    }
}

/// The hash profile of an explicit seed, `None` for a seed outside [`SUPPORTED_HASH_SEEDS`].
pub fn secret_for_seed(seed: u32) -> Option<HashSecret> {
    SUPPORTED_HASH_SEEDS
        .contains(&seed)
        .then(|| HashSecret::from_seed(seed))
}
