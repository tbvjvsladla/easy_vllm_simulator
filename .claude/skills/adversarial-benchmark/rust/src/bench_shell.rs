//! `bench_shell`: the native replacement of the small Python programs the benchmark shell scripts started inline
//! (`python3 -c ...` and `python3 - <<'PY'`). Each former program is one named, typed operation with the arguments,
//! environment variables and standard input the script handed it; none of them is a query language.
//!
//! An operation reads what the original read, in the original's order, and its observable result is the original's:
//! the printed lines, the files it writes (including the part of a file an interrupted `json.dump` leaves), the exit
//! status and, for an exception nothing catches, `Class: message` after the traceback marker. Values keep Python's
//! truthiness, `str()` and `int()` rules through [`PyValue`], so an odd document gives the answer Python gave.
//!
//! The operations that apply a rule another native owner already holds call that owner (the bench-mode record
//! reader, the events-scan summary, the lite-verdict reader, the GPU key, the dataset argument builders); no rule is
//! restated here.

mod broad;
mod document;
mod envelope;
mod projection;
mod run_metrics;
mod selftest;
mod sweep_path;

use crate::assemble_sweep_meta::{current_utc_text, EnvSource, ProcessEnv};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pystr::PyStr;

pub(crate) const PROGRAM: &str = "bench_shell";
const USAGE: &str = "usage: bench_shell OPERATION [ARGUMENT ...] | --self-test\n";

/// The named operations of the program, each one a former inline Python program of the shell scripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    BroadInit,
    BroadCell,
    /// plan_26111708 B L12: the cell record of a standalone `sweep_bench.sh --campaign-cell` (same builder as `broad-cell`).
    CampaignCellRecord,
    ProvenanceStatus,
    MeasurementOk,
    BroadMeasurementOk,
    CellOutcome,
    EventReason,
    LiteRawStatus,
    VoidReasonSource,
    NodeId,
    EventsScanned,
    JudgeModelTp,
    JudgeLog,
    Verdict,
    Authority,
    JsonEscapeStdin,
    LiteVerdict,
    MaxLevel,
    MaxIndex,
    DatasetKind,
    ImageRef,
    ToolSidecar,
    WarmFraction,
    FloorMib,
    ColumnMapper,
    PrefixSnapshot,
    PrefixRecord,
    SweepPath,
    ExtractDataset,
    DryKnobs,
    PriorLevels,
    ReadLiteVerdict,
    BenchModeLog,
    DataArg,
}

const OPERATIONS: [(&str, Operation); 35] = [
    ("broad-init", Operation::BroadInit),
    ("broad-cell", Operation::BroadCell),
    ("campaign-cell-record", Operation::CampaignCellRecord),
    ("provenance-status", Operation::ProvenanceStatus),
    ("measurement-ok", Operation::MeasurementOk),
    ("broad-measurement-ok", Operation::BroadMeasurementOk),
    ("cell-outcome", Operation::CellOutcome),
    ("event-reason", Operation::EventReason),
    ("lite-raw-status", Operation::LiteRawStatus),
    ("void-reason-source", Operation::VoidReasonSource),
    ("node-id", Operation::NodeId),
    ("events-scanned", Operation::EventsScanned),
    ("judge-model-tp", Operation::JudgeModelTp),
    ("judge-log", Operation::JudgeLog),
    ("verdict", Operation::Verdict),
    ("authority", Operation::Authority),
    ("json-escape-stdin", Operation::JsonEscapeStdin),
    ("lite-verdict", Operation::LiteVerdict),
    ("max-level", Operation::MaxLevel),
    ("max-index", Operation::MaxIndex),
    ("dataset-kind", Operation::DatasetKind),
    ("image-ref", Operation::ImageRef),
    ("tool-sidecar", Operation::ToolSidecar),
    ("warm-fraction", Operation::WarmFraction),
    ("floor-mib", Operation::FloorMib),
    ("column-mapper", Operation::ColumnMapper),
    ("prefix-snapshot", Operation::PrefixSnapshot),
    ("prefix-record", Operation::PrefixRecord),
    ("sweep-path", Operation::SweepPath),
    ("extract-dataset", Operation::ExtractDataset),
    ("dry-knobs", Operation::DryKnobs),
    ("prior-levels", Operation::PriorLevels),
    ("read-lite-verdict", Operation::ReadLiteVerdict),
    ("bench-mode-log", Operation::BenchModeLog),
    ("data-arg", Operation::DataArg),
];

