//! The asset selection of `resolve_wheel.py` `main` (lines 33-85): the release document's asset names become wheel candidates, and the requested
//! CUDA text (or none) picks one, or the report of why none was picked.
//!
//! The original matches names with two regular expressions built from the escaped version and architecture:
//!
//! ```text
//! ^vllm-<version>\+cu(\d+)-cp\d+-abi3-(manylinux_\d+_\d+)_<arch>\.whl$      (a "+cuNNN" variant)
//! ^vllm-<version>-cp\d+-abi3-(manylinux_\d+_\d+)_<arch>\.whl$               (the release default: no CUDA digits in the name)
//! ```
//!
//! `\d` is any Unicode decimal digit (category Nd) and the `$` also accepts one final line feed, so a name that ends in `.whl\n` matches. The
//! digit runs are separated by literals no digit can match, so each match is decided by one left-to-right pass without backtracking. Every
//! quirk of the original is kept as it is: candidates keep their input order and duplicates; the automatic choice is the LAST of the greatest
//! numeric values; an explicit choice is the FIRST candidate whose digit text is equal (so `0129` is not `129`); the neighbours of a missing
//! explicit choice are sorted stably by distance; `str.isdigit` and `int()` accept different characters; the default-variant list is reported
//! beside the candidates under `default_variants`, while `all_arch_assets` holds only the `+cuNNN` candidates; `browser_download_url` is never
//! read. Each failure of the original's Python code is returned as the exception it raises, with the place it was raised from.

use crate::http_direct::failure::ErrorValue;
use crate::http_direct::PyException;
use agent_runtime_support::pynumber::int_from_str;
use agent_runtime_support::pystr::{is_decimal_digit, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::Signed;

/// The place in the original script (or its comprehension and generator frames) an exception was raised from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaiseSite {
    /// `rel.get` of line 34: the release document has no `get`.
    ReleaseGet,
    /// The comprehension's `for x in rel.get("assets", [])` over a value that cannot be iterated.
    AssetsIteration,
    /// `x["name"]` of line 34.
    AssetName,
    /// `pat_cu.match(name)` of line 45 (the same call raises for either pattern; only the first can raise, as it runs first).
    NameMatch,
    /// `int(req[:-1])` of line 67.
    RequestedMajor,
    /// `int(req[-1])` of line 67.
    RequestedMinor,
    /// `int(c["cuda"][:-1])` inside the generator of lines 68-72.
    NearestGenerator,
    /// `int(c["cuda"])` inside the lambda of line 83.
    HighestKey,
}

/// An exception the script's Python code raises.
#[derive(Clone, Debug)]
pub struct Raised {
    pub error: Box<PyException>,
    pub site: RaiseSite,
}

/// A wheel asset the patterns matched.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub name: PyStr,
    /// The digits after `+cu` (any Unicode decimal digits, as text); `None` for a default-variant wheel.
    pub cuda: Option<PyStr>,
    pub manylinux: PyStr,
    /// The fixed label of a default-variant wheel.
    pub variant: Option<&'static str>,
}

const DEFAULT_VARIANT_LABEL: &str = "default(release-CUDA, 파일명에 cuXXX 없음)";

impl Candidate {
    /// The dictionary the original builds for it, with its keys in the original order.
    pub fn to_value(&self) -> PyValue {
        let mut row = PyDict::new();
        row.set("name", PyValue::Str(self.name.clone()));
        row.set(
            "cuda",
            self.cuda.clone().map_or(PyValue::None, PyValue::Str),
        );
        row.set("manylinux", PyValue::Str(self.manylinux.clone()));
        if let Some(label) = self.variant {
            row.set("variant", PyValue::str(label));
        }
        PyValue::Dict(row)
    }
}

