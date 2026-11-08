//! `packaging.specifiers.Specifier` and `SpecifierSet` of packaging 24.0 as the requirement constructor uses them: the specifier pattern, the
//! operator/version split of one specifier, its canonical form (the equality and hash key) and the `frozenset` the set keeps its specifiers in.
//!
//! The pattern texts below are copied verbatim from `packaging/specifiers.py` (`Specifier._operator_regex_str`, `Specifier._version_regex_str`) and
//! compiled by [`super::pyre`] with `re.VERBOSE | re.IGNORECASE`, so the matching is the original expression's, not a rewrite of it.
//! Adapted from packaging 24.0 (dual licensed Apache-2.0 / BSD-2-Clause, see `notices/`).

use super::pyre::Pattern;
use super::Rejection;
use crate::pep440::{Refusal, Version};
use crate::pyhash::{frozenset_iteration_order, hash_str, hash_tuple, HashSecret, PyHash};
use agent_runtime_support::pystr::{is_python_space, PyStr};
use std::sync::LazyLock;

/// `Specifier._operator_regex_str`.
pub(super) const OPERATOR_REGEX: &str = r#"
    (?P<operator>(~=|==|!=|<=|>=|<|>|===))
    "#;

/// `Specifier._version_regex_str`.
pub(super) const VERSION_REGEX: &str = r#"
    (?P<version>
        (?:
            # The identity operators allow for an escape hatch that will
            # do an exact string match of the version you wish to install.
            # This will not be parsed by PEP 440 and we cannot determine
            # any semantic meaning from it. This operator is discouraged
            # but included entirely as an escape hatch.
            (?<====)  # Only match for the identity operator
            \s*
            [^\s;)]*  # The arbitrary version can be just about anything,
                      # we match everything except for whitespace, a
                      # semi-colon for marker support, and a closing paren
                      # since versions can be enclosed in them.
        )
        |
        (?:
            # The (non)equality operators allow for wild card and local
            # versions to be specified so we have to define these two
            # operators separately to enable that.
            (?<===|!=)            # Only match for equals and not equals

            \s*
            v?
            (?:[0-9]+!)?          # epoch
            [0-9]+(?:\.[0-9]+)*   # release

            # You cannot use a wild card and a pre-release, post-release, a dev or
            # local version together so group them with a | and make them optional.
            (?:
                \.\*  # Wild card syntax of .*
                |
                (?:                                  # pre release
                    [-_\.]?
                    (alpha|beta|preview|pre|a|b|c|rc)
                    [-_\.]?
                    [0-9]*
                )?
                (?:                                  # post release
                    (?:-[0-9]+)|(?:[-_\.]?(post|rev|r)[-_\.]?[0-9]*)
                )?
                (?:[-_\.]?dev[-_\.]?[0-9]*)?         # dev release
                (?:\+[a-z0-9]+(?:[-_\.][a-z0-9]+)*)? # local
            )?
        )
        |
        (?:
            # The compatible operator requires at least two digits in the
            # release segment.
            (?<=~=)               # Only match for the compatible operator

            \s*
            v?
            (?:[0-9]+!)?          # epoch
            [0-9]+(?:\.[0-9]+)+   # release  (We have a + instead of a *)
            (?:                   # pre release
                [-_\.]?
                (alpha|beta|preview|pre|a|b|c|rc)
                [-_\.]?
                [0-9]*
            )?
            (?:                                   # post release
                (?:-[0-9]+)|(?:[-_\.]?(post|rev|r)[-_\.]?[0-9]*)
            )?
            (?:[-_\.]?dev[-_\.]?[0-9]*)?          # dev release
        )
        |
        (?:
            # All other operators only allow a sub set of what the
            # (non)equality operators do. Specifically they do not allow
            # local versions to be specified nor do they allow the prefix
            # matching wild cards.
            (?<!==|!=|~=)         # We have special cases for these
                                  # operators so we want to make sure they
                                  # don't match here.

            \s*
            v?
            (?:[0-9]+!)?          # epoch
            [0-9]+(?:\.[0-9]+)*   # release
            (?:                   # pre release
                [-_\.]?
                (alpha|beta|preview|pre|a|b|c|rc)
                [-_\.]?
                [0-9]*
            )?
            (?:                                   # post release
                (?:-[0-9]+)|(?:[-_\.]?(post|rev|r)[-_\.]?[0-9]*)
            )?
            (?:[-_\.]?dev[-_\.]?[0-9]*)?          # dev release
        )
    )
    "#;

/// `Specifier._regex`: `^\s*` + operator + version + `\s*$`, `re.VERBOSE | re.IGNORECASE`.
static SPECIFIER_REGEX: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::compile(
        &format!(r"^\s*{OPERATOR_REGEX}{VERSION_REGEX}\s*$"),
        true,
        true,
    )
    .unwrap_or_else(|error| panic!("specifier pattern: {error}"))
});

/// `str.strip()` of a code-point slice.
fn strip(text: &[u32]) -> &[u32] {
    let start = text
        .iter()
        .position(|&c| !is_python_space(c))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|&c| !is_python_space(c))
        .map_or(start, |last| last + 1);
    &text[start..end]
}

/// One `Specifier`: the operator and the stripped version text as the pattern captured them, and the `_canonical_spec` that decides its
/// equality and hash.
pub(super) struct Specifier {
    operator: Vec<u32>,
    version: Vec<u32>,
    canonical_version: Vec<u32>,
}

/// `Specifier.__eq__` for two specifiers: equal `_canonical_spec`.
impl PartialEq for Specifier {
    fn eq(&self, other: &Specifier) -> bool {
        self.operator == other.operator && self.canonical_version == other.canonical_version
    }
}