/// The names of every operation, in the order the program documents them.
pub fn operation_names() -> impl Iterator<Item = &'static str> {
    OPERATIONS.iter().map(|&(name, _)| name)
}

impl Operation {
    pub fn named(name: &PyStr) -> Option<Operation> {
        OPERATIONS
            .iter()
            .find(|(own, _)| name.equals(own))
            .map(|&(_, operation)| operation)
    }

    pub fn name(self) -> &'static str {
        OPERATIONS
            .iter()
            .find(|&&(_, own)| own == self)
            .map(|&(name, _)| name)
            .expect("every operation is listed")
    }

    /// Whether the original read standard input.
    fn reads_stdin(self) -> bool {
        matches!(
            self,
            Operation::ProvenanceStatus
                | Operation::EventReason
                | Operation::LiteRawStatus
                | Operation::VoidReasonSource
                | Operation::EventsScanned
                | Operation::JsonEscapeStdin
                | Operation::ImageRef
                | Operation::ToolSidecar
                | Operation::FloorMib
                | Operation::PrefixSnapshot
                | Operation::BenchModeLog
        )
    }
}

/// How an operation ended before its last line: `SystemExit(status)` or an exception nothing catches.
#[derive(Debug)]
pub(crate) enum Stop {
    Exit(i32),
    Raise(Failure),
}

impl From<Failure> for Stop {
    fn from(failure: Failure) -> Self {
        Stop::Raise(failure)
    }
}

pub(crate) type Done = Result<(), Stop>;

/// What an operation has printed so far: standard output as lines (`print`), standard error as text.
#[derive(Default)]
pub struct Console {
    lines: Vec<PyStr>,
    errors: PyStr,
}

impl Console {
    /// `print(text)`.
    pub(crate) fn print(&mut self, text: PyStr) {
        self.lines.push(text);
    }

    /// `sys.stderr.write(text)`.
    pub(crate) fn warn(&mut self, text: &str) {
        self.errors.push_str(text);
    }

    pub fn stdout_lines(&self) -> &[PyStr] {
        &self.lines
    }

    pub fn stderr_text(&self) -> &PyStr {
        &self.errors
    }
}

/// Everything an operation may look at: its arguments (after the operation name), the environment, the decoded
/// standard input (empty for an operation that does not read it) and the clock.
pub struct Context<'a> {
    pub args: &'a [PyStr],
    pub env: &'a dyn EnvSource,
    pub stdin: PyStr,
    pub clock: &'a dyn Fn() -> String,
}

impl Context<'_> {
    /// `sys.argv[position + 1]`.
    pub(crate) fn argument(&self, position: usize) -> Result<&PyStr, Failure> {
        self.args
            .get(position)
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))
    }

    /// `os.environ[name]`.
    pub(crate) fn required(&self, name: &str) -> Result<PyStr, Failure> {
        self.env
            .get(name)
            .ok_or_else(|| Failure::with_message("KeyError", PyStr::new(&PyStr::new(name).repr())))
    }

    /// `os.environ.get(name) or None`: unset and empty are both nothing.
    pub(crate) fn non_empty(&self, name: &str) -> Option<PyStr> {
        self.env.get(name).filter(|value| !value.is_empty())
    }
}

/// How a whole operation ended.
#[derive(Debug, PartialEq)]
pub enum Ending {
    /// The last line was reached: status 0.
    Finished,
    /// `sys.exit(status)` or `raise SystemExit(status)`: the status, nothing more to say.
    Exit(i32),
    /// An exception nothing catches: the traceback marker and `Class: message`, status 1.
    Raised(Failure),
}

