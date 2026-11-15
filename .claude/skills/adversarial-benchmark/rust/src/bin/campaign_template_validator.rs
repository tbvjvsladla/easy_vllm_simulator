//! Public `campaign_template_validator.py` command line (launcher: terraforming_node/scripts/campaign_template_validator.sh).
//!
//! `--acceptance INSTANCE` is answered natively; every other shape reaches the owner program unchanged.
use std::io::{stderr, stdout};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::ExitCode;
use sweep_stop::campaign_validator::{run_request, Request};
use sweep_stop::campaign_write::{Ctx, Sinks};

/// Exactly `--campaign P`, `--instance P`, `--template`, `--acceptance P` (each at most once, no abbreviation, no `=`).
fn parse(args: &[std::ffi::OsString]) -> Option<Request> {
    let mut request = Request { campaign: None, instance: None, template: false, acceptance: None };
    let mut at = 0;
    while at < args.len() {
        let word = args[at].to_str()?;
        if word == "--template" {
            request.template = true;
            at += 1;
            continue;
        }
        let value = args.get(at + 1)?.to_str()?;
        if value.starts_with('-') {
            return None;
        }
        let slot = match word {
            "--campaign" => &mut request.campaign,
            "--instance" => &mut request.instance,
            "--acceptance" => &mut request.acceptance,
            _ => return None,
        };
        *slot = Some(value.to_string());
        at += 2;
    }
    Some(request)
}

const OWNER_ENV: &str = "CAMPAIGN_VALIDATOR_COMPAT_OWNER";

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let owner = std::env::var_os(OWNER_ENV).filter(|v| !v.is_empty());
    let explicit = std::env::var_os("CAMPAIGN_VALIDATOR_PYTHON").filter(|v| !v.is_empty());
    if explicit.is_none() {
        if let (Some(owner), Some(request)) = (owner.as_ref(), parse(&args)) {
            if let Ok(site) = role_contract::campaign::module_site(owner.as_bytes()) {
                let ctx = Ctx { root: std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&site.root)) };
                let (mut out, mut err) = (stdout().lock(), stderr().lock());
                // a schema the owner cannot read is an exception only the owner can render: it gets the vector unchanged
                if let Ok(status) = run_request(&ctx, &request, &mut Sinks { out: &mut out, err: &mut err }) {
                    return ExitCode::from(u8::try_from(status).unwrap_or(1));
                }
            }
        }
    }
    let Some(owner) = owner else {
        eprintln!("[campaign_template_validator] ERROR {OWNER_ENV} 가 없다 — 런처 없이 실행할 수 없다");
        return ExitCode::from(2);
    };
    let python = explicit
        .or_else(|| std::env::var_os("CAMPAIGN_VALIDATOR_COMPAT_PYTHON").filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "python3".into());
    let error = std::process::Command::new(&python).arg("-B").arg(&owner).args(&args).exec();
    eprintln!("[campaign_template_validator] ERROR 호환 인터프리터를 시작할 수 없다: {error}");
    ExitCode::from(127)
}