/// `canonicalize_version(version, strip_trailing_zero=...)` of a version text: text that is not a version stays as it is, an integer conversion
/// failure (`ValueError`, not `InvalidVersion`) escapes.
fn canonicalize_version(version: &[u32], strip_trailing_zero: bool) -> Result<Vec<u32>, Rejection> {
    match Version::parse(version) {
        Ok(parsed) => Ok(parsed.canonical_text(strip_trailing_zero)),
        Err(Refusal::Invalid) => Ok(version.to_vec()),
        Err(Refusal::Thrown(_)) => Err(Rejection::VersionIntegerLimit),
    }
}

impl Specifier {
    /// `Specifier(spec)` followed by the evaluation of its `_canonical_spec` (which `hash()` forces when the specifier enters a `frozenset`).
    pub(super) fn parse(spec: &[u32]) -> Result<Specifier, Rejection> {
        let found = SPECIFIER_REGEX
            .search(spec)
            .ok_or(Rejection::InvalidSpecifier)?;
        let span = |name: &str| {
            let (start, end) = found
                .named(name)
                .expect("the specifier pattern captures the operator and the version");
            strip(&spec[start..end]).to_vec()
        };
        let operator = span("operator");
        let version = span("version");
        let strip_trailing_zero = operator != [u32::from(b'~'), u32::from(b'=')];
        let canonical_version = canonicalize_version(&version, strip_trailing_zero)?;
        Ok(Specifier {
            operator,
            version,
            canonical_version,
        })
    }

    /// `hash(self._canonical_spec)`.
    fn hash(&self, secret: &HashSecret) -> PyHash {
        hash_tuple(&[
            hash_str(&PyStr::from_code_points(self.operator.clone()), secret),
            hash_str(
                &PyStr::from_code_points(self.canonical_version.clone()),
                secret,
            ),
        ])
    }

    pub(super) fn operator(&self) -> &[u32] {
        &self.operator
    }

    pub(super) fn version(&self) -> &[u32] {
        &self.version
    }

    /// `str(specifier)`: the operator followed by the version.
    fn text(&self) -> Vec<u32> {
        let mut text = self.operator.clone();
        text.extend_from_slice(&self.version);
        text
    }
}

/// A `SpecifierSet`: its specifiers in the order iterating its `frozenset` yields them (equal specifiers appear once, with the spelling of the
/// first).
pub(super) struct SpecifierSet {
    in_iteration_order: Vec<Specifier>,
}

impl SpecifierSet {
    /// `SpecifierSet(specifiers)` under the hash secret of the process.
    pub(super) fn parse(
        specifiers: &[u32],
        secret: &HashSecret,
    ) -> Result<SpecifierSet, Rejection> {
        let comma = u32::from(b',');
        let mut produced: Vec<(PyHash, Specifier)> = Vec::new();
        for piece in specifiers.split(|&c| c == comma) {
            let piece = strip(piece);
            if piece.is_empty() {
                continue;
            }
            let specifier = Specifier::parse(piece)?;
            produced.push((specifier.hash(secret), specifier));
        }
        let order = frozenset_iteration_order(&produced);
        let mut slots: Vec<Option<Specifier>> = produced
            .into_iter()
            .map(|(_, specifier)| Some(specifier))
            .collect();
        let in_iteration_order = order
            .into_iter()
            .map(|index| {
                slots[index]
                    .take()
                    .expect("an iteration order names each retained specifier once")
            })
            .collect();
        Ok(SpecifierSet { in_iteration_order })
    }

    /// Iterating the set: the specifiers in `frozenset` slot order.
    pub(super) fn iter(&self) -> impl Iterator<Item = &Specifier> {
        self.in_iteration_order.iter()
    }

    /// `str(specifier_set)`: `",".join(sorted(str(s) for s in specs))`, sorted by code point.
    pub(super) fn text(&self) -> Vec<u32> {
        let mut texts: Vec<Vec<u32>> = self.iter().map(Specifier::text).collect();
        texts.sort();
        let mut joined = Vec::new();
        for (at, text) in texts.iter().enumerate() {
            if at > 0 {
                joined.push(u32::from(b','));
            }
            joined.extend_from_slice(text);
        }
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).collect()
    }

    fn set(text: &str, seed: u32) -> SpecifierSet {
        SpecifierSet::parse(&points(text), &HashSecret::from_seed(seed))
            .ok()
            .unwrap()
    }

    #[test]
    fn a_specifier_splits_into_operator_and_stripped_version() {
        let specifier = Specifier::parse(&points("  ==  1.0 ")).ok().unwrap();
        assert_eq!(specifier.operator(), points("==").as_slice());
        assert_eq!(specifier.version(), points("1.0").as_slice());
    }

    #[test]
    fn equal_canonical_specifiers_keep_the_first_spelling() {
        let kept = set("==1.0,==1.0.0", 0);
        assert_eq!(kept.iter().count(), 1);
        assert_eq!(kept.text(), points("==1.0"));
        assert_eq!(set("==1.0.0,==1.0", 0).text(), points("==1.0.0"));
    }

    #[test]
    fn arbitrary_equality_is_not_equality() {
        assert_eq!(set("===1.0,==1.0", 0).iter().count(), 2);
    }

    #[test]
    fn compatible_release_keeps_its_trailing_zero() {
        assert_eq!(set("~=1.0,~=1.0.0", 0).iter().count(), 2);
    }

    #[test]
    fn text_is_sorted_by_code_point() {
        assert_eq!(set("<=4,>=1,!=2", 0).text(), points("!=2,<=4,>=1"));
    }

    #[test]
    fn a_text_that_is_not_a_specifier_is_refused() {
        assert!(Specifier::parse(&points("lolwat")).is_err());
        assert!(Specifier::parse(&points("~=2")).is_err());
    }
}