/// Runs one operation to its end; the console holds what was printed whether or not it finished.
pub fn execute(operation: Operation, context: &Context<'_>, console: &mut Console) -> Ending {
    match perform(operation, context, console) {
        Ok(()) | Err(Stop::Exit(0)) => Ending::Finished,
        Err(Stop::Exit(status)) => Ending::Exit(status),
        Err(Stop::Raise(failure)) => Ending::Raised(failure),
    }
}

fn perform(operation: Operation, context: &Context<'_>, console: &mut Console) -> Done {
    match operation {
        Operation::BroadInit => broad::init(context, console),
        Operation::BroadCell => broad::cell(context, console),
        Operation::CampaignCellRecord => broad::campaign_cell(context, console),
        Operation::ProvenanceStatus => projection::provenance_status(context, console),
        Operation::MeasurementOk => projection::measurement_ok(context),
        Operation::BroadMeasurementOk => projection::broad_measurement_ok(context),
        Operation::CellOutcome => projection::cell_outcome(context, console),
        Operation::EventReason => projection::event_reason(context, console),
        Operation::LiteRawStatus => projection::stdin_member(context, console, "lite_raw_status"),
        Operation::VoidReasonSource => {
            projection::stdin_member(context, console, "void_reason_source")
        }
        Operation::NodeId => projection::file_member(context, console, "node_id"),
        Operation::EventsScanned => projection::events_scanned(context, console),
        Operation::JudgeModelTp => projection::judge_model_tp(context, console),
        Operation::JudgeLog => projection::judge_log(context, console),
        Operation::Verdict => projection::file_member(context, console, "verdict"),
        Operation::Authority => projection::authority(context, console),
        Operation::JsonEscapeStdin => projection::json_escape_stdin(context, console),
        Operation::LiteVerdict => projection::lite_verdict(context, console),
        Operation::MaxLevel => envelope::max_level(context, console),
        Operation::MaxIndex => envelope::max_index(context, console),
        Operation::DatasetKind => projection::dataset_kind(context, console),
        Operation::ImageRef => projection::stdin_required(context, console, "image_ref"),
        Operation::ToolSidecar => projection::tool_sidecar(context),
        Operation::WarmFraction => run_metrics::warm_fraction(context, console),
        Operation::FloorMib => projection::stdin_required(context, console, "floor_mib"),
        Operation::ColumnMapper => projection::column_mapper(context, console),
        Operation::PrefixSnapshot => run_metrics::prefix_snapshot(context, console),
        Operation::PrefixRecord => run_metrics::prefix_record(context, console),
        Operation::SweepPath => sweep_path::sweep_path(context, console),
        Operation::ExtractDataset => projection::extract_dataset(context),
        Operation::DryKnobs => projection::dry_knobs(context, console),
        Operation::PriorLevels => projection::prior_levels(context, console),
        Operation::ReadLiteVerdict => projection::read_lite_verdict(context, console),
        Operation::BenchModeLog => projection::bench_mode_log(context, console),
        Operation::DataArg => projection::data_arg(context, console),
    }
}