/// How the selection ended.
#[derive(Clone, Debug)]
pub enum Selection {
    /// No `+cu` and no default wheel for the architecture: the error report of exit status 4.
    NoWheelAssets { assets: Vec<PyStr> },
    /// An explicit CUDA text without an exact candidate: the report of exit status 5.
    NoExactCuda {
        requested: PyStr,
        available: Vec<Candidate>,
        default_variants: Vec<Candidate>,
        nearest: Vec<Candidate>,
    },
    /// The chosen wheel and what it was chosen from.
    Chosen {
        chosen: Candidate,
        candidates: Vec<Candidate>,
        default_variants: Vec<Candidate>,
    },
}

fn plain(class: &'static str, message: &str) -> PyException {
    PyException {
        class,
        module: "builtins",
        message: PyStr::new(message),
        args: vec![ErrorValue::Plain(PyValue::str(message))],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    }
}

fn raised(class: &'static str, message: &str, site: RaiseSite) -> Raised {
    Raised {
        error: Box::new(plain(class, message)),
        site,
    }
}

fn type_name(value: &PyValue) -> &'static str {
    value.type_name()
}

/// `rel.get("assets", [])`: the value of the key, or an empty list; a document that is not an object has no `get`.
fn assets_value(release: &PyValue) -> Result<PyValue, Raised> {
    match release {
        PyValue::Dict(members) => Ok(members
            .get("assets")
            .cloned()
            .unwrap_or_else(|| PyValue::List(Vec::new()))),
        other => {
            let message = format!("'{}' object has no attribute 'get'", type_name(other));
            let mut error = plain("AttributeError", &message);
            error.attributes = vec![
                ("name", PyValue::str("get").into()),
                ("obj", ErrorValue::Plain(other.clone())),
            ];
            Err(Raised {
                error: Box::new(error),
                site: RaiseSite::ReleaseGet,
            })
        }
    }
}

/// The elements `for x in value` yields: list items, dictionary keys, string characters; anything else cannot be iterated.
fn iterate(value: &PyValue) -> Result<Vec<PyValue>, Raised> {
    match value {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(members) => Ok(members
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        other => Err(raised(
            "TypeError",
            &format!("'{}' object is not iterable", type_name(other)),
            RaiseSite::AssetsIteration,
        )),
    }
}

/// `x["name"]`.
fn name_of(asset: &PyValue) -> Result<PyValue, Raised> {
    let site = RaiseSite::AssetName;
    match asset {
        PyValue::Dict(members) => match members.get("name") {
            Some(name) => Ok(name.clone()),
            None => {
                let mut error = plain("KeyError", "'name'");
                error.args = vec![ErrorValue::Plain(PyValue::str("name"))];
                Err(Raised {
                    error: Box::new(error),
                    site,
                })
            }
        },
        PyValue::Str(_) => Err(raised(
            "TypeError",
            "string indices must be integers, not 'str'",
            site,
        )),
        PyValue::List(_) => Err(raised(
            "TypeError",
            "list indices must be integers or slices, not str",
            site,
        )),
        PyValue::Tuple(_) => Err(raised(
            "TypeError",
            "tuple indices must be integers or slices, not str",
            site,
        )),
        other => Err(raised(
            "TypeError",
            &format!("'{}' object is not subscriptable", type_name(other)),
            site,
        )),
    }
}

/// The names of `[x["name"] for x in rel.get("assets", [])]`, as the comprehension builds them (values of any type).
pub fn asset_names(release: &PyValue) -> Result<Vec<PyValue>, Raised> {
    let listed = assets_value(release)?;
    iterate(&listed)?.iter().map(name_of).collect()
}

/// The literal and digit steps of the two patterns over one name.
struct NameScan<'a> {
    name: &'a [u32],
    at: usize,
}

impl NameScan<'_> {
    fn literal(&mut self, text: &[u32]) -> bool {
        let ends = self.at + text.len();
        if ends <= self.name.len() && self.name[self.at..ends] == *text {
            self.at = ends;
            return true;
        }
        false
    }

    fn digits(&mut self) -> Option<std::ops::Range<usize>> {
        let begin = self.at;
        while self
            .name
            .get(self.at)
            .is_some_and(|&point| is_decimal_digit(point))
        {
            self.at += 1;
        }
        (self.at > begin).then_some(begin..self.at)
    }

    /// `$`: the end of the name, or the position before one final line feed.
    fn at_end(&self) -> bool {
        self.at == self.name.len()
            || (self.at + 1 == self.name.len() && self.name[self.at] == u32::from('\n'))
    }
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// The two compiled patterns of one run: the escaped version and architecture are matched as literal text.
struct WheelPatterns {
    version: Vec<u32>,
    arch: Vec<u32>,
}

