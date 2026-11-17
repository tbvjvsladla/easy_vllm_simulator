use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let argv0 = argv.first().cloned().unwrap_or_default();
    let repo = policy_audit::cli::repo_root_from(&argv0, "policy_registry.sh");
    let prog = policy_audit::cli::prog_name(&argv0, "policy_registry.sh");
    let code = policy_audit::cli::main_with(&argv[1..], &repo, &prog);
    ExitCode::from((code & 0xff) as u8)
}
