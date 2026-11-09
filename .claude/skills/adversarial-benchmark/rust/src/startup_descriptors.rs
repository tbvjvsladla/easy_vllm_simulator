//! The state of the standard descriptors when the process STARTED, as a mechanical B-side boundary. Rust's own start-up re-opens a closed
//! descriptor 0, 1 or 2 as `/dev/null` before `main`, so a program can no longer tell "closed" from "the caller's `/dev/null`" afterwards
//! (a Python interpreter started with descriptor 1 closed has `sys.stdout = None`; with `/dev/null` it has a real stream). A program that must
//! reproduce that difference installs [`record_standard_descriptors`] as a load-time constructor (see `src/bin/turn_budget.rs`: a `.init_array`
//! entry runs before the runtime's start-up) and reads [`closed_at_start`]. The probe is `fcntl(fd, F_GETFD)` and nothing else: it changes no
//! descriptor, decides nothing about any command and is not part of any business library.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static CLOSED_BITS: AtomicU8 = AtomicU8::new(0);
static RECORDED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}

const F_GETFD: i32 = 1;
const EBADF: i32 = 9;

/// Records which of the descriptors 0, 1, 2 are not open (`EBADF`). Safe to call more than once; it must run before the Rust runtime start-up
/// to see the caller's descriptors.
pub fn record_standard_descriptors() {
    let mut bits = 0u8;
    for fd in 0..3 {
        // SAFETY: `fcntl(fd, F_GETFD)` takes no pointer and only reads the descriptor flags of an integer, whatever it names.
        let result = unsafe { fcntl(fd, F_GETFD) };
        if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(EBADF) {
            bits |= 1 << fd;
        }
    }
    CLOSED_BITS.store(bits, Ordering::SeqCst);
    RECORDED.store(true, Ordering::SeqCst);
}

/// Whether descriptor `fd` (0..=2) was closed when the process started; `None` when the program did not install the load-time recorder.
pub fn closed_at_start(fd: i32) -> Option<bool> {
    if !RECORDED.load(Ordering::SeqCst) || !(0..3).contains(&fd) {
        return None;
    }
    Some(CLOSED_BITS.load(Ordering::SeqCst) & (1 << fd) != 0)
}
