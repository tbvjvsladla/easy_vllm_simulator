//! Descriptor-relative, symlink-refusing filesystem primitives (the `os.open(..., dir_fd=)` / `os.lstat(..., dir_fd=)` /
//! `os.listdir(fd)` calls of completion_gate.py), plus Python's `OSError` text.

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// An owned file descriptor, closed on drop.
pub struct Fd(pub libc::c_int);

impl Drop for Fd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

impl Fd {
    pub fn raw(&self) -> libc::c_int {
        self.0
    }
    pub fn dup(&self) -> Result<Fd, i32> {
        let fd = unsafe { libc::fcntl(self.0, libc::F_DUPFD_CLOEXEC, 0) };
        if fd < 0 {
            Err(errno())
        } else {
            Ok(Fd(fd))
        }
    }
}

pub fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

pub fn strerror(code: i32) -> String {
    unsafe {
        let p = libc::strerror(code);
        if p.is_null() {
            return format!("Unknown error {code}");
        }
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// `str(OSError)` with a filename: `[Errno N] text: 'name'`.
pub fn os_error_text(code: i32, filename: &str) -> String {
    format!("[Errno {code}] {}: {}", strerror(code), crate::pytext::repr(filename))
}

fn cstr(bytes: &[u8]) -> Option<CString> {
    CString::new(bytes.to_vec()).ok()
}

/// `os.open(path, flags)` (Python adds O_CLOEXEC).
pub fn open_path(path: &Path, flags: libc::c_int) -> Result<Fd, i32> {
    let c = cstr(path.as_os_str().as_bytes()).ok_or(libc::EINVAL)?;
    let fd = unsafe { libc::open(c.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        Err(errno())
    } else {
        Ok(Fd(fd))
    }
}

/// `os.open(name, flags, dir_fd=dir)`.
pub fn openat(dir: libc::c_int, name: &[u8], flags: libc::c_int) -> Result<Fd, i32> {
    let c = cstr(name).ok_or(libc::EINVAL)?;
    let fd = unsafe { libc::openat(dir, c.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        Err(errno())
    } else {
        Ok(Fd(fd))
    }
}

pub fn fstat(fd: libc::c_int) -> Result<libc::stat, i32> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        Err(errno())
    } else {
        Ok(st)
    }
}

/// `os.lstat(name, dir_fd=dir)`.
pub fn lstatat(dir: libc::c_int, name: &[u8]) -> Result<libc::stat, i32> {
    let c = cstr(name).ok_or(libc::EINVAL)?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatat(dir, c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } != 0 {
        Err(errno())
    } else {
        Ok(st)
    }
}

pub fn is_dir(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFDIR
}
pub fn is_reg(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFREG
}
pub fn is_lnk(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFLNK
}

/// `os.listdir(fd)`: entry names (raw bytes) without `.`/`..`, in directory order.
pub fn listdir_fd(fd: libc::c_int) -> Result<Vec<Vec<u8>>, i32> {
    let dupfd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if dupfd < 0 {
        return Err(errno());
    }
    let dir = unsafe { libc::fdopendir(dupfd) };
    if dir.is_null() {
        let e = errno();
        unsafe { libc::close(dupfd) };
        return Err(e);
    }
    unsafe { libc::rewinddir(dir) };
    let mut out = Vec::new();
    loop {
        unsafe { *libc::__errno_location() = 0 };
        let ent = unsafe { libc::readdir(dir) };
        if ent.is_null() {
            let e = errno();
            unsafe { libc::closedir(dir) };
            if e != 0 {
                return Err(e);
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) }.to_bytes().to_vec();
        if name == b"." || name == b".." {
            continue;
        }
        out.push(name);
    }
    Ok(out)
}

/// Read a whole fd from its current position through a duplicate (the caller's descriptor is untouched).
pub fn read_all_dup(fd: &Fd) -> Result<Vec<u8>, i32> {
    use std::io::Read;
    use std::os::unix::io::FromRawFd;
    let d = fd.dup()?;
    let raw = d.0;
    std::mem::forget(d);
    let mut f = unsafe { std::fs::File::from_raw_fd(raw) };
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.raw_os_error().unwrap_or(libc::EIO))?;
    Ok(buf)
}
