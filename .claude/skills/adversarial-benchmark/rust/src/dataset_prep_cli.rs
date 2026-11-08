//! Command line of `dataset_prep`: the original's argparse interface with three subcommands, `--self-test`, and
//! byte-for-byte usage, help and error text (including the logical program names `dataset_prep.py` and
//! `dataset_prep.py <subcommand>`). The Rust-only surface is the separate `dataset_prep_api`.
//!
//! Standard output is the interpreter's: block-buffered into a pipe or file (so output a child process writes
//! straight to the shared descriptor comes first), flushed after every `print` when it is a terminal. An uncaught
//! exception ends with a traceback marker and `Class: message` on standard error and status 1.

use crate::argv::{
    self, dataset_prepare_args as prepare, dataset_slice_args as slice,
    dataset_synthetic_args as synthetic, Known, Spec, DATASET_PREPARE, DATASET_SLICE,
    DATASET_SYNTHETIC_SIDECAR, DATASET_TOP,
};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::dataset_commands::{cmd_prepare, cmd_slice, cmd_synthetic_sidecar, Namespace};
use crate::dataset_prep::Console;
use crate::dataset_prep_selftest::self_test;
use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::stdout::OutputFailure;
use crate::terminal;

/// The subcommands in the order `add_subparsers` registered them.
const COMMANDS: [&str; 3] = ["prepare", "slice", "synthetic-sidecar"];

/// `sys.stdout` / `sys.stderr` as the command sees them.
pub(crate) struct StreamsConsole<'a, 'b> {
    pub(crate) io: &'a mut Streams<'b>,
}

fn print_failure(failure: &crate::cli::PrintFailure) -> Failure {
    match failure {
        crate::cli::PrintFailure::Encode(encode) => {
            Failure::new("UnicodeEncodeError", &encode.message())
        }
        crate::cli::PrintFailure::Write(OutputFailure { class, message }) => {
            Failure::new(class, message)
        }
    }
}

impl Console for StreamsConsole<'_, '_> {
    fn print(&mut self, line: &PyStr) -> Result<(), Failure> {
        self.io
            .print(line)
            .map_err(|failure| print_failure(&failure))?;
        self.io
            .flush_if_terminal()
            .map_err(|failure| print_failure(&failure))
    }

