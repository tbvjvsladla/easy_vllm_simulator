//! The deployed boundary of the sweep-index producer, as the verifiers see it: the caller binding in `sweep_bench.sh`,
//! the launcher `assemble_sweep_meta.sh`, the installed program behind it, and one real run of that chain.
//!
//! The checks that need the producer (`selftest_sweep_meta`, the lite report check, the `full ⊇ lite` conformance
//! checker) do not link the library to get its answers: they bind to the installation the way production does and
//! run the launcher. [`DeployedProducer::bind`] is where a missing or broken producer stops them: the caller no longer
//! calls the launcher, a Python program that still assembles the index is hidden in the script, the launcher is
//! gone, or the program behind it does not start.

use crate::assemble_sweep_meta::ENV_NAMES;
use crate::captured_process::{captured_text, run_captured_in, Captured, ChildSettings};
use crate::conformance::Raised;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SWEEP_SCRIPT: &str = "sweep_bench.sh";
const LAUNCHER: &str = "assemble_sweep_meta.sh";
/// The call `sweep_bench.sh` makes: the launcher beside it, with the environment of the sweep.
const CALLER_BINDING: &str = "bash \"$SDIR/assemble_sweep_meta.sh\"";
const PROGRAM_OPEN: &str = "python3 - <<'PY'\n";
const PROGRAM_CLOSE: &str = "\nPY\n";
const OVERRIDE_NAME: &str = "ASSEMBLE_SWEEP_META_BIN";
/// The wall-clock limit of one producer call.
const PRODUCER_SECONDS: u64 = 60;

fn system_exit(program: &str, message: &str) -> Raised {
    Raised::new("SystemExit", &format!("[{program}] {message}"))
}

/// Every `python3 - <<'PY'` program embedded in the script (the text between an opening marker and the next closing
/// marker).
fn embedded_programs(script: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(opening) = script[from..].find(PROGRAM_OPEN) {
        let body_start = from + opening + PROGRAM_OPEN.len();
        match script[body_start..].find(PROGRAM_CLOSE) {
            Some(length) => {
                found.push(&script[body_start..body_start + length]);
                from = body_start + length + PROGRAM_CLOSE.len();
            }
            None => break,
        }
    }
    found
}

/// Whether `sweep_bench.sh` is bound to the producer: it calls the launcher beside it, and no Python program that
/// assembles the index is left in it (a dead copy of the old producer would satisfy a reader that only looks for the
/// old text). The one owner of this rule: the binding at start-up and the conformance check both ask it.
pub fn caller_binding(script: &str) -> Result<(), String> {
    if !script.contains(CALLER_BINDING) {
        return Err(format!(
            "sweep_bench.sh 가 index 조립 생성부를 호출하지 않는다({CALLER_BINDING} 부재)"
        ));
    }
    if embedded_programs(script)
        .iter()
        .any(|body| body.contains("sweep_index.json") && body.contains("json.dump"))
    {
        return Err(
            "sweep_bench.sh 에 index 를 조립하는 옛 python heredoc 이 남아 있다(호출 뒤의 죽은 생성부)"
                .to_string(),
        );
    }
    Ok(())
}

/// What one producer call left: its status and both streams as bytes.
pub struct ProducerRun {
    pub status: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl From<Captured> for ProducerRun {
    fn from(captured: Captured) -> Self {
        ProducerRun {
            status: captured.status,
            stdout: captured.stdout,
            stderr: captured.stderr,
        }
    }
}

/// The installed producer the verifiers bind to.
pub struct DeployedProducer {
    scripts_dir: PathBuf,
}

impl DeployedProducer {
    /// Binds to the installation whose scripts directory is given, or stops with a `SystemExit` naming what is wrong.
    /// `program` is the name of the verifier in the message.
    pub fn bind(scripts_dir: &Path, program: &str) -> Result<DeployedProducer, Raised> {
        let script = crate::verify_fixture::read_text(&scripts_dir.join(SWEEP_SCRIPT))?;
        if let Err(reason) = caller_binding(&script) {
            return Err(system_exit(program, &format!("{reason} — 중단")));
        }
        DeployedProducer::bind_installed(scripts_dir, program)
    }

