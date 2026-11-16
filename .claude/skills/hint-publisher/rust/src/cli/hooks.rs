//! Self-test injection points of the CLI (the Rust form of the original self-test's `mock.patch.object(<module>, <fn>)`).
//!
//! The original patches module attributes (`naming.derive_name`, `evidence.binding_artifact_path`, `branch.commit_payload`,
//! `hint._lint_fn`, `artifacts.collect`, `evidence.from_campaign`, `evidence.drive_publisher`,
//! `evidence.rebind_campaign_narratives`) for the duration of one check. Rust cannot rebind another module's function, so the
//! CLI routes each of those calls through the wrappers below; a wrapper calls the real owner unless a hook is installed for
//! the current thread. Production never installs a hook (`--self-test` only, scoped by [`with`]). A patched function only
//! affects calls made from the CLI (the original's patch also reached calls made inside the owner module itself).
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::core::HR;
use crate::naming::DerivedName;
use crate::value::J;
use crate::{artifacts, branch, evidence, naming};

pub type DeriveNameFn = Rc<dyn Fn(&J, &J) -> HR<DerivedName>>;
pub type BindingPathFn = Rc<dyn Fn(&J, Option<&Path>) -> HR<Option<String>>>;
pub type CommitPayloadFn = Rc<dyn Fn(&Path, &Path, &str, &str, Option<&str>, Option<&str>) -> HR<String>>;
/// `artifacts.collect(repo, ev, payload_dir, runner=...)` — `ev` mutable (the original's hook mutates its argument).
pub type CollectFn = Rc<dyn Fn(&Path, &mut J, &Path, &J) -> HR<J>>;
pub type FromCampaignFn = Rc<dyn Fn(&Path, &J, &J, &J, &J) -> HR<J>>;
pub type DrivePublisherFn = Rc<dyn Fn(&Path, &J, &J, &J, &Path) -> HR<PathBuf>>;
pub type RebindFn = Rc<dyn Fn(&Path, &J, &J, &J, &Path) -> HR<J>>;

/// The installed hooks (`None` = the real function).
#[derive(Clone, Default)]
pub struct Hooks {
    pub derive_name: Option<DeriveNameFn>,
    pub binding_artifact_path: Option<BindingPathFn>,
    pub commit_payload: Option<CommitPayloadFn>,
    /// `_lint_fn` patched to return a linter that answers these findings (a J list).
    pub lint_findings: Option<J>,
    pub collect: Option<CollectFn>,
    pub from_campaign: Option<FromCampaignFn>,
    pub drive_publisher: Option<DrivePublisherFn>,
    pub rebind_campaign_narratives: Option<RebindFn>,
}

thread_local! {
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
}

fn get() -> Hooks {
    HOOKS.with(|h| h.borrow().clone())
}

/// Install hooks for the duration of `f` (`with mock.patch.object(...)`), restoring the previous set afterwards.
pub fn with<T>(set: impl FnOnce(&mut Hooks), f: impl FnOnce() -> T) -> T {
    let prev = get();
    HOOKS.with(|h| set(&mut h.borrow_mut()));
    let r = f();
    HOOKS.with(|h| *h.borrow_mut() = prev);
    r
}

pub fn derive_name(facts: &J, vocab: &J) -> HR<DerivedName> {
    match get().derive_name {
        Some(h) => h(facts, vocab),
        None => naming::derive_name(facts, vocab),
    }
}

pub fn binding_artifact_path(manifest: &J, repo: Option<&Path>) -> HR<Option<String>> {
    match get().binding_artifact_path {
        Some(h) => h(manifest, repo),
        None => evidence::binding_artifact_path(manifest, repo),
    }
}

pub fn commit_payload(repo: &Path, payload: &Path, message: &str, utc: &str, guide: Option<&str>, remote: Option<&str>) -> HR<String> {
    match get().commit_payload {
        Some(h) => h(repo, payload, message, utc, guide, remote),
        None => branch::commit_payload(repo, payload, message, utc, guide, remote),
    }
}

pub fn lint_findings() -> Option<J> {
    get().lint_findings
}

pub fn collect(repo: &Path, ev: &mut J, payload: &Path, runner: &J) -> HR<J> {
    match get().collect {
        Some(h) => h(repo, ev, payload, runner),
        None => artifacts::collect(repo, ev, payload, &J::Null, &J::Null, runner),
    }
}

pub fn from_campaign(repo: &Path, campaign: &J, cell: &J, node: &J, docker: &J) -> HR<J> {
    match get().from_campaign {
        Some(h) => h(repo, campaign, cell, node, docker),
        None => evidence::from_campaign(repo, campaign, cell, node, docker),
    }
}

pub fn drive_publisher(repo: &Path, ev: &J, topic: &J, utc: &J, draft: &Path) -> HR<PathBuf> {
    match get().drive_publisher {
        Some(h) => h(repo, ev, topic, utc, draft),
        None => evidence::drive_publisher(repo, ev, topic, utc, draft),
    }
}

pub fn rebind_campaign_narratives(repo: &Path, ev: &J, topic: &J, utc: &J, draft: &Path) -> HR<J> {
    match get().rebind_campaign_narratives {
        Some(h) => h(repo, ev, topic, utc, draft),
        None => evidence::rebind_campaign_narratives(repo, ev, topic, utc, draft),
    }
}