/// What a pattern captured of a name.
struct Matched {
    cuda: Option<Range>,
    manylinux: Range,
}

type Range = std::ops::Range<usize>;

impl WheelPatterns {
    /// `(manylinux_\d+_\d+)_<arch>\.whl$` after the `-abi3-` literal: the capture, or no match.
    fn manylinux_tail(&self, scan: &mut NameScan<'_>) -> Option<Range> {
        let begin = scan.at;
        scan.literal(&points("manylinux_")).then_some(())?;
        scan.digits()?;
        scan.literal(&points("_")).then_some(())?;
        scan.digits()?;
        let capture = begin..scan.at;
        scan.literal(&points("_")).then_some(())?;
        scan.literal(&self.arch).then_some(())?;
        scan.literal(&points(".whl")).then_some(())?;
        scan.at_end().then_some(capture)
    }

    /// `^vllm-<version>\+cu(\d+)-cp\d+-abi3-(manylinux...)...$`.
    fn match_with_cuda(&self, name: &[u32]) -> Option<Matched> {
        let mut scan = NameScan { name, at: 0 };
        scan.literal(&points("vllm-")).then_some(())?;
        scan.literal(&self.version).then_some(())?;
        scan.literal(&points("+cu")).then_some(())?;
        let cuda = scan.digits()?;
        scan.literal(&points("-cp")).then_some(())?;
        scan.digits()?;
        scan.literal(&points("-abi3-")).then_some(())?;
        let manylinux = self.manylinux_tail(&mut scan)?;
        Some(Matched {
            cuda: Some(cuda),
            manylinux,
        })
    }

    /// `^vllm-<version>-cp\d+-abi3-(manylinux...)...$`.
    fn match_default(&self, name: &[u32]) -> Option<Matched> {
        let mut scan = NameScan { name, at: 0 };
        scan.literal(&points("vllm-")).then_some(())?;
        scan.literal(&self.version).then_some(())?;
        scan.literal(&points("-cp")).then_some(())?;
        scan.digits()?;
        scan.literal(&points("-abi3-")).then_some(())?;
        let manylinux = self.manylinux_tail(&mut scan)?;
        Some(Matched {
            cuda: None,
            manylinux,
        })
    }
}

fn slice_text(name: &[u32], range: &Range) -> PyStr {
    PyStr::from_code_points(name[range.clone()].to_vec())
}

/// The loop of lines 44-52: the `+cu` candidates and the default-variant candidates, in the order of the names.
fn classify(
    names: &[PyValue],
    patterns: &WheelPatterns,
) -> Result<(Vec<Candidate>, Vec<Candidate>), Raised> {
    let (mut candidates, mut defaults) = (Vec::new(), Vec::new());
    for entry in names {
        let PyValue::Str(name) = entry else {
            let message = format!(
                "expected string or bytes-like object, got '{}'",
                type_name(entry)
            );
            return Err(raised("TypeError", &message, RaiseSite::NameMatch));
        };
        let text = name.code_points();
        if let Some(found) = patterns.match_with_cuda(text) {
            candidates.push(Candidate {
                name: name.clone(),
                cuda: found.cuda.as_ref().map(|range| slice_text(text, range)),
                manylinux: slice_text(text, &found.manylinux),
                variant: None,
            });
            continue;
        }
        if let Some(found) = patterns.match_default(text) {
            defaults.push(Candidate {
                name: name.clone(),
                cuda: None,
                manylinux: slice_text(text, &found.manylinux),
                variant: Some(DEFAULT_VARIANT_LABEL),
            });
        }
    }
    Ok((candidates, defaults))
}