    /// The installed chain without the caller: the launcher is there, follows the install convention, and the program
    /// behind it starts and answers its own start-up check. The conformance checker judges the caller in a row of its
    /// own and asks only this of the producer, so a caller that has gone does not hide what the producer writes.
    pub fn bind_installed(scripts_dir: &Path, program: &str) -> Result<DeployedProducer, Raised> {
        let launcher = scripts_dir.join(LAUNCHER);
        let launcher_text = match crate::verify_fixture::read_text(&launcher) {
            Ok(text) => text,
            Err(_) => {
                return Err(system_exit(
                    program,
                    &format!("{LAUNCHER} 런처가 설치돼 있지 않다 — 중단"),
                ));
            }
        };
        if !launcher_text.contains(OVERRIDE_NAME) {
            return Err(system_exit(
                program,
                &format!("{LAUNCHER} 가 {OVERRIDE_NAME} 설치 규약을 따르지 않는다 — 중단"),
            ));
        }
        let producer = DeployedProducer {
            scripts_dir: scripts_dir.to_path_buf(),
        };
        let started = producer.start_up(&["--self-test"])?;
        let stdout = String::from_utf8_lossy(&started.stdout).into_owned();
        if started.status != 0 || !stdout.contains("self-test OK") {
            let stderr = String::from_utf8_lossy(&started.stderr).into_owned();
            return Err(system_exit(
                program,
                &format!(
                    "native index 생성부가 기동하지 않는다(rc={}): {} — 중단",
                    started.status,
                    crate::verify_fixture::tail(stderr.trim(), 1000)
                ),
            ));
        }
        Ok(producer)
    }

    fn command(&self) -> Vec<OsString> {
        vec![
            "bash".into(),
            self.scripts_dir.join(LAUNCHER).into_os_string(),
        ]
    }

    fn start_up(&self, arguments: &[&str]) -> Result<ProducerRun, Raised> {
        let mut command = self.command();
        command.extend(arguments.iter().map(OsString::from));
        let shown: Vec<PyValue> = command
            .iter()
            .map(|word| PyValue::Str(crate::verify_fixture::path_text(Path::new(word))))
            .collect();
        let settings = ChildSettings {
            env_remove: ENV_NAMES.iter().map(OsString::from).collect(),
            ..ChildSettings::default()
        };
        run_captured_in(
            &command,
            &shown,
            Duration::from_secs(PRODUCER_SECONDS),
            &PRODUCER_SECONDS.to_string(),
            &settings,
        )
        .map(ProducerRun::from)
        .map_err(Raised::from_failure)
    }

    /// One real run of the launcher with exactly these producer variables: every name the producer reads is removed
    /// from the inherited environment first, so nothing but `environment` reaches it.
    pub fn run(&self, environment: &[(&str, String)]) -> Result<ProducerRun, Raised> {
        let command = self.command();
        let shown: Vec<PyValue> = command
            .iter()
            .map(|word| PyValue::Str(crate::verify_fixture::path_text(Path::new(word))))
            .collect();
        let settings = ChildSettings {
            env_remove: ENV_NAMES.iter().map(OsString::from).collect(),
            env_set: environment
                .iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                .collect(),
            ..ChildSettings::default()
        };
        run_captured_in(
            &command,
            &shown,
            Duration::from_secs(PRODUCER_SECONDS),
            &PRODUCER_SECONDS.to_string(),
            &settings,
        )
        .map(ProducerRun::from)
        .map_err(Raised::from_failure)
    }
}

impl ProducerRun {
    /// The text of a stream as a verifier prints it.
    pub fn text(bytes: &[u8]) -> Result<PyStr, Raised> {
        captured_text(bytes).map_err(Raised::from_failure)
    }
}
