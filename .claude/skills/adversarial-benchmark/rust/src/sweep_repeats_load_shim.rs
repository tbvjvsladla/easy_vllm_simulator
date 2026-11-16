//! The load tool stand-ins of the repeat-sweep sandbox ([`crate::sweep_repeats_fixture::Sandbox`]): what the sandbox's
//! `run_bench.sh` and `lite_bench.sh` shims do, as a native program (`selftest_load_shim`) so that no program in the
//! sandbox runs Python (2026-11-16 · plan_26111615). They never touch a GPU, docker or a server.
//!
//! * `run-bench` — one GuideLLM level run: from the shipped GuideLLM sample it writes `guidellm_<cfg>.json` with the
//!   planned median TPOT and stream count, an engine log and a post-health record into the run directory; the plan
//!   (`SHIM_PLAN.fail["<conc>:<run>"]`) can instead inject a blackbox kill/trip (event line + exit 3), a bare failure
//!   (exit 3), a blocked run-boundary record (`norecord`) or an incomplete measurement (`measurement`).
//! * `lite-plan` — prints the lite gate plan the `lite_bench.sh` shim follows: `SHIM_PLAN.lite.verdict` (default `pass`)
//!   on the first line and `SHIM_PLAN.lite.rc` (default `0`) on the second.
//!
//! The arguments and environment are those the former Python bodies read (`SHIM_ROOT`, `SHIM_CONFIG`, `SHIM_CONC`,
//! `SHIM_OUT`, `SHIM_PLAN`); the files keep their names, keys and order.

use crate::pyjson::loads;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The shipped GuideLLM sample, relative to the checkout.
const SAMPLE_REL: &str =
    ".claude/skills/adversarial-benchmark/fixtures/guidellm_benchmarks_sample.json";

fn fail(message: &str) -> i32 {
    eprintln!("[selftest_load_shim] {message}");
    2
}

fn env_text(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// `json.loads(os.environ.get("SHIM_PLAN") or "{}")`.
fn plan() -> Result<PyValue, String> {
    let text = env_text("SHIM_PLAN").filter(|text| !text.is_empty());
    loads(text.as_deref().unwrap_or("{}")).map_err(|error| format!("SHIM_PLAN: {error}"))
}

/// `(plan.get(name) or {})` as a dict view (a missing, falsy or non-dict entry is empty).
fn section<'a>(plan: &'a PyValue, name: &str) -> Option<&'a PyDict> {
    match plan.peel() {
        PyValue::Dict(map) => match map.get(name).map(PyValue::peel) {
            Some(PyValue::Dict(inner)) if !inner.is_empty() => Some(inner),
            _ => None,
        },
        _ => None,
    }
}

/// What `print(value)` writes for a JSON scalar.
fn printed(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().unwrap_or_default(),
        other => other.display_text().to_utf8().unwrap_or_default(),
    }
}

fn lite_plan() -> i32 {
    let plan = match plan() {
        Ok(plan) => plan,
        Err(message) => return fail(&message),
    };
    let lite = section(&plan, "lite");
    let verdict = lite
        .and_then(|map| map.get("verdict"))
        .map_or_else(|| "pass".to_string(), printed);
    let rc = lite
        .and_then(|map| map.get("rc"))
        .map_or_else(|| "0".to_string(), printed);
    println!("{verdict}\n{rc}");
    0
}

fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn dumps(value: &PyValue) -> Result<String, String> {
    value.dumps_ascii().map_err(|error| error.to_string())
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

/// The run number of a run directory `.../run_KK` (1 when the name has no such form).
fn run_number(out: &Path) -> u64 {
    out.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("run_"))
        .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(1)
}

fn dict_mut<'a>(value: &'a mut PyValue, key: &str) -> Result<&'a mut PyValue, String> {
    match value {
        PyValue::Dict(map) => map
            .get_mut(key)
            .ok_or_else(|| format!("the GuideLLM sample has no {key:?}")),
        _ => Err(format!("the GuideLLM sample is not an object at {key:?}")),
    }
}

