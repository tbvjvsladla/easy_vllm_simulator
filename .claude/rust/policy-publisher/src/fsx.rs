//! Descriptor-relative file operations (`os.open(..., dir_fd=)`, `os.mkdir(..., dir_fd=)`, `os.lstat(..., dir_fd=)`,
//! `os.replace(..., src_dir_fd=, dst_dir_fd=)`, `os.unlink(..., dir_fd=)`, `os.listdir(fd)`, `fcntl.flock`) and the text
//! of the `OSError` Python would raise (`[Errno N] strerror: 'name'`).

use std::ffi::CString;

pub struct Fd(pub libc::c_int);

impl Drop for Fd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe { libc::close(self.0) };
        }
    }
}
impl Fd {
    pub fn raw(&self) -> libc::c_int {
        self.0
    }
    pub fn dup(&self) -> Result<Fd, i32> {
        let fd = unsafe { libc::dup(self.0) };
        if fd < 0 {
            Err(errno())
        } else {
            Ok(Fd(fd))
        }
    }
    /// Gives the number up without closing (an `os.close` the caller does itself, or a deliberately closed number).
    pub fn into_raw(mut self) -> libc::c_int {
        let n = self.0;
        self.0 = -1;
        n
    }
}

pub fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

pub fn strerror(code: i32) -> String {
    let p = unsafe { libc::strerror(code) };
    if p.is_null() {
        return format!("Unknown error {code}");
    }
    unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// `str(OSError(errno, strerror, filename))`.
pub fn os_error(code: i32, filename: Option<&str>) -> String {
    match filename {
        Some(name) => format!("[Errno {code}] {}: {}", strerror(code), crate::json::repr(name)),
        None => format!("[Errno {code}] {}", strerror(code)),
    }
}

fn c(name: &str) -> CString {
    CString::new(name.as_bytes()).unwrap_or_else(|_| CString::new("\u{fffd}").unwrap())
}

pub fn open_path(path: &str, flags: libc::c_int) -> Result<Fd, i32> {
    let p = c(path);
    let fd = unsafe { libc::open(p.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        Err(errno())
    } else {
        Ok(Fd(fd))
    }
}

pub fn openat(dir: libc::c_int, name: &str, flags: libc::c_int, mode: libc::mode_t) -> Result<Fd, i32> {
    let p = c(name);
    let fd = unsafe { libc::openat(dir, p.as_ptr(), flags | libc::O_CLOEXEC, mode as libc::c_uint) };
    if fd < 0 {
        Err(errno())
    } else {
        Ok(Fd(fd))
    }
}

pub fn mkdirat(dir: libc::c_int, name: &str) -> Result<(), i32> {
    let p = c(name);
    if unsafe { libc::mkdirat(dir, p.as_ptr(), 0o777) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

/// `os.lstat(name, dir_fd=dir)`.
pub fn lstatat(dir: libc::c_int, name: &str) -> Result<libc::stat, i32> {
    let p = c(name);
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatat(dir, p.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } < 0 {
        Err(errno())
    } else {
        Ok(st)
    }
}

pub fn renameat(dir: libc::c_int, from: &str, to: &str) -> Result<(), i32> {
    let a = c(from);
    let b = c(to);
    if unsafe { libc::renameat(dir, a.as_ptr(), dir, b.as_ptr()) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

pub fn unlinkat(dir: libc::c_int, name: &str) -> Result<(), i32> {
    let p = c(name);
    if unsafe { libc::unlinkat(dir, p.as_ptr(), 0) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

pub fn write_all(fd: libc::c_int, mut bytes: &[u8]) -> Result<(), i32> {
    while !bytes.is_empty() {
        let n = unsafe { libc::write(fd, bytes.as_ptr() as *const libc::c_void, bytes.len()) };
        if n < 0 {
            let e = errno();
            if e == libc::EINTR {
                continue;
            }
            return Err(e);
        }
        bytes = &bytes[n as usize..];
    }
    Ok(())
}

pub fn fsync(fd: libc::c_int) -> Result<(), i32> {
    if unsafe { libc::fsync(fd) } < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

pub fn read_to_end(fd: libc::c_int) -> Result<Vec<u8>, i32> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n < 0 {
            let e = errno();
            if e == libc::EINTR {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Ok(out);
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
}

/// One `os.read(fd, limit)` call.
pub fn read_once(fd: libc::c_int, limit: usize) -> Result<Vec<u8>, i32> {
    let mut buf = vec![0u8; limit];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
    if n < 0 {
        return Err(errno());
    }
    buf.truncate(n as usize);
    Ok(buf)
}

pub fn lseek(fd: libc::c_int, whence: libc::c_int) {
    unsafe { libc::lseek(fd, 0, whence) };
}

pub fn flock(fd: libc::c_int, op: libc::c_int) {
    unsafe { libc::flock(fd, op) };
}

/// `os.listdir(fd)` (names decoded with `surrogateescape`; here lossily — names this publisher compares are ASCII/UTF-8).
pub fn listdir_fd(fd: libc::c_int) -> Result<Vec<String>, i32> {
    let dup = unsafe { libc::dup(fd) };
    if dup < 0 {
        return Err(errno());
    }
    let dir = unsafe { libc::fdopendir(dup) };
    if dir.is_null() {
        let e = errno();
        unsafe { libc::close(dup) };
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
            return Ok(out);
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*ent).d_name.as_ptr()) };
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        out.push(String::from_utf8_lossy(bytes).into_owned());
    }
}

pub fn is_lnk(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFLNK
}
pub fn is_reg(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFREG
}
pub fn is_dir(st: &libc::stat) -> bool {
    st.st_mode & libc::S_IFMT == libc::S_IFDIR
}