/// `str.isdigit()`: a decimal digit, or a character whose numeric type is "digit" (superscripts, circled and parenthesized numbers, ...).
///
/// The extra ranges are the characters of Unicode 15.0 with the property Numeric_Type=Digit that are not decimal digits (category Nd is
/// [`is_decimal_digit`]). They were written from that property listing, not generated from the reference interpreter's tables.
pub fn is_python_digit(point: u32) -> bool {
    const NON_DECIMAL_DIGITS: [(u32, u32); 20] = [
        (0xB2, 0xB3),
        (0xB9, 0xB9),
        (0x1369, 0x1371),
        (0x19DA, 0x19DA),
        (0x2070, 0x2070),
        (0x2074, 0x2079),
        (0x2080, 0x2089),
        (0x2460, 0x2468),
        (0x2474, 0x247C),
        (0x2488, 0x2490),
        (0x24EA, 0x24EA),
        (0x24F5, 0x24FD),
        (0x24FF, 0x24FF),
        (0x2776, 0x277E),
        (0x2780, 0x2788),
        (0x278A, 0x2792),
        (0x10A40, 0x10A43),
        (0x10E60, 0x10E68),
        (0x11052, 0x1105A),
        (0x1F100, 0x1F10A),
    ];
    is_decimal_digit(point)
        || NON_DECIMAL_DIGITS
            .iter()
            .any(|&(first, last)| (first..=last).contains(&point))
}

fn is_digit_text(text: &PyStr) -> bool {
    !text.is_empty()
        && text
            .code_points()
            .iter()
            .all(|&point| is_python_digit(point))
}

/// `int(text)` of a slice of a CUDA text: an unusable digit or too many digits is the `ValueError` of the call at `site`.
fn int_of(text: &PyStr, site: RaiseSite) -> Result<BigInt, Raised> {
    int_from_str(text).map_err(|failure| raised("ValueError", &failure.message, site))
}

fn text_part(text: &PyStr, range: std::ops::Range<usize>) -> PyStr {
    PyStr::from_code_points(text.code_points()[range].to_vec())
}

/// The explicit request had no exact match: the stable list of same-major candidates by distance of the minor digit.
fn nearest_candidates(
    requested: &PyStr,
    candidates: &[Candidate],
) -> Result<Vec<Candidate>, Raised> {
    let length = requested.len();
    if !(is_digit_text(requested) && length >= 2) {
        return Ok(Vec::new());
    }
    let requested_major = int_of(
        &text_part(requested, 0..length - 1),
        RaiseSite::RequestedMajor,
    )?;
    let requested_minor = int_of(
        &text_part(requested, length - 1..length),
        RaiseSite::RequestedMinor,
    )?;
    let mut same_major = Vec::new();
    for candidate in candidates {
        let cuda = candidate
            .cuda
            .as_ref()
            .expect("a +cu candidate has its digits");
        if is_digit_text(cuda)
            && cuda.len() >= 2
            && int_of(
                &text_part(cuda, 0..cuda.len() - 1),
                RaiseSite::NearestGenerator,
            )? == requested_major
        {
            same_major.push(candidate.clone());
        }
    }
    // the sort key `abs(int(c["cuda"][-1]) - req_minor)`; a last character is one digit, so it never meets the digit limit
    let mut keyed = Vec::with_capacity(same_major.len());
    for candidate in same_major {
        let cuda = candidate
            .cuda
            .as_ref()
            .expect("a +cu candidate has its digits");
        let minor = int_of(
            &text_part(cuda, cuda.len() - 1..cuda.len()),
            RaiseSite::NearestGenerator,
        )?;
        keyed.push(((minor - &requested_minor).abs(), candidate));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, candidate)| candidate).collect())
}