fn run_bench() -> Result<i32, String> {
    let required = |name: &str| env_text(name).ok_or_else(|| format!("{name} is not set"));
    let root = PathBuf::from(required("SHIM_ROOT")?);
    let config = required("SHIM_CONFIG")?;
    let concurrency: i64 = required("SHIM_CONC")?
        .trim()
        .parse()
        .map_err(|_| "SHIM_CONC is not an integer".to_string())?;
    let out = PathBuf::from(required("SHIM_OUT")?);
    let run = run_number(&out);
    let plan = plan()?;
    let key = format!("{concurrency}:{run}");
    let action = section(&plan, "fail")
        .and_then(|map| map.get(&key))
        .map(printed);
    let action = action.as_deref();
    if matches!(action, Some("kill" | "trip")) {
        let events = root.join("docs/logs/main/events/2026-09.jsonl");
        std::fs::create_dir_all(events.parent().expect("an events directory"))
            .map_err(|error| error.to_string())?;
        let kind = if action == Some("kill") {
            "watchdog_kill_ack"
        } else {
            "thermal_trip"
        };
        let mut line = PyDict::new();
        line.set("ts", PyValue::str(&utc_now()));
        line.set("kind", PyValue::str(kind));
        line.set("source", PyValue::str("selftest-shim"));
        line.set("mode", PyValue::str("armed"));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events)
            .map_err(|error| format!("{}: {error}", events.display()))?;
        writeln!(file, "{}", dumps(&PyValue::Dict(line))?).map_err(|error| error.to_string())?;
        return Ok(3);
    }
    if action == Some("rc") {
        return Ok(3);
    }
    if action == Some("norecord") {
        // A run-boundary record that cannot be written: its place is blocked by a directory (the measurement is normal).
        std::fs::create_dir_all(out.join("repeat_run.json")).map_err(|error| error.to_string())?;
    }
    let sample = root.join(SAMPLE_REL);
    let text = std::fs::read_to_string(&sample)
        .map_err(|error| format!("{}: {error}", sample.display()))?;
    let mut document = loads(&text).map_err(|error| error.to_string())?;
    if let PyValue::Dict(metadata) = dict_mut(&mut document, "metadata")? {
        metadata.set("guidellm_version", PyValue::str("0.0.0+selftest-shim"));
    }
    let tpots: Vec<PyValue> = section(&plan, "tpot")
        .and_then(|map| map.get(&concurrency.to_string()))
        .and_then(|value| match value.peel() {
            PyValue::List(items) if !items.is_empty() => Some(items.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            vec![
                PyValue::Float(25.0),
                PyValue::Float(24.0),
                PyValue::Float(26.0),
            ]
        });
    let index = usize::try_from((run - 1) % tpots.len() as u64).unwrap_or(0);
    let tpot = match tpots[index].peel() {
        PyValue::Float(number) => *number,
        PyValue::Int(number) => num_traits::ToPrimitive::to_f64(number).unwrap_or(f64::NAN),
        PyValue::Bool(flag) => f64::from(u8::from(*flag)),
        other => return Err(format!("a planned TPOT is a {}", other.type_name())),
    };
    let benchmarks = dict_mut(&mut document, "benchmarks")?;
    let PyValue::List(items) = benchmarks else {
        return Err("the GuideLLM sample's benchmarks is not a list".to_string());
    };
    let benchmark = items
        .first_mut()
        .ok_or_else(|| "the GuideLLM sample has no benchmark".to_string())?;
    {
        let metrics = dict_mut(benchmark, "metrics")?;
        let successful = dict_mut(dict_mut(metrics, "time_per_output_token_ms")?, "successful")?;
        if let PyValue::Dict(map) = successful {
            map.set("median", PyValue::Float(tpot));
        }
        if action == Some("measurement") {
            if let PyValue::Dict(totals) = dict_mut(metrics, "request_totals")? {
                totals.set("incomplete", PyValue::int(2));
            }
        }
    }
    if let PyValue::Dict(profile) = dict_mut(dict_mut(benchmark, "config")?, "profile")? {
        profile.set("streams", PyValue::List(vec![PyValue::int(concurrency)]));
    }
    write_file(
        &out.join(format!("guidellm_{config}.json")),
        &dumps(&document)?,
    )?;
    write_file(
        &out.join(format!("engine_{config}.log")),
        "INFO generation throughput: 40.0 tokens/s\n",
    )?;
    let mut health = PyDict::new();
    health.set("server_alive_at_bench_end", PyValue::Bool(true));
    health.set("provenance", PyValue::str("selftest-shim"));
    write_file(
        &out.join(format!("post_health_{config}.json")),
        &dumps(&PyValue::Dict(health))?,
    )?;
    Ok(0)
}

/// The program: `selftest_load_shim run-bench` or `selftest_load_shim lite-plan`.
pub fn main(args: &[PyStr]) -> i32 {
    match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("run-bench") => run_bench().unwrap_or_else(|message| fail(&message)),
        Some("lite-plan") => lite_plan(),
        _ => fail("usage: selftest_load_shim run-bench | lite-plan"),
    }
}
