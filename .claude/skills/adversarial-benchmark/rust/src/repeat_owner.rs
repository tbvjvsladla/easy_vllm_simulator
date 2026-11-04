//! The campaign-declaration side of the repeat axis: [`load_campaign_budgets`] and [`resolve_repeats`].
//!
//! The *owner* of "which campaign is active", "where does a campaign live" and "which ids are reserved" is
//! `terraforming_node/scripts/campaign_init.py`, a Python program that has not been ported (pending S4). This
//! module owns everything else: the order of the queries, the truth of a raw `campaign_id`, the reserved-id
//! refusal, the existence and shape checks of the declaration, the repeat-count precedence
//! (argv > campaign declaration > preset default) and the provenance text.
//!
//! The owner is reached only through [`CampaignOwner`]. Its one implementation, [`PythonCampaignOwner`], starts
//! a query adapter - a small Python program given by the explicit parameter or `REPEAT_AXIS_CAMPAIGN_ADAPTER` -
//! that loads the actual `campaign_init.py` the way the original did and answers four questions by calling the
//! owner's own functions and attributes, returning their results or exceptions. The adapter holds no repeat
//! logic, and nothing here falls back to `repeat_axis.py` or recomputes the owner's rules: when the owner exists
//! but cannot be queried the failure says so.

use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::loads;
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_axis::{parse_repeats, repeat_axis_error, CAMPAIGN_INIT_REL};
use crate::repeat_text::cat;
use num_bigint::BigInt;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Environment variable naming the query adapter program.
pub const ADAPTER_ENV: &str = "REPEAT_AXIS_CAMPAIGN_ADAPTER";
/// Environment variable naming the Python interpreter that runs the adapter (default `python3`).
pub const PYTHON_ENV: &str = "REPEAT_AXIS_PYTHON";
/// Exception class reported when the owner exists but the query path to it is not usable. It is not an exception
/// of the original program; the original had no such path.
pub const ADAPTER_ERROR: &str = "CampaignOwnerAdapterError";

/// Environment variable naming the repository root the owner and the schema are read under.
pub const REPO_ROOT_ENV: &str = "REPEAT_AXIS_REPO_ROOT";
/// Exception class reported when a path that needs the repository root runs without one.
pub const CONFIGURATION_ERROR: &str = "RepeatAxisConfigurationError";

/// The repository root of a run. The original's default was the location of its own file; a native build has no
/// such file, so the root is given (parameter or [`REPO_ROOT_ENV`]) and an operation that needs it without one
/// fails by name. Operations that never touch the root (an explicit `--repeats`) do not need it.
#[derive(Clone, Debug, Default)]
pub struct RepoRoot(RootState);

#[derive(Clone, Debug, Default)]
enum RootState {
    #[default]
    Unset,
    Given(PyStr),
    /// A value that is not a path (`Path(value)` raises `TypeError` when the root is first used).
    Invalid(String),
}

impl RepoRoot {
    pub fn given(path: PyStr) -> Self {
        RepoRoot(RootState::Given(path))
    }

    pub fn unset() -> Self {
        RepoRoot(RootState::Unset)
    }

    /// A root as a caller's JSON value gives it: a string is a path, any other type fails when it is used.
    pub fn from_value(value: &PyValue) -> Self {
        match value.peel() {
            PyValue::Str(path) => RepoRoot::given(path.clone()),
            other => RepoRoot(RootState::Invalid(format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                other.type_name()
            ))),
        }
    }

    /// The root named by [`REPO_ROOT_ENV`], if any.
    pub fn from_environment() -> Self {
        use std::os::unix::ffi::OsStrExt;
        match std::env::var_os(REPO_ROOT_ENV).filter(|value| !value.is_empty()) {
            Some(value) => RepoRoot::given(PyStr::from_surrogateescape(value.as_bytes())),
            None => RepoRoot::unset(),
        }
    }

    pub fn get(&self) -> Result<&PyStr, Failure> {
        match &self.0 {
            RootState::Given(path) => Ok(path),
            RootState::Invalid(message) => Err(Failure::new("TypeError", message)),
            RootState::Unset => Err(Failure::new(
                CONFIGURATION_ERROR,
                &format!("the repository root is not configured; set {REPO_ROOT_ENV} (the original derived it from its own file location)"),
            )),
        }
    }
}

