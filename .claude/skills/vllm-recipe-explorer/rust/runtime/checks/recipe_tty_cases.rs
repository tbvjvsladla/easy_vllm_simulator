//! The terminal width the programs lay `--help` out for, checked on a real private pseudo-terminal.
//!
//! The expected 40-column help (`data/tty_help_40.stdout`, 778 bytes) is the saved output of the original `quant_table.py --help`
//! run once on a 40-column terminal with `COLUMNS` and `LINES` unset; the original is never run here. Each case gives the child
//! the slave side of a fresh pseudo-terminal as its standard output (raw mode, so the bytes are not rewritten) and compares
//! what it prints with that file or with the same program run on a pipe.

use std::ffi::{c_char, c_int, c_ulong, c_void};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::{Command, Stdio};

const TIOCSWINSZ: c_ulong = 0x5414;
const O_RDWR: c_int = 2;
const O_NOCTTY: c_int = 0o400;
/// `O_CLOEXEC` is 0o2000000 in the generic Linux numbering both supported architectures (x86_64, aarch64) share.
const O_CLOEXEC: c_int = 0o2_000_000;
const F_GETFD: c_int = 1;
const FD_CLOEXEC: c_int = 1;

extern "C" {
    fn posix_openpt(flags: c_int) -> c_int;
    fn grantpt(fd: c_int) -> c_int;
    fn unlockpt(fd: c_int) -> c_int;
    fn ptsname_r(fd: c_int, buffer: *mut c_char, length: usize) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn tcgetattr(fd: c_int, termios: *mut c_void) -> c_int;
    fn tcsetattr(fd: c_int, action: c_int, termios: *const c_void) -> c_int;
    fn cfmakeraw(termios: *mut c_void);
}

/// Room for any libc `struct termios` (60 bytes on the supported targets), aligned for its integer fields.
#[repr(C, align(8))]
struct TermiosSpace([u8; 256]);

/// A new pseudo-terminal master, owned by the returned `File`. `flags` go to `posix_openpt`, so `O_CLOEXEC` makes the descriptor
/// close-on-exec from the moment it exists (no window between creating and flagging it, which matters because the tests run on
/// several threads and any of them may be spawning a child).
fn new_master(flags: c_int) -> File {
    // SAFETY: `posix_openpt` returns a new descriptor nothing else owns (checked >= 0), which the `File` takes over and closes
    // exactly once; `grantpt` and `unlockpt` take that open descriptor and have no other argument.
    unsafe {
        let raw = posix_openpt(flags);
        assert!(raw >= 0, "posix_openpt failed");
        let master = File::from_raw_fd(raw);
        assert_eq!(grantpt(raw), 0);
        assert_eq!(unlockpt(raw), 0);
        master
    }
}

/// Whether the descriptor is close-on-exec.
fn is_close_on_exec(file: &File) -> bool {
    // SAFETY: `F_GETFD` takes no further argument and only reads the flags of the open descriptor `file` owns.
    let flags = unsafe { fcntl(file.as_raw_fd(), F_GETFD) };
    assert!(flags >= 0, "fcntl(F_GETFD) failed");
    flags & FD_CLOEXEC != 0
}

/// The output of `program args` with its standard output on a pseudo-terminal of `columns` x 24, its standard error on a pipe
/// and `COLUMNS`/`LINES` as given: (exit code, terminal bytes, standard error bytes).
fn on_terminal(
    program: &str,
    args: &[&str],
    columns: u16,
    columns_env: Option<&str>,
) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    // The master must not reach the child: a close-on-exec master is closed by the kernel at the child's `exec`.
    let master = new_master(O_RDWR | O_NOCTTY | O_CLOEXEC);
    assert!(is_close_on_exec(&master), "the master is not close-on-exec");
    // SAFETY: every call below follows its libc contract. `ptsname_r` writes at most `length` bytes (a NUL-terminated name)
    // into the 128-byte local buffer it is given, for the open master descriptor `master` owns; the slave descriptor is owned
    // by a `File`; `ioctl(TIOCSWINSZ)` reads one `struct winsize` (four `u16`) from the local array of that layout;
    // `tcgetattr`/`cfmakeraw`/`tcsetattr` use a 256-byte, 8-aligned buffer larger than any `struct termios`.
    let slave = unsafe {
        let raw = master.as_raw_fd();
        let mut name = [0 as c_char; 128];
        assert_eq!(ptsname_r(raw, name.as_mut_ptr(), name.len()), 0);
        let path = std::ffi::CStr::from_ptr(name.as_ptr())
            .to_str()
            .unwrap()
            .to_string();
        let slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let size: [u16; 4] = [24, columns, 0, 0];
        assert_eq!(ioctl(slave.as_raw_fd(), TIOCSWINSZ, size.as_ptr()), 0);
        let mut termios = TermiosSpace([0; 256]);
        let space = termios.0.as_mut_ptr().cast::<c_void>();
        assert_eq!(tcgetattr(slave.as_raw_fd(), space), 0);
        cfmakeraw(space);
        assert_eq!(tcsetattr(slave.as_raw_fd(), 0, space), 0);
        slave
    };
    assert!(
        is_close_on_exec(&slave),
        "the slave is not close-on-exec before it becomes the child's output"
    );
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped());
    if let Some(value) = columns_env {
        command.env("COLUMNS", value);
    }
    let child = command.spawn().unwrap();
    // The parent's copy of the slave must be gone, or reading the master never ends.
    drop(command);
    let mut master = master;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
            // Linux reports the closed slave as EIO once the child has exited.
            Err(error) if error.raw_os_error() == Some(5) => break,
            Err(error) => panic!("reading the terminal: {error}"),
        }
    }
    let output = child.wait_with_output().unwrap();
    (output.status.code(), bytes, output.stderr)
}