/// The candidate with the greatest `int(cuda)`: the last of the greatest, as `sorted(...)[-1]` takes it. Keys are computed in order, so the
/// first one that cannot be converted ends the call.
fn highest_cuda(candidates: &[Candidate]) -> Result<Candidate, Raised> {
    let mut keyed = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let cuda = candidate
            .cuda
            .as_ref()
            .expect("a +cu candidate has its digits");
        keyed.push((int_of(cuda, RaiseSite::HighestKey)?, candidate));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.pop().expect("there is a candidate").1.clone())
}

/// Lines 33-85 after `fetch_json`: `release` is the parsed release document, `cuda` the `--cuda` text (an empty text counts as not given).
pub fn select(
    release: &PyValue,
    version: &PyStr,
    arch: &PyStr,
    cuda: Option<&PyStr>,
) -> Result<Selection, Raised> {
    let names = asset_names(release)?;
    let patterns = WheelPatterns {
        version: version.code_points().to_vec(),
        arch: arch.code_points().to_vec(),
    };
    let (candidates, default_variants) = classify(&names, &patterns)?;
    if candidates.is_empty() && default_variants.is_empty() {
        let assets = names
            .iter()
            .map(|name| {
                if let PyValue::Str(text) = name {
                    text.clone()
                } else {
                    unreachable!("a non-text name already raised")
                }
            })
            .collect();
        return Ok(Selection::NoWheelAssets { assets });
    }
    let requested = cuda.filter(|text| !text.is_empty());
    let chosen = match requested {
        Some(requested) => match candidates
            .iter()
            .find(|candidate| candidate.cuda.as_ref() == Some(requested))
        {
            Some(exact) => exact.clone(),
            None => {
                let nearest = nearest_candidates(requested, &candidates)?;
                return Ok(Selection::NoExactCuda {
                    requested: requested.clone(),
                    available: candidates,
                    default_variants,
                    nearest,
                });
            }
        },
        None if !candidates.is_empty() => highest_cuda(&candidates)?,
        None => default_variants[0].clone(),
    };
    Ok(Selection::Chosen {
        chosen,
        candidates,
        default_variants,
    })
}

