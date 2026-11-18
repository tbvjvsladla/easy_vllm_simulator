//! Program names and argument vectors: which static texts start the Python interpreter.
//!
//! A text is one value an expression can take (see `finder`); an argument is the list of texts it can take (empty =
//! not known statically, which never counts as the interpreter).

/// An interpolated part of a `format!` string: `format!("{venv}/bin/python")` reads as `"\0/bin/python"`.
pub const HOLE: char = '\0';

/// Shells whose `-c` argument is a command line.
const SHELLS: [&str; 7] = ["sh", "bash", "dash", "zsh", "ksh", "ash", "mksh"];

/// Programs that run the rest of their argument vector as a command (`env python3 …`, `timeout 9 python3 …`).
const WRAPPERS: [&str; 21] = [
    "env", "nice", "nohup", "timeout", "stdbuf", "setsid", "ionice", "taskset", "chrt", "time",
    "strace", "ltrace", "sudo", "doas", "runuser", "flock", "unshare", "nsenter", "chroot",
    "xargs", "busybox",
];

/// Programs whose arguments are a command line on another host (the sub node).
const REMOTE: [&str; 1] = ["ssh"];

/// How an argument vector reaches the interpreter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The program itself is the interpreter.
    Interpreter,
    /// A shell runs a `-c` command line that starts it.
    Shell,
    /// A wrapper (`env`, `timeout`, `strace`, …) runs it from its arguments.
    Wrapper,
    /// `ssh` runs a remote command line that starts it.
    Remote,
}

/// The file name part of a program text (`/usr/bin/python3` → `python3`).
fn base_name(text: &str) -> &str {
    text.rsplit('/').next().unwrap_or(text)
}

/// `python`, `python3`, `python3.12` with or without a directory: the names T's safe_run counts (`^python[0-9.]*$`).
/// An interpolated version (`python{minor}`) counts too.
pub fn is_interpreter(text: &str) -> bool {
    base_name(text)
        .strip_prefix("python")
        .is_some_and(|version| {
            version
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == HOLE)
        })
}

fn is_shell(text: &str) -> bool {
    SHELLS.contains(&base_name(text))
}

fn is_wrapper(text: &str) -> bool {
    WRAPPERS.contains(&base_name(text))
}

fn is_remote(text: &str) -> bool {
    REMOTE.contains(&base_name(text))
}

/// A short option cluster that names a command line: `-c`, `-lc`, `-ec`.
fn is_command_option(text: &str) -> bool {
    text.strip_prefix('-').is_some_and(|flags| {
        !flags.is_empty() && flags.chars().all(|c| c.is_ascii_alphabetic()) && flags.contains('c')
    })
}

/// Does a command line start the interpreter on one of its lines? Interpolations read as shell expansions.
fn script_starts_interpreter(script: &str) -> bool {
    script.replace(HOLE, "$X").lines().any(crate::starts_python)
}

fn any_text(argument: &[String], test: impl Fn(&str) -> bool) -> bool {
    argument.iter().any(|text| test(text))
}

/// A shell's arguments: the argument after the first `-c` cluster is a command line that starts the interpreter.
fn shell_starts_interpreter(args: &[Vec<String>]) -> bool {
    let Some(at) = args
        .iter()
        .position(|argument| any_text(argument, is_command_option))
    else {
        return false;
    };
    args.get(at + 1)
        .is_some_and(|script| any_text(script, script_starts_interpreter))
}

/// A wrapper's arguments: one of them is the interpreter (an `X=Y` assignment or option value is not a program), or a
/// shell whose own arguments start it.
fn wrapper_starts_interpreter(args: &[Vec<String>]) -> bool {
    args.iter().enumerate().any(|(at, argument)| {
        any_text(argument, |text| !text.contains('=') && is_interpreter(text))
            || (any_text(argument, is_shell) && shell_starts_interpreter(&args[at + 1..]))
    })
}

/// `ssh` joins its arguments into a remote command line: one of them starts the interpreter.
fn remote_starts_interpreter(args: &[Vec<String>]) -> bool {
    args.iter()
        .any(|argument| any_text(argument, script_starts_interpreter))
}