    /// `sys.stdout.write(text)`: on a terminal the stream is line-buffered, so text with a line break goes out at once.
    fn write(&mut self, text: &PyStr) -> Result<(), Failure> {
        self.io
            .write_plain(text)
            .map_err(|failure| print_failure(&failure))?;
        let breaks_line = text
            .code_points()
            .iter()
            .any(|&point| point == u32::from('\n') || point == u32::from('\r'));
        if breaks_line {
            self.io
                .flush_if_terminal()
                .map_err(|failure| print_failure(&failure))?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Failure> {
        self.io
            .flush_stdout()
            .map_err(|failure| print_failure(&failure))
    }

    fn help(&mut self, text: &str) {
        self.io.print_help(text);
    }

    fn error(&mut self, text: &PyStr) {
        write_stderr(self.io, text);
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name. Deeply nested JSON needs the
/// large stack of [`crate::cli::on_large_stack`]; the binary runs this inside it.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let mut console = StreamsConsole { io };
    let status = match main_with(args, &mut console) {
        Ok(status) => status,
        Err(failure) if failure.class == "SystemExit" => exit_status(&failure),
        Err(failure) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&traceback_line(&failure));
            text.push_str("\n");
            console.error(&text);
            1
        }
    };
    io.finish(status)
}

/// `SystemExit(n)` as a process status: an integer is the status, anything else prints and exits 1.
pub(crate) fn exit_status(failure: &Failure) -> i32 {
    failure
        .message
        .to_utf8()
        .and_then(|text| text.parse::<i64>().ok())
        .map_or(1, |code| (code & 0xff) as i32)
}

/// `Class: message` as the interpreter prints the last traceback line (library classes carry their module).
fn traceback_line(failure: &Failure) -> PyStr {
    let class = match failure.class {
        "JSONDecodeError" => "json.decoder.JSONDecodeError",
        other => other,
    };
    let mut text = PyStr::new(class);
    if !failure.message.is_empty() {
        text.push_str(": ");
        text.push_py(&failure.message);
    }
    text
}

/// What the dispatcher found on the command line.
enum Dispatch {
    Run(&'static str, PyDict),
    Help(&'static Spec),
    Error(&'static Spec, PyStr),
}

fn text_value(text: Option<&PyStr>, default: &str) -> PyValue {
    PyValue::Str(text.cloned().unwrap_or_else(|| PyStr::new(default)))
}

fn namespace_of(command: &str, matches: &argv::Matches) -> PyDict {
    let mut fields = PyDict::new();
    let mut put = |name: &str, action: usize, default: Option<&str>| {
        if let Some(text) = matches.value(action) {
            fields.set(name, PyValue::Str(text.clone()));
        } else if let Some(default) = default {
            fields.set(name, text_value(None, default));
        }
    };
    match command {
        "prepare" => {
            put("spec", prepare::SPEC, None);
            put("tokstage", prepare::TOKSTAGE, None);
            put("cache", prepare::CACHE, None);
            put("image", prepare::IMAGE, None);
            put("budget_mib", prepare::BUDGET_MIB, None);
            put("input_len", prepare::INPUT_LEN, None);
            put("output_len", prepare::OUTPUT_LEN, None);
            put("timeout_s", prepare::TIMEOUT_S, Some("1800"));
        }
        "slice" => {
            put("spec", slice::SPEC, None);
            put("cache", slice::CACHE, None);
            put("run_index", slice::RUN_INDEX, None);
            put("count", slice::COUNT, None);
            put("input_len", slice::INPUT_LEN, None);
            put("output_len", slice::OUTPUT_LEN, None);
            put("out", slice::OUT, None);
            put("sidecar", slice::SIDECAR, None);
            put("tokenizer_label", slice::TOKENIZER_LABEL, None);
            put("sweep_salt", slice::SWEEP_SALT, Some(""));
        }
        _ => {
            put("input_len", synthetic::INPUT_LEN, None);
            put("output_len", synthetic::OUTPUT_LEN, None);
            put("sidecar", synthetic::SIDECAR, None);
        }
    }
    fields
}

/// `ap.parse_args(argv)`: the top-level parser takes its own options up to the first word, and that word (the
/// subcommand) hands the rest of the line to its parser. Leftovers of either are refused by the top-level parser.
fn dispatch(args: &[PyStr]) -> Dispatch {
    let dashes = PyStr::new("--");
    let mut command_at = None;
    for (index, argument) in args.iter().enumerate() {
        if *argument == dashes {
            command_at = Some(index);
            break;
        }
        match argv::is_option_like(&DATASET_TOP, argument) {
            Err(message) => return Dispatch::Error(&DATASET_TOP, message),
            Ok(false) => {
                command_at = Some(index);
                break;
            }
            Ok(true) => {}
        }
    }
    let end = command_at.unwrap_or(args.len());
    let mut extras = match argv::parse_known_with(&DATASET_TOP, &args[..end]) {
        Known::Help => return Dispatch::Help(&DATASET_TOP),
        Known::Error(message) => return Dispatch::Error(&DATASET_TOP, message),
        Known::Run(_, extras) => extras,
    };
    let required = || {
        Dispatch::Error(
            &DATASET_TOP,
            PyStr::new("the following arguments are required: cmd"),
        )
    };
    let Some(at) = command_at else {
        return required();
    };
    if args[at] == dashes && at + 1 == args.len() {
        return required();
    }
    let Some(command) = COMMANDS.iter().find(|name| args[at].equals(name)) else {
        let choices: Vec<String> = COMMANDS
            .iter()
            .map(|name| PyStr::new(name).repr())
            .collect();
        let mut message = PyStr::new("argument cmd: invalid choice: ");
        message.push_str(&args[at].repr());
        message.push_str(&format!(" (choose from {})", choices.join(", ")));
        return Dispatch::Error(&DATASET_TOP, message);
    };
    let sub: &'static Spec = match *command {
        "prepare" => &DATASET_PREPARE,
        "slice" => &DATASET_SLICE,
        _ => &DATASET_SYNTHETIC_SIDECAR,
    };
    match argv::parse_known_with(sub, &args[at + 1..]) {
        Known::Help => Dispatch::Help(sub),
        Known::Error(message) => Dispatch::Error(sub, message),
        Known::Run(matches, more) => {
            extras.extend(more);
            if extras.is_empty() {
                Dispatch::Run(command, namespace_of(command, &matches))
            } else {
                Dispatch::Error(&DATASET_TOP, argv::unrecognized(&extras))
            }
        }
    }
}

/// `main(argv)`: the exit status, or the exception that ends the program (`SystemExit` included).
pub fn main_with(args: &[PyStr], console: &mut dyn Console) -> Result<i32, Failure> {
    if args.len() == 1 && args[0].equals("--self-test") {
        return self_test(console);
    }
    match dispatch(args) {
        Dispatch::Help(spec) => {
            console.help(spec.help(terminal::columns()));
            Err(Failure::new("SystemExit", "0"))
        }
        Dispatch::Error(spec, message) => {
            let mut text = PyStr::new(spec.usage(terminal::columns()));
            text.push_str(spec.prog);
            text.push_str(": error: ");
            text.push_py(&message);
            text.push_str("\n");
            console.error(&text);
            Err(Failure::new("SystemExit", "2"))
        }
        Dispatch::Run(command, fields) => {
            let namespace = Namespace(&fields);
            match command {
                "prepare" => cmd_prepare(&namespace, console),
                "slice" => cmd_slice(&namespace, console),
                _ => cmd_synthetic_sidecar(&namespace, console),
            }
        }
    }
}