// ── the owner ───────────────────────────────────────────────────────────────────

/// A loaded `campaign_init` owner. Every method is one query to the owner and returns what the owner returned
/// or the exception it raised.
pub trait OwnerSession {
    /// `mod.active_campaign_id()`.
    fn active_campaign_id(&mut self) -> Result<PyValue, Failure>;
    /// `mod.BOOTSTRAP` (an `AttributeError` when the owner has no such attribute).
    fn bootstrap_id(&mut self) -> Result<PyValue, Failure>;
    /// `getattr(mod, "RESERVED_IDS", ())`.
    fn reserved_ids(&mut self) -> Result<PyValue, Failure>;
    /// `str(mod.derive_path("root", camp_id=camp_id))`.
    fn derive_root_path(&mut self, camp_id: &PyValue) -> Result<PyStr, Failure>;
}

/// Where the owner is looked up.
pub trait CampaignOwner {
    /// Loads the owner file under `repo_root` as the original's `_campaign_init_module` did: `Ok(None)` when the
    /// file is absent; an exception of the owner's own top level propagates.
    fn open(&self, repo_root: &PyStr) -> Result<Option<Box<dyn OwnerSession>>, Failure>;
}

/// The query adapter configuration.
#[derive(Clone, Debug)]
pub struct PythonCampaignOwner {
    python: OsString,
    adapter: Option<OsString>,
}

impl PythonCampaignOwner {
    pub fn new(python: impl Into<OsString>, adapter: Option<OsString>) -> Self {
        PythonCampaignOwner {
            python: python.into(),
            adapter,
        }
    }

    /// The configuration of this process: [`ADAPTER_ENV`] and [`PYTHON_ENV`].
    pub fn from_environment() -> Self {
        Self::new(
            std::env::var_os(PYTHON_ENV).unwrap_or_else(|| OsString::from("python3")),
            std::env::var_os(ADAPTER_ENV).filter(|value| !value.is_empty()),
        )
    }
}

fn adapter_error(message: String) -> Failure {
    Failure::new(ADAPTER_ERROR, &message)
}

/// The exception classes the adapter relays are names; a `Failure` wants a static one.
fn exception_class(name: &str) -> &'static str {
    const KNOWN: [&str; 16] = [
        "AttributeError",
        "TypeError",
        "ValueError",
        "KeyError",
        "OSError",
        "FileNotFoundError",
        "PermissionError",
        "NotADirectoryError",
        "IsADirectoryError",
        "ImportError",
        "ModuleNotFoundError",
        "SyntaxError",
        "NameError",
        "RuntimeError",
        "RecursionError",
        "PurgeGateRefusal",
    ];
    KNOWN
        .iter()
        .find(|known| **known == name)
        .copied()
        // An owner-defined class: the name is kept for the report; the handful of names leaked live for the process.
        .unwrap_or_else(|| Box::leak(name.to_string().into_boxed_str()))
}

struct PythonSession {
    child: Child,
    writer: Option<ChildStdin>,
    reader: BufReader<ChildStdout>,
}

impl PythonSession {
    fn request(&mut self, request: PyDict) -> Result<PyDict, Failure> {
        let line = PyValue::Dict(request)
            .dumps_ascii()
            .map_err(|error| adapter_error(error.to_string()))?;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| adapter_error("adapter input is closed".to_string()))?;
        writeln!(writer, "{line}")
            .and_then(|()| writer.flush())
            .map_err(|error| adapter_error(format!("cannot write to the adapter: {error}")))?;
        let mut answer = String::new();
        let read = self
            .reader
            .read_line(&mut answer)
            .map_err(|error| adapter_error(format!("cannot read from the adapter: {error}")))?;
        if read == 0 {
            return Err(adapter_error(
                "the adapter ended without answering (see its stderr)".to_string(),
            ));
        }
        match loads(&answer) {
            Ok(PyValue::Dict(reply)) => Ok(reply),
            Ok(other) => Err(adapter_error(format!(
                "the adapter answered a {}, not an object",
                other.type_name()
            ))),
            Err(error) => Err(adapter_error(format!("unreadable adapter answer: {error}"))),
        }
    }

    /// Sends one query; an owner exception becomes the same exception here.
    fn ask(&mut self, operation: &str, extra: Vec<(&str, PyValue)>) -> Result<PyDict, Failure> {
        let mut request = PyDict::new();
        request.set("op", PyValue::str(operation));
        for (name, value) in extra {
            request.set(name, value);
        }
        let reply = self.request(request)?;
        match reply.get("ok") {
            Some(PyValue::Bool(true)) => Ok(reply),
            _ => {
                let class = match reply.get("type") {
                    Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
                    _ => ADAPTER_ERROR.to_string(),
                };
                let message = match reply.get("message") {
                    Some(PyValue::Str(text)) => text.clone(),
                    _ => PyStr::default(),
                };
                Err(Failure::with_message(exception_class(&class), message))
            }
        }
    }

    fn value_of(reply: &PyDict) -> PyValue {
        reply.get("value").cloned().unwrap_or(PyValue::None)
    }
}

