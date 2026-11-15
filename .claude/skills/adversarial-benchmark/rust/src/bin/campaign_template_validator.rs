//! Public `campaign_template_validator.py` command line (launcher: terraforming_node/scripts/campaign_template_validator.sh).
//!
//! `--acceptance INSTANCE` is answered natively; every other shape reaches the owner program unchanged.
use std::io::{stderr, stdout};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::ExitCode;
use sweep_stop::campaign_validator::acceptance;
use sweep_stop::campaign_write::{Ctx, Sinks};

const OWNER_ENV: &str = "CAMPAIGN_VALIDATOR_COMPAT_OWNER";

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let owner = std::env::var_os(OWNER_ENV).filter(|v| !v.is_empty());
    let explicit = std::env::var_os("CAMPAIGN_VALIDATOR_PYTHON").filter(|v| !v.is_empty());
    if explicit.is_none() {
        if let (Some(owner), [flag, instance]) = (owner.as_ref(), args.as_slice()) {
            if flag == "--acceptance" && !instance.as_bytes().starts_with(b"-") {
                if let (Ok(site), Some(instance)) = (
                    role_contract::campaign::module_site(owner.as_bytes()),
                    instance.to_str(),
                ) {
                    let ctx = Ctx { root: std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&site.root)) };
                    let (mut out, mut err) = (stdout().lock(), stderr().lock());
                    let status = acceptance(&ctx, instance, &mut Sinks { out: &mut out, err: &mut err });
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
