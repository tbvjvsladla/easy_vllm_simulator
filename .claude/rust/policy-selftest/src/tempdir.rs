//! `tempfile.TemporaryDirectory()`: a fresh directory under the system temp dir, removed on drop.
use std::path::{Path, PathBuf};

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("tmp{}{}{}", std::process::id(), t % 1_000_000_007, n));
        std::fs::create_dir_all(&p).expect("temp dir");
        TempDir(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
    pub fn s(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Default for TempDir {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
