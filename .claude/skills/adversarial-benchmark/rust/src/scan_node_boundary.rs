//! The process boundary of the scan-node P1 migration: the real file reads and the local clock that the T library
//! (`role_contract::scan_node*`) takes as explicit arguments. Nothing here decides anything: [`StdFiles`] answers `exists`/`read`
//! with the operating system's own errno and message, and [`LocalClock`] answers the system-zone hour (`datetime.now().strftime("%Y%m%d%H")`)
//! with the C library's `localtime_r`, exactly the source of Python's `datetime.now()`. A check supplies [`FixedClock`] with a
//! literal hour and its provenance instead of pretending to be the current time.

use agent_runtime_support::pystr::PyStr;
use role_contract::scan_node_files::{FileFault, Files};
use role_contract::scan_node_manifest::Clock;
use std::ffi::{c_char, c_int, c_long};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The real file boundary.
pub struct StdFiles;

/// `strerror(errno)`: the operating system's text for the error (Rust appends ` (os error N)`, which `OSError` does not carry).
fn strerror(error: &std::io::Error) -> (i32, String) {
    let errno = error.raw_os_error().unwrap_or(0);
    let shown = error.to_string();
    let suffix = format!(" (os error {errno})");
    let message = shown.strip_suffix(&suffix).unwrap_or(&shown).to_string();
    (errno, message)
}

impl Files for StdFiles {
    fn exists(&self, path: &[u8]) -> bool {
        std::fs::metadata(Path::new(std::ffi::OsStr::from_bytes(path))).is_ok()
    }

    fn read(&self, path: &[u8]) -> Result<Vec<u8>, FileFault> {
        std::fs::read(Path::new(std::ffi::OsStr::from_bytes(path))).map_err(|error| {
            let (errno, strerror) = strerror(&error);
            FileFault { errno, strerror }
        })
    }
}

#[repr(C)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

extern "C" {
    fn time(out: *mut i64) -> i64;
    fn tzset();
    fn localtime_r(seconds: *const i64, out: *mut Tm) -> *mut Tm;
}

/// The system-zone wall clock (the zone comes from `TZ` / `/etc/localtime`, as it does for the interpreter).
pub struct LocalClock;

impl Clock for LocalClock {
    fn local_hour(&self) -> PyStr {
        // SAFETY: `time` writes one i64; `tzset` takes no argument; `localtime_r` fills the caller-owned `Tm` (glibc layout of
        // `struct tm`, identical on aarch64 and x86_64) from a valid time value, and `Tm` is plain data.
        let (year, month, day, hour) = unsafe {
            tzset();
            let mut seconds = 0_i64;
            time(&mut seconds);
            let mut parts = std::mem::MaybeUninit::<Tm>::zeroed();
            if localtime_r(&seconds, parts.as_mut_ptr()).is_null() {
                return PyStr::new("");
            }
            let parts = parts.assume_init();
            (
                parts.tm_year + 1900,
                parts.tm_mon + 1,
                parts.tm_mday,
                parts.tm_hour,
            )
        };
        PyStr::new(&format!("{year:04}{month:02}{day:02}{hour:02}"))
    }
}

/// A clock a check supplies: a literal hour and the provenance it came from (never presented as the current time).
pub struct FixedClock {
    pub hour: String,
    pub provenance: &'static str,
}

impl Clock for FixedClock {
    fn local_hour(&self) -> PyStr {
        PyStr::new(&self.hour)
    }
}