impl Drop for PythonSession {
    fn drop(&mut self) {
        self.writer.take();
        let _ = self.child.wait();
    }
}

impl OwnerSession for PythonSession {
    fn active_campaign_id(&mut self) -> Result<PyValue, Failure> {
        Ok(Self::value_of(&self.ask("active_campaign_id", vec![])?))
    }

    fn bootstrap_id(&mut self) -> Result<PyValue, Failure> {
        Ok(Self::value_of(&self.ask(
            "attribute",
            vec![("name", PyValue::str("BOOTSTRAP"))],
        )?))
    }

    fn reserved_ids(&mut self) -> Result<PyValue, Failure> {
        let reply = self.ask(
            "attribute_or_empty",
            vec![("name", PyValue::str("RESERVED_IDS"))],
        )?;
        Ok(Self::value_of(&reply))
    }

    fn derive_root_path(&mut self, camp_id: &PyValue) -> Result<PyStr, Failure> {
        let reply = self.ask("derive_root_path", vec![("camp_id", camp_id.clone())])?;
        match Self::value_of(&reply) {
            PyValue::Str(path) => Ok(path),
            other => Err(adapter_error(format!(
                "derive_path returned a {}, not a path",
                other.type_name()
            ))),
        }
    }
}

impl CampaignOwner for PythonCampaignOwner {
    fn open(&self, repo_root: &PyStr) -> Result<Option<Box<dyn OwnerSession>>, Failure> {
        let owner_path = pyfs::join_text(repo_root, &PyStr::new(CAMPAIGN_INIT_REL));
        if !pyfs::is_file(&owner_path)? {
            return Ok(None);
        }
        let Some(adapter) = &self.adapter else {
            return Err(adapter_error(format!(
                "the campaign_init owner exists at {} but no query adapter is configured; set {ADAPTER_ENV} (this build does not fall back to repeat_axis.py)",
                owner_path.repr()
            )));
        };
        let mut child = Command::new(&self.python)
            .arg("-B")
            .arg(adapter)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| {
                adapter_error(format!(
                    "cannot start the campaign owner adapter {:?} with {:?}: {error}",
                    adapter, self.python
                ))
            })?;
        let writer = child.stdin.take();
        let reader = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut session = PythonSession {
            child,
            writer,
            reader,
        };
        session.ask("load", vec![("path", PyValue::Str(owner_path))])?;
        Ok(Some(Box::new(session)))
    }
}

// ── load_campaign_budgets ───────────────────────────────────────────────────────

/// What the declaration query found: the campaign and its `budgets`, or why there is none.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignBudgets {
    /// The campaign whose declaration was read; `None` when there is no active campaign or no owner.
    pub campaign: Option<PyValue>,
    pub budgets: Option<PyDict>,
    /// Why nothing was read (empty when something was).
    pub absence: PyStr,
}

impl CampaignBudgets {
    fn absent(reason: PyStr) -> Self {
        CampaignBudgets {
            campaign: None,
            budgets: None,
            absence: reason,
        }
    }

    /// The Python return value `(campaign id | None, budgets | None, absence reason)`.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![
            self.campaign.clone().unwrap_or(PyValue::None),
            self.budgets.clone().map_or(PyValue::None, PyValue::Dict),
            PyValue::Str(self.absence.clone()),
        ])
    }
}

