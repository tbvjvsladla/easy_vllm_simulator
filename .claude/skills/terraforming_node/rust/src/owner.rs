//! Where the program the executable stands for lives: its `__file__`. The launcher exports `TERRAFORMING_OWNER_PATH`
//! (the `.py` sibling path of `scripts/<program>.sh`, a location, not user intent); without it the path is derived from
//! this executable's place in the repository (`<repo>/.claude/skills/terraforming_node/rust/target/release/<bin>`).

use std::path::{Path, PathBuf};

/// `Path(__file__).resolve()` (the file itself need not exist; its directory is resolved).
pub fn owner_path(rel_from_scripts: &str) -> PathBuf {
    if let Some(p) = std::env::var_os("TERRAFORMING_OWNER_PATH") {
        let p = PathBuf::from(p);
        return resolve(&p);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let repo = exe
        .ancestors()
        .nth(7)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    repo.join(".claude/skills/terraforming_node/scripts")
        .join(rel_from_scripts)
}

fn resolve(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    match std::fs::canonicalize(&abs) {
        Ok(c) => c,
        Err(_) => match (abs.parent(), abs.file_name()) {
            (Some(d), Some(n)) => std::fs::canonicalize(d)
                .unwrap_or_else(|_| d.to_path_buf())
                .join(n),
            _ => abs,
        },
    }
}

/// `Path(__file__).resolve().parents[n]`.
pub fn parents(rel_from_scripts: &str, n: usize) -> PathBuf {
    owner_path(rel_from_scripts)
        .ancestors()
        .nth(n + 1)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}
