//! The campaign-declaration side of the repeat axis: [`load_campaign_budgets`] and [`resolve_repeats`].
//!
//! The *owner* of "which campaign is active", "where does a campaign live" and "which ids are reserved" is the campaign
//! read side that `role-contract` owns (the semantics of `terraforming_node/scripts/campaign_init.py`). This module owns
//! everything else: the order of the queries, the truth of a raw `campaign_id`, the reserved-id refusal, the existence
//! and shape checks of the declaration, the repeat-count precedence (argv > campaign declaration > preset default) and
//! the provenance text.
//!
//! The owner is reached only through [`CampaignOwner`]. [`ConfiguredCampaignOwner`] decides, per open, whether the
//! campaign resolver is installed in the checkout (see [`Selection`]): it is when the owner `campaign_init.py` or its
//! launcher `campaign_init.sh` is a regular file (absence of both alone means *no resolver*). An installed resolver is
//! always answered natively in this process by the `role-contract` read side: no interpreter, no query adapter, no
//! source compile, whatever the bytes of the owner file are. There is no Python route and nothing falls back
//! (2026-11-16 · plan_26111615: the former `REPEAT_AXIS_CAMPAIGN_ADAPTER` / `REPEAT_AXIS_PYTHON` /
//! `REPEAT_AXIS_COMPAT_ADAPTER` settings and the adapter `repeat_axis_owner_query.py` are no longer used).

use crate::guidellm::Failure;
use crate::pyfs;
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_axis::{parse_repeats, repeat_axis_error, CAMPAIGN_INIT_REL};
use crate::repeat_text::cat;
use agent_runtime_support::pystr::PyStr as RuntimeStr;
use agent_runtime_support::pyvalue::{PyDict as RuntimeDict, PyValue as RuntimeValue};
use num_bigint::BigInt;
use role_contract::campaign::{self as campaign_read, OwnedModuleSite};
use role_contract::topology::Crash;
use std::ffi::OsString;
use std::io::Write;

/// The launcher of the campaign resolver, relative to the checkout: its presence also means the resolver is installed.
pub const CAMPAIGN_LAUNCHER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.sh";
/// Environment variable naming a private file that receives one JSON line per owner open (route evidence outside the
/// business output). Unset: nothing is recorded.
pub const ROUTE_OBSERVER_ENV: &str = "REPEAT_AXIS_ROUTE_OBSERVER";
/// Exception class reported when a requested route observation cannot be written.
pub const ROUTE_OBSERVER_ERROR: &str = "CampaignRouteObserverError";
/// Exception class reported when the owner exists but cannot be queried. It is not an exception of the original
/// program; the original had no such path.
pub const ADAPTER_ERROR: &str = "CampaignOwnerAdapterError";

fn adapter_error(message: String) -> Failure {
    Failure::new(ADAPTER_ERROR, &message)
}

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

// ── the native owner ────────────────────────────────────────────────────────────

fn text_of(value: &RuntimeStr) -> PyStr {
    PyStr::from_code_points(value.code_points().to_vec())
}

fn runtime_text(value: &PyStr) -> RuntimeStr {
    RuntimeStr::from_code_points(value.code_points().to_vec())
}

/// The exception a read-side function raised, as the adapter relays it: the class name and `str(exception)`.
fn failure_of(crash: &Crash) -> Failure {
    Failure::with_message(crash.class, text_of(&crash.message))
}