/// The name a traceback prints for the class of a failure.
fn traceback_class(class: &str) -> String {
    match class {
        "JSONDecodeError" => "json.decoder.JSONDecodeError".to_string(),
        other => other.to_string(),
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let Some(first) = args.first() else {
        write_stderr(io, &PyStr::new(USAGE));
        return 2;
    };
    if first.equals("--self-test") && args.len() == 1 {
        return selftest::run(io);
    }
    let Some(operation) = Operation::named(first) else {
        let shown = first.to_utf8().unwrap_or_default();
        write_stderr(
            io,
            &PyStr::new(&format!(
                "{USAGE}[{PROGRAM}] ERROR 알 수 없는 연산: {shown}\n"
            )),
        );
        return 2;
    };
    let stdin = if operation.reads_stdin() {
        match read_stdin(&mut *io.stdin) {
            Ok(text) => text,
            // Every operation that reads standard input does so before anything else it prints or writes, so a failed
            // read ends the run with nothing done, as the interpreter's `sys.stdin.read()` did.
            Err(failure) => return emit(io, &Console::default(), Ending::Raised(failure)),
        }
    } else {
        PyStr::default()
    };
    let context = Context {
        args: &args[1..],
        env: &ProcessEnv,
        stdin,
        clock: &current_utc_text,
    };
    let mut console = Console::default();
    let ended = execute(operation, &context, &mut console);
    emit(io, &console, ended)
}

/// `sys.stdin.read()`: everything to the end, decoded as UTF-8 with `surrogateescape` and no newline translation
/// (CPython opens `sys.stdin` with `newline="\n"` on POSIX). A read that fails, whatever was read before it, raises the
/// operating-system error as Python words it (`IsADirectoryError: [Errno 21] Is a directory`); the bytes read so far are
/// dropped, as the interpreter drops them.
fn read_stdin(reader: &mut dyn std::io::Read) -> Result<PyStr, Failure> {
    let mut bytes = Vec::new();
    match reader.read_to_end(&mut bytes) {
        Ok(_) => Ok(PyStr::from_surrogateescape(&bytes)),
        Err(error) => Err(crate::json_stream::stream_failure(&error)),
    }
}

/// Writes what the operation printed and turns its ending into the exit status, as the interpreter would.
pub(crate) fn emit(io: &mut Streams<'_>, console: &Console, ended: Ending) -> i32 {
    let mut stderr_so_far = console.errors.clone();
    for line in &console.lines {
        if let Err(failure) = io.print(line) {
            stderr_so_far.push_str(TRACEBACK_HEADER);
            stderr_so_far.push_str(&format!("{}\n", failure.line()));
            write_stderr(io, &stderr_so_far);
            return 1;
        }
    }
    let status = match ended {
        Ending::Finished => 0,
        Ending::Exit(status) => status,
        Ending::Raised(failure) => {
            stderr_so_far.push_str(TRACEBACK_HEADER);
            stderr_so_far.push_str(&traceback_class(failure.class));
            if !failure.message.is_empty() {
                stderr_so_far.push_str(": ");
                stderr_so_far.push_py(&failure.message);
            }
            stderr_so_far.push_str("\n");
            1
        }
    };
    write_stderr(io, &stderr_so_far);
    status
}

#[cfg(test)]
mod tests {
    use super::read_stdin;
    use std::io::{Error, Read};

    /// Gives its bytes, then fails.
    struct FailsAfter<'a>(&'a [u8]);

    impl Read for FailsAfter<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() {
                return Err(Error::from_raw_os_error(21));
            }
            let count = self.0.len().min(buffer.len());
            buffer[..count].copy_from_slice(&self.0[..count]);
            self.0 = &self.0[count..];
            Ok(count)
        }
    }

    #[test]
    fn a_read_error_after_some_bytes_is_the_python_error_and_the_bytes_are_dropped() {
        let failure = read_stdin(&mut FailsAfter(b"partial\n")).expect_err("the read fails");
        assert_eq!(failure.class, "IsADirectoryError");
        assert_eq!(
            failure.message.to_utf8().unwrap(),
            "[Errno 21] Is a directory"
        );
    }

    #[test]
    fn a_complete_read_keeps_carriage_returns_and_undecodable_bytes() {
        let text = read_stdin(&mut &b"a\r\n\xff"[..]).expect("reads");
        assert_eq!(text.code_points(), [0x61, 0x0D, 0x0A, 0xDCFF]);
    }

    #[test]
    fn other_error_numbers_use_their_python_classes() {
        struct Fails(i32);
        impl Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(Error::from_raw_os_error(self.0))
            }
        }
        let failure = read_stdin(&mut Fails(5)).expect_err("EIO");
        assert_eq!(
            (failure.class, failure.message.to_utf8().unwrap()),
            ("OSError", "[Errno 5] Input/output error".to_string())
        );
    }
}
