//! The width of the terminal on standard output, asked of the descriptor itself (`os.get_terminal_size(1)` of the original's
//! `shutil.get_terminal_size`). Standard Rust has no such query, so the one Linux request it needs is declared here.

/// The kernel's `struct winsize` (`<asm-generic/termios.h>`): four unsigned 16-bit fields, the same on x86_64 and aarch64.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[repr(C)]
struct WinSize {
    rows: u16,
    columns: u16,
    x_pixels: u16,
    y_pixels: u16,
}

/// `TIOCGWINSZ`: 0x5413 in the generic Linux ioctl numbering used by both supported architectures.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const TIOCGWINSZ: std::os::raw::c_ulong = 0x5413;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
extern "C" {
    fn ioctl(fd: std::os::raw::c_int, request: std::os::raw::c_ulong, ...) -> std::os::raw::c_int;
}

/// The column count the terminal behind descriptor 1 reports, or `None` when it is not a terminal (or on a platform
/// without the request). A terminal that reports 0 columns is returned as `Some(0)`; the caller applies its fallback.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub fn stdout_columns() -> Option<u16> {
    let mut size = WinSize {
        rows: 0,
        columns: 0,
        x_pixels: 0,
        y_pixels: 0,
    };
    // SAFETY: `ioctl(fd, TIOCGWINSZ, p)` writes at most `sizeof(struct winsize)` bytes through `p`. `p` points to a live,
    // properly aligned local of exactly that layout (`WinSize` is `repr(C)` with the four `u16` fields of the kernel
    // structure), so the write stays inside it. The descriptor number is a plain integer: if descriptor 1 is closed or is
    // not a terminal the call returns -1 and writes nothing. The variadic declaration matches how libc takes the third argument.
    let result = unsafe { ioctl(1, TIOCGWINSZ, &mut size as *mut WinSize) };
    (result == 0).then_some(size.columns)
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
pub fn stdout_columns() -> Option<u16> {
    None
}