/// The same program with its standard output on a pipe.
fn on_pipe(
    program: &str,
    args: &[&str],
    columns_env: Option<&str>,
) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let mut command = Command::new(program);
    command.args(args).env_clear().stdin(Stdio::null());
    if let Some(value) = columns_env {
        command.env("COLUMNS", value);
    }
    let output = command.output().unwrap();
    (output.status.code(), output.stdout, output.stderr)
}

const QUANT: &str = env!("CARGO_BIN_EXE_quant_table");
const ESTIMATE: &str = env!("CARGO_BIN_EXE_estimate_vram");
const RANK: &str = env!("CARGO_BIN_EXE_rank_recipes");
const ORIGINAL_40: &[u8] = include_bytes!("data/tty_help_40.stdout");

#[test]
fn the_frozen_original_layout_is_the_40_column_one() {
    assert_eq!(ORIGINAL_40.len(), 778);
    assert!(ORIGINAL_40
        .starts_with(b"usage: quant_table.py [-h]\n                      [--quant QUANT]\n"));
}

#[test]
fn an_unset_environment_takes_the_width_of_the_terminal_on_standard_output() {
    let (code, bytes, error) = on_terminal(QUANT, &["--help"], 40, None);
    assert_eq!(code, Some(0));
    assert!(error.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(ORIGINAL_40)
    );
    assert_eq!(bytes, ORIGINAL_40);
}

#[test]
fn a_positive_columns_value_wins_over_the_terminal() {
    // The terminal is 120 wide, COLUMNS says 40: argparse lays out for 40, as `shutil.get_terminal_size` does.
    for value in ["40", " 40 ", "+40", "4_0"] {
        let (code, bytes, _) = on_terminal(QUANT, &["--help"], 120, Some(value));
        assert_eq!(
            (code, bytes.as_slice()),
            (Some(0), ORIGINAL_40),
            "COLUMNS={value:?}"
        );
    }
}

#[test]
fn an_unusable_columns_value_falls_through_to_the_terminal_width() {
    for value in ["", "abc", "0", "-5", "4.0", "0x28", "40 40"] {
        let (code, bytes, _) = on_terminal(QUANT, &["--help"], 40, Some(value));
        assert_eq!(
            (code, bytes.as_slice()),
            (Some(0), ORIGINAL_40),
            "COLUMNS={value:?}"
        );
    }
}

#[test]
fn an_eighty_column_terminal_and_a_pipe_give_the_same_help() {
    let pipe = on_pipe(QUANT, &["--help"], None);
    assert_eq!(pipe.0, Some(0));
    assert_ne!(pipe.1.as_slice(), ORIGINAL_40);
    assert_eq!(on_terminal(QUANT, &["--help"], 80, None), pipe);
    // A terminal that reports no width has nothing to give, so the fallback of 80 applies.
    assert_eq!(on_terminal(QUANT, &["--help"], 0, None), pipe);
    // A pipe with an unusable COLUMNS has no terminal to ask either.
    assert_eq!(on_pipe(QUANT, &["--help"], Some("junk")), pipe);
}