/// A list of candidates as the list of dictionaries the original dumps.
pub fn candidate_values(candidates: &[Candidate]) -> PyValue {
    PyValue::List(candidates.iter().map(Candidate::to_value).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(names: &[&str]) -> PyValue {
        let assets = names
            .iter()
            .map(|name| {
                let mut asset = PyDict::new();
                asset.set("name", PyValue::str(name));
                PyValue::Dict(asset)
            })
            .collect();
        let mut document = PyDict::new();
        document.set("assets", PyValue::List(assets));
        PyValue::Dict(document)
    }

    fn pick(names: &[&str], arch: &str, cuda: Option<&str>) -> Selection {
        let cuda = cuda.map(PyStr::new);
        select(
            &release(names),
            &PyStr::new("0.32.0"),
            &PyStr::new(arch),
            cuda.as_ref(),
        )
        .expect("selected")
    }

    fn chosen_name(selection: &Selection) -> String {
        match selection {
            Selection::Chosen { chosen, .. } => chosen.name.to_utf8().unwrap(),
            other => panic!("not chosen: {other:?}"),
        }
    }

    const CU129: &str = "vllm-0.32.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl";
    const CU131: &str = "vllm-0.32.0+cu131-cp38-abi3-manylinux_2_35_aarch64.whl";
    const DEFAULT: &str = "vllm-0.32.0-cp38-abi3-manylinux_2_34_aarch64.whl";

    #[test]
    fn the_automatic_choice_is_the_last_of_the_greatest_numeric_value() {
        let first = "vllm-0.32.0+cu0131-cp38-abi3-manylinux_2_34_aarch64.whl";
        let selection = pick(&[CU129, first, CU131], "aarch64", None);
        // 0131 and 131 are the same number: the later one wins the tie
        assert_eq!(chosen_name(&selection), CU131);
        assert_eq!(
            chosen_name(&pick(&[CU131, first, CU129], "aarch64", None)),
            first
        );
    }

    #[test]
    fn an_explicit_choice_is_the_first_equal_text_and_zero_prefixes_differ() {
        let other = "vllm-0.32.0+cu129-cp39-abi3-manylinux_2_35_aarch64.whl";
        assert_eq!(
            chosen_name(&pick(&[CU129, other], "aarch64", Some("129"))),
            CU129
        );
        assert!(matches!(
            pick(&[CU129], "aarch64", Some("0129")),
            Selection::NoExactCuda { .. }
        ));
        // an empty request is no request
        assert_eq!(
            chosen_name(&pick(&[CU129, CU131], "aarch64", Some(""))),
            CU131
        );
    }

    #[test]
    fn a_default_wheel_is_chosen_only_without_any_cuda_candidate_and_is_reported_apart() {
        let selection = pick(&[DEFAULT], "aarch64", None);
        let Selection::Chosen {
            chosen,
            candidates,
            default_variants,
        } = selection
        else {
            panic!("chosen")
        };
        assert_eq!(chosen.cuda, None);
        assert!(candidates.is_empty());
        assert_eq!(default_variants.len(), 1);
        assert_eq!(chosen.variant, Some(DEFAULT_VARIANT_LABEL));
        assert_eq!(
            chosen_name(&pick(&[DEFAULT, CU129], "aarch64", None)),
            CU129
        );
    }

    #[test]
    fn nearest_candidates_are_the_same_major_sorted_stably_by_minor_distance() {
        let names = [
            "vllm-0.32.0+cu131-cp38-abi3-manylinux_2_34_aarch64.whl",
            "vllm-0.32.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl",
            "vllm-0.32.0+cu133-cp38-abi3-manylinux_2_34_aarch64.whl",
            "vllm-0.32.0+cu121-cp38-abi3-manylinux_2_34_aarch64.whl",
        ];
        let Selection::NoExactCuda { nearest, .. } = pick(&names, "aarch64", Some("132")) else {
            panic!("missing")
        };
        let order: Vec<String> = nearest
            .iter()
            .map(|c| c.cuda.as_ref().unwrap().to_utf8().unwrap())
            .collect();
        // distance 1 (131), distance 1 (133), in input order, then the other major is absent
        assert_eq!(order, ["131", "133"]);
        let Selection::NoExactCuda { nearest, .. } = pick(&names, "aarch64", Some("letters"))
        else {
            panic!("missing")
        };
        assert!(nearest.is_empty());
    }

    #[test]
    fn isdigit_accepts_what_int_refuses() {
        let Err(raised) = select(
            &release(&[CU129]),
            &PyStr::new("0.32.0"),
            &PyStr::new("aarch64"),
            Some(&PyStr::new("²3")),
        ) else {
            panic!("ValueError")
        };
        assert_eq!(raised.site, RaiseSite::RequestedMajor);
        assert_eq!(
            raised.error.message.to_utf8().unwrap(),
            "invalid literal for int() with base 10: '²'"
        );
        let Err(raised) = select(
            &release(&[CU129]),
            &PyStr::new("0.32.0"),
            &PyStr::new("aarch64"),
            Some(&PyStr::new("1²")),
        ) else {
            panic!("ValueError")
        };
        assert_eq!(raised.site, RaiseSite::RequestedMinor);
        assert!(
            is_python_digit(0xB2)
                && is_python_digit(0x0661)
                && !is_python_digit(0x2469)
                && !is_python_digit(u32::from('a'))
        );
    }

    #[test]
    fn unicode_decimal_digits_match_the_digit_groups_and_the_dollar_takes_one_final_line_feed() {
        let arabic = "vllm-0.32.0+cu١٢٩-cp٣٨-abi3-manylinux_٢_٣٤_aarch64.whl";
        let Selection::Chosen { chosen, .. } = pick(&[arabic], "aarch64", None) else {
            panic!("chosen")
        };
        assert_eq!(chosen.cuda, Some(PyStr::new("١٢٩")));
        assert_eq!(chosen.manylinux, PyStr::new("manylinux_٢_٣٤"));
        let with_feed = format!("{CU129}\n");
        assert_eq!(
            chosen_name(&pick(&[with_feed.as_str()], "aarch64", None)),
            with_feed
        );
        let two_feeds = format!("{CU129}\n\n");
        assert!(matches!(
            pick(&[two_feeds.as_str(), DEFAULT], "aarch64", None),
            Selection::Chosen { .. }
        ));
        assert!(matches!(
            pick(&[CU129, "x"], "x86_64", None),
            Selection::NoWheelAssets { .. }
        ));
    }

    #[test]
    fn version_and_arch_are_literal_text() {
        let names = ["vllm-0.32.0+dev(1)+cu129-cp38-abi3-manylinux_2_34_arch.+().whl"];
        let selection = select(
            &release(&names),
            &PyStr::new("0.32.0+dev(1)"),
            &PyStr::new("arch.+()"),
            None,
        );
        assert!(matches!(selection, Ok(Selection::Chosen { .. })));
        let selection = select(
            &release(&["vllm-0X32.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl"]),
            &PyStr::new("0.32.0"),
            &PyStr::new("aarch64"),
            None,
        );
        assert!(matches!(selection, Ok(Selection::NoWheelAssets { .. })));
    }

    #[test]
    fn the_comprehension_and_the_match_raise_what_python_raises() {
        let mut dict = PyDict::new();
        dict.set("assets", PyValue::None);
        let cases: Vec<(PyValue, RaiseSite, &str)> = vec![
            (
                PyValue::Dict(dict),
                RaiseSite::AssetsIteration,
                "'NoneType' object is not iterable",
            ),
            (
                PyValue::List(Vec::new()),
                RaiseSite::ReleaseGet,
                "'list' object has no attribute 'get'",
            ),
        ];
        for (document, site, message) in cases {
            let Err(raised) = asset_names(&document) else {
                panic!("{message}")
            };
            assert_eq!(
                (
                    raised.site,
                    raised.error.message.to_utf8().unwrap().as_str()
                ),
                (site, message)
            );
        }
        let mut with_text = PyDict::new();
        with_text.set("assets", PyValue::str("ab"));
        let Err(raised) = asset_names(&PyValue::Dict(with_text)) else {
            panic!("TypeError")
        };
        assert_eq!(
            raised.error.message.to_utf8().unwrap(),
            "string indices must be integers, not 'str'"
        );
        let mut nameless = PyDict::new();
        nameless.set("assets", PyValue::List(vec![PyValue::Dict(PyDict::new())]));
        let Err(raised) = asset_names(&PyValue::Dict(nameless)) else {
            panic!("KeyError")
        };
        assert_eq!(
            (
                raised.error.class,
                raised.error.message.to_utf8().unwrap().as_str()
            ),
            ("KeyError", "'name'")
        );
        let mut numbered = PyDict::new();
        let mut asset = PyDict::new();
        asset.set("name", PyValue::int(7));
        numbered.set("assets", PyValue::List(vec![PyValue::Dict(asset)]));
        let Err(raised) = select(
            &PyValue::Dict(numbered),
            &PyStr::new("1"),
            &PyStr::new("a"),
            None,
        ) else {
            panic!("TypeError")
        };
        assert_eq!(raised.site, RaiseSite::NameMatch);
        assert_eq!(
            raised.error.message.to_utf8().unwrap(),
            "expected string or bytes-like object, got 'int'"
        );
    }

    #[test]
    fn a_digit_run_over_the_integer_limit_ends_the_automatic_choice_with_the_value_error() {
        let long = format!(
            "vllm-0.32.0+cu{}-cp38-abi3-manylinux_2_34_aarch64.whl",
            "7".repeat(4301)
        );
        let Err(raised) = select(
            &release(&[long.as_str()]),
            &PyStr::new("0.32.0"),
            &PyStr::new("aarch64"),
            None,
        ) else {
            panic!("ValueError")
        };
        assert_eq!(raised.site, RaiseSite::HighestKey);
        assert!(raised.error.message.to_utf8().unwrap().starts_with(
            "Exceeds the limit (4300 digits) for integer string conversion: value has 4301 digits"
        ));
    }
}