/// A caller's `camp_id` as the read side sees it. The owner's function only ever observes its type name, its truth and (for
/// text) its code points, so a container keeps its emptiness and nothing deeper. A value outside JSON's family is refused
/// rather than guessed.
fn runtime_camp_id(value: &PyValue) -> Result<RuntimeValue, Failure> {
    Ok(match value.peel() {
        PyValue::None => RuntimeValue::None,
        PyValue::Bool(flag) => RuntimeValue::Bool(*flag),
        PyValue::Int(number) => RuntimeValue::Int(number.clone()),
        PyValue::Float(number) => RuntimeValue::Float(*number),
        PyValue::Str(text) => RuntimeValue::Str(runtime_text(text)),
        PyValue::Bytes(bytes) => RuntimeValue::Bytes(bytes.clone()),
        PyValue::List(items) => RuntimeValue::List(vec![RuntimeValue::None; items.len()]),
        PyValue::Tuple(items) => RuntimeValue::Tuple(vec![RuntimeValue::None; items.len()]),
        PyValue::Dict(map) => {
            let mut stand_in = RuntimeDict::new();
            if !map.is_empty() {
                stand_in.set("_", RuntimeValue::None);
            }
            RuntimeValue::Dict(stand_in)
        }
        other => {
            return Err(adapter_error(format!(
                "a campaign id of type {} cannot be passed to the native owner",
                other.type_name()
            )))
        }
    })
}

/// The reviewed `campaign_init.py` answering in this process. Its module-global root is where the loaded file itself
/// resolves to (`Path(__file__).resolve().parents[4]`), not the caller's checkout.
struct NativeSession {
    site: OwnedModuleSite,
}

impl NativeSession {
    /// The import of the module: its own location is the only thing the top level of the reviewed file computes that can fail.
    fn load(owner_bytes: &[u8]) -> Result<Self, Failure> {
        campaign_read::module_site(owner_bytes)
            .map(|site| NativeSession { site })
            .map_err(|crash| failure_of(&crash))
    }
}

impl OwnerSession for NativeSession {
    fn active_campaign_id(&mut self) -> Result<PyValue, Failure> {
        campaign_read::active_campaign_id(None, &self.site.root, &self.site.file)
            .map(|id| PyValue::Str(text_of(&id)))
            .map_err(|crash| failure_of(&crash))
    }

    fn bootstrap_id(&mut self) -> Result<PyValue, Failure> {
        Ok(PyValue::str(campaign_read::BOOTSTRAP))
    }

    /// The adapter's JSON erased the tuple: an array of the reserved ids.
    fn reserved_ids(&mut self) -> Result<PyValue, Failure> {
        Ok(PyValue::List(
            campaign_read::RESERVED_IDS
                .iter()
                .map(|id| PyValue::str(id))
                .collect(),
        ))
    }

    fn derive_root_path(&mut self, camp_id: &PyValue) -> Result<PyStr, Failure> {
        campaign_read::derive_root_path(&runtime_camp_id(camp_id)?, self.site.as_site())
            .map(|path| text_of(&path))
            .map_err(|crash| failure_of(&crash))
    }
}

// ── the route ───────────────────────────────────────────────────────────────────

/// Which program answers the campaign queries of one open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignRoute {
    /// Neither the owner nor its launcher is a regular file: the resolver is absent (nothing is asked).
    Absent,
    /// The installed resolver, answered by the native read side in this process (the only route there is).
    NativeCurrentSource,
}

impl CampaignRoute {
    pub fn name(&self) -> &'static str {
        match self {
            CampaignRoute::Absent => "Absent",
            CampaignRoute::NativeCurrentSource => "NativeCurrentSource",
        }
    }
}

/// The decision of one open.
#[derive(Clone, Debug)]
pub enum Selection {
    Absent,
    Native { owner_path: PyStr },
}

impl Selection {
    pub fn route(&self) -> CampaignRoute {
        match self {
            Selection::Absent => CampaignRoute::Absent,
            Selection::Native { .. } => CampaignRoute::NativeCurrentSource,
        }
    }
}

/// The product's owner: decides per open whether the resolver is installed, then answers natively.
#[derive(Clone, Debug, Default)]
pub struct ConfiguredCampaignOwner {
    observer: Option<OsString>,
}

impl ConfiguredCampaignOwner {
    pub fn new() -> Self {
        ConfiguredCampaignOwner { observer: None }
    }

