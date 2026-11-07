use agent_runtime_support::pystr::PyStr;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;
use upstream_runtime::cli::run_process;
use upstream_runtime::host::YamlProfile;

fn main() -> ExitCode {
    let args: Vec<PyStr> = std::env::args_os()
        .skip(1)
        .map(|arg| PyStr::from_surrogateescape(arg.as_bytes()))
        .collect();
    let columns =
        recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref());
    // `UPSTREAM_RUNTIME_YAML_PROFILE=flat`: an installation without PyYAML (the bundled PyYAML 6.0.3 otherwise)
    let profile = YamlProfile::from_setting(
        std::env::var("UPSTREAM_RUNTIME_YAML_PROFILE")
            .ok()
            .as_deref(),
    );
    let status = run_process(&args, Some(&columns), profile);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
