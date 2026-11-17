//! Public `campaign_template_validator` command line (launcher: terraforming_node/scripts/campaign_template_validator.sh): the
//! retired `campaign_template_validator.py` (`[-h] [--campaign C] [--instance I] [--template] [--acceptance INSTANCE]
//! [--selftest]`), answered natively - argparse's surface (`-h`, usage errors, abbreviations, `--x=y`), `--selftest`, and the
//! template / campaign / instance / acceptance checks. No shape starts a Python interpreter.
//!
//! Residuals (documented): the help/usage text is argparse's 80-column layout whatever the terminal width; an exception the
//! original did not handle (an unreadable or non-object document where the original raised) is reported as
//! `[campaign_template_validator] ERROR ...` with status 1 instead of a Python traceback.
use std::io::{stderr, stdout, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use sweep_stop::campaign_args::{parse_table, K};
use sweep_stop::campaign_validator::{run_request, Request};
use sweep_stop::campaign_write::{Ctx, Sinks};

const OPTIONS: [(&str, K, &[&str]); 7] = [
    ("-h", K::Help, &[]),
    ("--help", K::Help, &[]),
    ("--campaign", K::Value, &[]),
    ("--instance", K::Value, &[]),
    ("--template", K::Flag, &[]),
    ("--acceptance", K::Value, &[]),
    ("--selftest", K::Flag, &[]),
];

/// `campaign_template_validator.py -h` at 80 columns (captured from the original).
const HELP: &str = "usage: campaign_template_validator.sh [-h] [--campaign CAMPAIGN]
                                      [--instance INSTANCE] [--template]
                                      [--acceptance INSTANCE] [--selftest]

campaign_template_validator.sh — 캠페인 선언·인스턴스의 결정론 검증기 (fail-closed).

왜 있는가(plan_26090616): 캠페인의 단계 사이에서 정보를 나르던 것이 **대화 기억**이었다. 세션이
끊기거나 문맥이 압축되면 앞 단계의 결정이 사라졌고, 각 스킬은 자기 기본값(루트 `config.yaml`·루트
`tasks/`)으로 되돌아가 산출물을 관리범위 밖에 흘렸다. 처방은 나를 것을 파일로 만드는 것이고,
이 파일은 **그 파일이 실제로 채워졌는지**를 묻는다.

무엇을 검사하지 않는가: 해시. 뼈대는 추적물이라 git 이 이미 바이트를 들고, 인스턴스는 휘발이라
대조할 두 번째 자리가 없다(`policy:GIT_SINGLE_AUTHORITY` 2문항). 사용자 결정 2026-09-06:
\"SHA256 같은 무결성 검증은 필요 없다\".

  검증:  campaign_template_validator.sh --campaign campaigns/<id>/campaign.yaml
  뼈대:  campaign_template_validator.sh --template
  단위:  campaign_template_validator.sh --selftest

options:
  -h, --help            show this help message and exit
  --campaign CAMPAIGN   검증할 campaign.yaml 경로
  --instance INSTANCE   검증할 campaigns/<id>/ 디렉터리
  --template            뼈대 자체를 검증
  --acceptance INSTANCE
                        합격 술어 P4(동시 착수 타임라인)·P5(서브 자기저작)·P6(셀 출처 표시)만 판정한다 —
                        purge 게이트와 분리된 자리다(진행을 막지 않는다)
  --selftest
";

/// The program path the checkout root is derived from (exported by the launcher; the file need not exist).
const OWNER_ENV: &str = "CAMPAIGN_VALIDATOR_COMPAT_OWNER";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let namespace = match parse_table(&OPTIONS, &argv) {
        Ok(Some(namespace)) => namespace,
        Ok(None) => {
            let _ = stdout().lock().write_all(HELP.as_bytes());
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            let usage = &HELP[..=HELP.find("\n\n").expect("usage paragraph")];
            let _ = write!(stderr().lock(), "{usage}campaign_template_validator.sh: error: {message}\n");
            return ExitCode::from(2);
        }
    };
    let last = |name: &str| namespace.get(name).and_then(|v| v.last()).cloned().filter(|v| !v.is_empty());
    let (mut out, mut err) = (stdout().lock(), stderr().lock());
    let mut io = Sinks { out: &mut out, err: &mut err };
    if namespace.contains_key("--selftest") {
        let status = sweep_stop::campaign_validator::selftest(&mut io);
        return ExitCode::from(u8::try_from(status).unwrap_or(1));
    }
    let Some(owner) = std::env::var_os(OWNER_ENV).filter(|v| !v.is_empty()) else {
        io.warn(&format!("[campaign_template_validator] ERROR {OWNER_ENV} 가 없다 — 런처 없이 실행할 수 없다"));
        return ExitCode::from(2);
    };
    let Ok(site) = role_contract::campaign::module_site(owner.as_bytes()) else {
        io.warn("[campaign_template_validator] ERROR 프로그램 경로에서 체크아웃 루트를 파생하지 못했다");
        return ExitCode::from(2);
    };
    let ctx = Ctx { root: std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&site.root)) };
    let request = Request {
        campaign: last("--campaign"),
        instance: last("--instance"),
        template: namespace.contains_key("--template"),
        acceptance: last("--acceptance"),
    };
    match run_request(&ctx, &request, &mut io) {
        Ok(status) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(()) => {
            io.warn("[campaign_template_validator] ERROR 이 입력은 원본에서 처리되지 않는 예외로 끝나는 형태다(읽을 수 없거나 객체가 아닌 문서) — 네이티브는 traceback 을 재현하지 않는다");
            ExitCode::from(1)
        }
    }
}