    /// Records one JSON line per open into `path` (evidence outside the business output).
    pub fn observed_by(mut self, path: OsString) -> Self {
        self.observer = Some(path);
        self
    }

    /// The configuration of this process: only the route observer ([`ROUTE_OBSERVER_ENV`]).
    pub fn from_environment() -> Self {
        let owner = Self::new();
        match std::env::var_os(ROUTE_OBSERVER_ENV).filter(|value| !value.is_empty()) {
            Some(path) => owner.observed_by(path),
            None => owner,
        }
    }

    /// The route of an open, decided before anything runs. The original's `is_file` comes first (the owner, then its
    /// launcher): absence of both is its answer and a `stat` failure it does not swallow propagates.
    pub fn select(&self, repo_root: &PyStr) -> Result<Selection, Failure> {
        let owner_path = pyfs::join_text(repo_root, &PyStr::new(CAMPAIGN_INIT_REL));
        if pyfs::is_file(&owner_path)? {
            return Ok(Selection::Native { owner_path });
        }
        let launcher = pyfs::join_text(repo_root, &PyStr::new(CAMPAIGN_LAUNCHER_REL));
        if pyfs::is_file(&launcher)? {
            return Ok(Selection::Native { owner_path });
        }
        Ok(Selection::Absent)
    }

    fn open_selected(selection: &Selection) -> Result<Option<Box<dyn OwnerSession>>, Failure> {
        match selection {
            Selection::Absent => Ok(None),
            Selection::Native { owner_path } => {
                let bytes = role_contract::pyprofile::fs_encode(&runtime_text(owner_path))
                    .map_err(|why| {
                        adapter_error(format!("the owner path cannot be encoded: {why:?}"))
                    })?;
                NativeSession::load(&bytes).map(|session| Some(Box::new(session) as _))
            }
        }
    }

    /// One line per open, written after the open ended and before its result is used. The line keeps the fields it
    /// had when a Python route existed (`reason` null, `python_child` false, `child_pid` null).
    fn record(
        &self,
        selection: &Selection,
        outcome: &Result<Option<Box<dyn OwnerSession>>, Failure>,
    ) -> Result<(), Failure> {
        let Some(path) = &self.observer else {
            return Ok(());
        };
        let route = selection.route();
        let mut line = PyDict::new();
        line.set("route", PyValue::str(route.name()));
        line.set("reason", PyValue::None);
        line.set("pid", PyValue::int(std::process::id()));
        line.set(
            "owner_path",
            match selection {
                Selection::Absent => PyValue::None,
                Selection::Native { owner_path } => PyValue::Str(owner_path.clone()),
            },
        );
        line.set("python_child", PyValue::Bool(false));
        line.set("child_pid", PyValue::None);
        line.set("opened", PyValue::Bool(matches!(outcome, Ok(Some(_)))));
        line.set(
            "failure",
            match outcome {
                Err(failure) => PyValue::Str(failure.line()),
                Ok(_) => PyValue::None,
            },
        );
        let text = PyValue::Dict(line)
            .dumps_ascii()
            .map_err(|error| Failure::new(ROUTE_OBSERVER_ERROR, &error.to_string()))?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{text}"))
            .map_err(|error| {
                Failure::new(
                    ROUTE_OBSERVER_ERROR,
                    &format!("the route observation cannot be written: {error}"),
                )
            })
    }
}

impl CampaignOwner for ConfiguredCampaignOwner {
    fn open(&self, repo_root: &PyStr) -> Result<Option<Box<dyn OwnerSession>>, Failure> {
        let selection = self.select(repo_root)?;
        let outcome = Self::open_selected(&selection);
        self.record(&selection, &outcome)?;
        outcome
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
                ") 프리셋 기본값도 받지 못했다 — 기본값의 자리는 프리셋 하나다(bench_preset.sh 가 넘긴다 · 코드 안 숫자로 메우지 않는다)"
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