#[test]
fn the_error_message_wraps_for_the_terminal_even_when_only_output_is_one() {
    // A missing required argument prints its usage line to standard error; CPython sizes it from the standard-output terminal.
    let on_40 = on_terminal(ESTIMATE, &[], 40, None);
    let pipe_40 = on_pipe(ESTIMATE, &[], Some("40"));
    assert_eq!(on_40.0, Some(2));
    assert_eq!(on_40, pipe_40);
    let on_120 = on_terminal(ESTIMATE, &[], 120, None);
    assert_ne!(on_120.2, on_40.2, "the width really changed the message");
    assert_eq!(on_120, on_pipe(ESTIMATE, &[], Some("120")));
}

#[test]
fn every_program_asks_the_terminal_and_a_text_result_reaches_it_unchanged() {
    for program in [ESTIMATE, RANK] {
        let terminal = on_terminal(program, &["--help"], 40, None);
        assert_eq!(
            terminal,
            on_pipe(program, &["--help"], Some("40")),
            "{program}"
        );
    }
    // Line-buffered or not, the printed lines are the same bytes: one `key: value` line per print.
    let args = ["--serve-bpw", "3.5", "--quant", "NONE"];
    let pipe = on_pipe(QUANT, &args, None);
    assert_eq!(pipe.0, Some(0));
    assert_eq!(
        String::from_utf8_lossy(&pipe.1),
        "quant: NONE\nserve_bpw: 3.5\nvllm_quant_bpw: 3.5\n"
    );
    assert_eq!(on_terminal(QUANT, &args, 40, None), pipe);
}

/// How many of this process's own descriptors are the pseudo-terminal multiplexer (`/dev/ptmx`, which is what every master is).
fn own_ptmx_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            std::fs::read_link(entry.path())
                .is_ok_and(|target| target == std::path::Path::new("/dev/ptmx"))
        })
        .count()
}

/// Runs one `ignore`d test of this same test executable as a fresh process (arguments only, no shell, no environment protocol)
/// and returns what it printed after `key`. These tests are never run by themselves; the shared harness process starts them.
fn run_ignored_child(name: &str, key: &str) -> String {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            name,
            "--nocapture",
            "--test-threads=1",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        // `--nocapture` prints the value after the harness's own "test ... " prefix on the same line.
        .find_map(|line| {
            line.split_once(key)
                .map(|(_, value)| value.trim().to_string())
        })
        .unwrap_or_else(|| panic!("{name} printed no {key}"))
}

/// Innermost process: reports how many masters it holds. Started only by [`descriptor_control_child`].
#[test]
#[ignore = "child process of descriptor_control_child"]
fn descriptor_probe_child() {
    println!("PTMX_DESCRIPTORS={}", own_ptmx_descriptors());
}

/// How many masters a newly executed grandchild (the probe above) finds open in the calling process's descendants.
fn masters_seen_by_a_probe() -> usize {
    run_ignored_child("descriptor_probe_child", "PTMX_DESCRIPTORS=")
        .parse()
        .unwrap()
}

/// Middle process, and the only one that ever owns the deliberate non-close-on-exec master: the negative control lives in a
/// process of its own, started with `--test-threads=1` and running only this test, so no other test's child can inherit it.
/// Prints `PTMX_OBSERVED=<inherited>,<held>,<leaked>,<after>`: what it received from the shared harness process, what its own probe
/// finds with a close-on-exec master open, with an extra master created without the flag, and after that one is closed.
#[test]
#[ignore = "child process of a_terminal_master_is_not_inherited_by_a_spawned_program"]
fn descriptor_control_child() {
    let inherited = own_ptmx_descriptors();
    let held = new_master(O_RDWR | O_NOCTTY | O_CLOEXEC);
    assert!(is_close_on_exec(&held));
    let with_held = masters_seen_by_a_probe();
    let leaked = new_master(O_RDWR | O_NOCTTY);
    assert!(
        !is_close_on_exec(&leaked),
        "the control master must lack the flag"
    );
    let with_leaked = masters_seen_by_a_probe();
    drop(leaked);
    let after = masters_seen_by_a_probe();
    drop(held);
    println!("PTMX_OBSERVED={inherited},{with_held},{with_leaked},{after}");
}

#[test]
fn a_terminal_master_is_not_inherited_by_a_spawned_program() {
    // The shared harness process opens only close-on-exec masters (the helper) and keeps one open across the whole check.
    let held = new_master(O_RDWR | O_NOCTTY | O_CLOEXEC);
    assert!(is_close_on_exec(&held));
    // The control child reports 0 inherited from here, 0 with its own close-on-exec master, 1 once it has opened one without
    // the flag (so the zeros are observations, not an absent probe), and 0 again after closing that one.
    let observed = run_ignored_child("descriptor_control_child", "PTMX_OBSERVED=");
    assert_eq!(observed, "0,0,1,0", "inherited, held, leaked, after");
    drop(held);
}