/// The program position of an argument vector other than the interpreter itself: a shell, wrapper or `ssh`.
fn via_launcher(program: &[String], args: &[Vec<String>]) -> Option<Via> {
    if any_text(program, is_shell) && shell_starts_interpreter(args) {
        Some(Via::Shell)
    } else if any_text(program, is_wrapper) && wrapper_starts_interpreter(args) {
        Some(Via::Wrapper)
    } else if any_text(program, is_remote) && remote_starts_interpreter(args) {
        Some(Via::Remote)
    } else {
        None
    }
}

/// A `Command` with this program and these arguments: does it start the interpreter, and how?
pub fn command_starts_interpreter(program: &[String], args: &[Vec<String>]) -> Option<Via> {
    if any_text(program, is_interpreter) {
        return Some(Via::Interpreter);
    }
    via_launcher(program, args)
}

/// An argument-vector array (`["python3", "-m", "venv", dir]`, `["/bin/sh", "-c", "python3 …"]`): does it start the
/// interpreter? A list that only names the interpreter (`["python3", "python"]`, a list of forbidden words) is not an
/// argument vector: the interpreter in front must be followed by an interpreter argument (an option or a `.py` path).
pub fn argv_starts_interpreter(items: &[Vec<String>]) -> Option<Via> {
    let (program, args) = items.split_first()?;
    let interpreter_argument = |text: &str| text.starts_with('-') || text.ends_with(".py");
    if any_text(program, is_interpreter)
        && args
            .first()
            .is_some_and(|first| any_text(first, interpreter_argument))
    {
        return Some(Via::Interpreter);
    }
    via_launcher(program, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<Vec<String>> {
        items.iter().map(|text| vec![text.to_string()]).collect()
    }

    #[test]
    fn interpreter_names_are_the_safe_run_names() {
        for yes in [
            "python",
            "python3",
            "python3.12",
            "/usr/bin/python3",
            "\0/bin/python",
            "python\0",
        ] {
            assert!(is_interpreter(yes), "{yes:?}");
        }
        for no in [
            "pythonic",
            "python3-config",
            "python_semantics-1a2b",
            "py",
            "python3 -c",
            "no-python",
            "",
        ] {
            assert!(!is_interpreter(no), "{no:?}");
        }
    }

    #[test]
    fn argument_vectors() {
        assert_eq!(
            argv_starts_interpreter(&argv(&["python3.12", "-m", "venv"])),
            Some(Via::Interpreter)
        );
        assert_eq!(
            argv_starts_interpreter(&argv(&["python3", "tool.py"])),
            Some(Via::Interpreter)
        );
        assert_eq!(
            argv_starts_interpreter(&argv(&["/bin/sh", "-c", "python3 -c pass"])),
            Some(Via::Shell)
        );
        assert_eq!(
            argv_starts_interpreter(&argv(&[
                "env",
                "-i",
                "PATH=/usr/bin/python3",
                "python3",
                "-V"
            ])),
            Some(Via::Wrapper)
        );
        assert_eq!(
            argv_starts_interpreter(&argv(&["ssh", "sub.invalid", "cd /x && python3 -c 1"])),
            Some(Via::Remote)
        );
        assert_eq!(argv_starts_interpreter(&argv(&["python3", "python"])), None);
        assert_eq!(
            argv_starts_interpreter(&argv(&["/bin/sh", "-c", "echo python3 is gone"])),
            None
        );
        assert_eq!(
            argv_starts_interpreter(&argv(&["env", "PYTHON=python3", "true"])),
            None
        );
        assert_eq!(
            command_starts_interpreter(&["python3".into()], &[]),
            Some(Via::Interpreter)
        );
        assert_eq!(
            command_starts_interpreter(&["bash".into()], &argv(&["-lc", "exec python3 \"$0\""])),
            Some(Via::Shell)
        );
        assert_eq!(
            command_starts_interpreter(&["bash".into()], &argv(&["-c", "echo hi"])),
            None
        );
    }
}