/// Python `item in container` for the container types a JSON attribute can be: a sequence compares with `==`, a
/// string tests for a substring (and needs a string item), a dict tests its keys (and needs a hashable item).
fn contains(container: &PyValue, item: &PyValue) -> Result<bool, Failure> {
    match container.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            Ok(items.iter().any(|own| py_eq(own, item)))
        }
        PyValue::Str(text) => match item.peel() {
            PyValue::Str(needle) => {
                let (haystack, needle) = (text.code_points(), needle.code_points());
                Ok(needle.is_empty()
                    || haystack
                        .windows(needle.len())
                        .any(|window| window == needle))
            }
            other => Err(Failure::new(
                "TypeError",
                &format!(
                    "'in <string>' requires string as left operand, not {}",
                    other.type_name()
                ),
            )),
        },
        PyValue::Dict(map) => match item.peel() {
            PyValue::List(_) | PyValue::Dict(_) => Err(Failure::new(
                "TypeError",
                &format!("unhashable type: '{}'", item.type_name()),
            )),
            PyValue::Str(key) => Ok(map.iter().any(|(own, _)| own == key)),
            _ => Ok(false),
        },
        other => Err(Failure::new(
            "TypeError",
            &format!("argument of type '{}' is not iterable", other.type_name()),
        )),
    }
}

/// The `budgets` of the active (or the given) campaign's declaration. Repeat counts and the bench knob pins
/// read the same declaration with the same discipline, so a declaration that cannot be read is never folded into
/// "not declared": it is a refusal.
///
/// * a declaration that exists but cannot be read (damaged, wrong shape, instance missing) is a `RepeatAxisError`;
/// * no owner, or no active campaign, is `budgets = None` with the reason, for the caller to name in its output.
pub fn load_campaign_budgets(
    campaign_id: &PyValue,
    repo_root: &RepoRoot,
    owner: &dyn CampaignOwner,
) -> Result<CampaignBudgets, Failure> {
    let Some(mut session) = owner.open(repo_root.get()?)? else {
        return Ok(CampaignBudgets::absent(cat!(
            "캠페인 해소자 부재: ",
            CAMPAIGN_INIT_REL,
            " — 선언을 읽지 못했다"
        )));
    };
    let camp = if campaign_id.is_truthy() {
        campaign_id.clone()
    } else {
        session.active_campaign_id()?
    };
    let bootstrap = session.bootstrap_id()?;
    if py_eq(&camp, &bootstrap) {
        return Ok(CampaignBudgets::absent(cat!(
            "활성 캠페인 없음(",
            bootstrap.display_text(),
            ")"
        )));
    }
    if contains(&session.reserved_ids()?, &camp)? {
        return Err(repeat_axis_error(cat!(
            "--campaign-id ",
            camp.repr(),
            " 는 예약 id 다 — 선언이 아니다"
        )));
    }
    let base = session.derive_root_path(&camp)?;
    let declaration = pyfs::join(&base, "campaign.yaml");
    if !pyfs::is_dir(&base)? {
        return Err(repeat_axis_error(cat!(
            "campaigns/",
            camp.display_text(),
            " 인스턴스가 없다 — 반복 수를 물을 선언이 없다"
        )));
    }
    let document = match pyfs::load_json_or_none(&declaration)? {
        Some(PyValue::Dict(document)) => document,
        _ => {
            return Err(repeat_axis_error(cat!(
            "campaigns/",
            camp.display_text(),
            "/campaign.yaml 을 읽지 못했다(부재·파손) — 읽지 못한 선언을 '미선언' 으로 접지 않는다"
        )))
        }
    };
    let budgets = match document.get("budgets") {
        None => PyDict::new(),
        Some(PyValue::Dict(budgets)) => budgets.clone(),
        Some(_) => {
            return Err(repeat_axis_error(cat!(
                "campaigns/",
                camp.display_text(),
                "/campaign.yaml budgets 가 객체가 아니다"
            )))
        }
    };
    Ok(CampaignBudgets {
        campaign: Some(camp),
        budgets: Some(budgets),
        absence: PyStr::default(),
    })
}

// ── resolve_repeats ─────────────────────────────────────────────────────────────

