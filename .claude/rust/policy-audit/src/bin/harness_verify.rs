use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let argv0 = argv.first().cloned().unwrap_or_default();
    let repo = policy_audit::harness::repo_from(&argv0);
    let prog = policy_audit::cli::prog_name(&argv0, "harness_verify.sh");
    let code = policy_audit::harness::main_with(&argv[1..], &repo, &prog);
    ExitCode::from((code & 0xff) as u8)
}