/// The inputs of a repeat-count resolution, each as the caller gave it (`None` is "not given").
#[derive(Clone, Debug)]
pub struct ResolveRequest {
    /// `--repeats`
    pub argv_value: PyValue,
    /// `--repeats-source`
    pub argv_source: PyValue,
    pub campaign_id: PyValue,
    /// The preset's `knobs.repeats.default`, passed in by `bench_preset.py`.
    pub preset_default: PyValue,
    /// The preset's name, for the provenance text.
    pub preset_source: PyValue,
}

impl Default for ResolveRequest {
    fn default() -> Self {
        ResolveRequest {
            argv_value: PyValue::None,
            argv_source: PyValue::None,
            campaign_id: PyValue::None,
            preset_default: PyValue::None,
            preset_source: PyValue::None,
        }
    }
}

/// A repeat count and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRepeats {
    pub count: BigInt,
    pub source: PyStr,
}

impl ResolvedRepeats {
    /// The Python return value `(count, source)`.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![
            PyValue::Int(self.count.clone()),
            PyValue::Str(self.source.clone()),
        ])
    }
}

/// The repeat count: argv, then the campaign declaration, then the preset default. The lowest source is the
/// preset's alone (`config/<tool>/<preset>.yaml knobs.repeats.default`, passed in by `bench_preset.py`); a number
/// is never invented here. The floor is the full definition and stays in [`parse_repeats`].
///
/// A declaration that cannot be read is a refusal. A missing owner is a loud substitution: the count is a
/// measurement configuration, not a gate, and the preset default meets the definition, so measurement goes on
/// with the absence named in the source.
pub fn resolve_repeats(
    request: &ResolveRequest,
    repo_root: &RepoRoot,
    owner: &dyn CampaignOwner,
) -> Result<ResolvedRepeats, Failure> {
    if !matches!(request.argv_value.peel(), PyValue::None)
        && !request.argv_value.display_text().strip().is_empty()
    {
        let count = parse_repeats(&request.argv_value, &PyValue::str("--repeats"))?;
        let suffix = if request.argv_source.is_truthy() {
            cat!(" · ", request.argv_source.display_text())
        } else {
            PyStr::default()
        };
        return Ok(ResolvedRepeats {
            count,
            source: cat!("declared(argv --repeats", suffix, ")"),
        });
    }
    if request.argv_source.is_truthy() {
        return Err(repeat_axis_error(cat!(
            "--repeats-source 는 --repeats 와 함께만 쓴다(값 없는 출처는 거짓 표시다)"
        )));
    }

    let defaulted = |why: PyStr| -> Result<ResolvedRepeats, Failure> {
        if matches!(request.preset_default.peel(), PyValue::None)
            || !request.preset_source.is_truthy()
        {
            return Err(repeat_axis_error(cat!(
                "반복 수 선언이 없고(",
                why,
                ") 프리셋 기본값도 받지 못했다 — 기본값의 자리는 프리셋 하나다(bench_preset.py 가 넘긴다 · 코드 안 숫자로 메우지 않는다)"
            )));
        }
        let preset = request.preset_source.display_text();
        let where_ = PyValue::Str(cat!("preset ", preset, " knobs.repeats.default"));
        let count = parse_repeats(&request.preset_default, &where_)?;
        let source = cat!(
            "defaulted(preset ",
            preset,
            " 기본 ",
            count.to_string(),
            " · ",
            why,
            ")"
        );
        Ok(ResolvedRepeats { count, source })
    };

    let found = load_campaign_budgets(&request.campaign_id, repo_root, owner)?;
    let Some(budgets) = &found.budgets else {
        return defaulted(found.absence);
    };
    let camp = found
        .campaign
        .as_ref()
        .map(PyValue::display_text)
        .unwrap_or_default();
    let Some(declared) = budgets.get("repeats") else {
        return defaulted(cat!(
            "campaigns/",
            camp,
            "/campaign.yaml budgets.repeats 미선언"
        ));
    };
    let where_ = PyValue::Str(cat!("campaigns/", camp, "/campaign.yaml budgets.repeats"));
    let count = parse_repeats(declared, &where_)?;
    Ok(ResolvedRepeats {
        count,
        source: cat!(
            "declared(campaigns/",
            camp,
            "/campaign.yaml budgets.repeats)"
        ),
    })
}
