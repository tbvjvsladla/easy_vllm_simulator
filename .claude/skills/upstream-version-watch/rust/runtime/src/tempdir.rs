//! `tempfile.TemporaryDirectory()` as `wheel_metadata` and the self-test use it (CPython 3.12.3 `tempfile.py`): the default temporary directory is
//! chosen once by a write probe, a directory named `tmp` + eight characters of `a-z0-9_` is made with mode 0700 below it, and it is removed again on
//! every way out.
//!
//! Everything here really happens on the file system (nothing is simulated or tabulated). What the native product did is DECLARED in a
//! [`Provenance`] (`temp_producers`, `metadata_read_attempts`); the declaration is the product's own account of its operations and is never presented
//! as a kernel or interpreter audit.
//!
//! Names come from operating-system entropy (`/dev/urandom`) by rejection sampling; they are not meant to equal any earlier run's names.

use crate::http_direct::PyException;
use crate::py_errors::{os_error_with_filename, with_message};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const NAME_CHARACTERS: &[u8; 37] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
const PREFIX: &str = "tmp";
/// `tempfile.TMP_MAX` for names tried before giving up on a directory name.
const MAX_NAME_TRIES: u32 = 10_000;
const PROBE_TRIES: u32 = 100;

/// One temporary directory the product made and what became of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Producer {
    pub path: String,
    pub created: bool,
    pub cleanup_complete: bool,
}

/// The product's declaration of the operations that matter for comparison: ordered temp producers and ordered metadata read attempts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provenance {
    pub temp_producers: Vec<Producer>,
    pub metadata_read_attempts: Vec<String>,
}

/// Eight characters of the temp-name alphabet from the operating system's entropy.
fn random_name() -> std::io::Result<String> {
    let mut urandom = std::fs::File::open("/dev/urandom")?;
    let mut name = String::with_capacity(8);
    let mut byte = [0u8; 1];
    while name.len() < 8 {
        urandom.read_exact(&mut byte)?;
        // 222 = 6 * 37: values above it would bias the choice
        if byte[0] < 222 {
            name.push(char::from(NAME_CHARACTERS[usize::from(byte[0]) % 37]));
        }
    }
    Ok(name)
}

fn absolute(path: &Path, current: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current.join(path)
    }
}

/// `tempfile.gettempdir()`: the first candidate directory in which a probe file can be created, written and removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempBase {
    directory: PathBuf,
}

impl TempBase {
    /// The candidates of `_candidate_tempdir_list` on POSIX: `TMPDIR`, `TEMP`, `TMP` (when set and not empty), `/tmp`, `/var/tmp`, `/usr/tmp`, the
    /// working directory.
    pub fn candidates_of_process() -> Vec<String> {
        let mut list: Vec<String> = ["TMPDIR", "TEMP", "TMP"]
            .iter()
            .filter_map(std::env::var_os)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        list.extend(["/tmp", "/var/tmp", "/usr/tmp"].map(String::from));
        list.push(
            std::env::current_dir()
                .map(|cwd| cwd.to_string_lossy().into_owned())
                .unwrap_or_else(|_| ".".to_string()),
        );
        list
    }

    /// The process default.
    pub fn of_process() -> Result<TempBase, Box<PyException>> {
        TempBase::probe(&TempBase::candidates_of_process())
    }

    /// `_get_default_tempdir` over `candidates`.
    pub fn probe(candidates: &[String]) -> Result<TempBase, Box<PyException>> {
        let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        for candidate in candidates {
            let directory = if candidate == "." {
                PathBuf::from(".")
            } else {
                absolute(Path::new(candidate), &current)
            };
            for _ in 0..PROBE_TRIES {
                let Ok(name) = random_name() else { break };
                let file = directory.join(&name);
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&file)
                {
                    Ok(mut handle) => {
                        let wrote = handle.write_all(b"blat");
                        drop(handle);
                        let removed = std::fs::remove_file(&file);
                        if wrote.is_ok() && removed.is_ok() {
                            return Ok(TempBase { directory });
                        }
                        break;
                    }
                    Err(error) if error.raw_os_error() == Some(17) => continue,
                    Err(_) => break,
                }
            }
        }
        let listed =
            PyValue::List(candidates.iter().map(|item| PyValue::str(item)).collect()).repr();
        let message = format!("No usable temporary directory found in {listed}");
        let mut error = with_message(
            "FileNotFoundError",
            "builtins",
            &format!("[Errno 2] {message}"),
        );
        error.args = vec![PyValue::int(2).into(), PyValue::str(&message).into()];
        error.attributes = vec![
            ("errno", PyValue::int(2).into()),
            ("strerror", PyValue::str(&message).into()),
            ("filename", PyValue::None.into()),
            ("filename2", PyValue::None.into()),
        ];
        Err(Box::new(error))
    }

    /// A base fixed by the caller (the checks' owned directory), without a probe.
    pub fn fixed(directory: PathBuf) -> TempBase {
        TempBase { directory }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// `tempfile.mkdtemp()` below `base`: mode 0700, the first unused name; only an existing name is retried.
fn make_directory(base: &TempBase) -> Result<PathBuf, Box<PyException>> {
    for _ in 0..MAX_NAME_TRIES {
        let name = random_name().map_err(|failure| {
            os_error_with_filename(&failure, &PyStr::new("/dev/urandom"))
                .unwrap_or_else(|| with_message("OSError", "builtins", "no entropy"))
        })?;
        let path = base.directory.join(format!("{PREFIX}{name}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.raw_os_error() == Some(17) => continue,
            Err(error) => {
                let shown = PyStr::new(&path.to_string_lossy());
                return Err(Box::new(
                    os_error_with_filename(&error, &shown)
                        .unwrap_or_else(|| with_message("OSError", "builtins", &error.to_string())),
                ));
            }
        }
    }
    let mut error = with_message(
        "FileExistsError",
        "builtins",
        "[Errno 17] No usable temporary directory name found",
    );
    error.args = vec![
        PyValue::int(17).into(),
        PyValue::str("No usable temporary directory name found").into(),
    ];
    Err(Box::new(error))
}

/// `with tempfile.TemporaryDirectory() as td: body(td)`: the directory exists while `body` runs and is removed afterwards whatever `body` returned
/// (an error included). The declaration records the directory when it was made and whether its removal completed. A failed removal is raised in
/// place of a successful result, as `shutil.rmtree` inside `__exit__` would; the body's own error is kept when there is one.
pub fn with_temporary_directory<T, E: From<Box<PyException>>>(
    base: &TempBase,
    provenance: &mut Provenance,
    body: impl FnOnce(&Path, &mut Provenance) -> Result<T, E>,
) -> Result<T, E> {
    let directory = make_directory(base)?;
    let index = provenance.temp_producers.len();
    provenance.temp_producers.push(Producer {
        path: directory.to_string_lossy().into_owned(),
        created: true,
        cleanup_complete: false,
    });
    let outcome = body(&directory, provenance);
    let removal = std::fs::remove_dir_all(&directory);
    provenance.temp_producers[index].cleanup_complete = removal.is_ok();
    match (outcome, removal) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => {
            let shown = PyStr::new(&directory.to_string_lossy());
            let raised = os_error_with_filename(&error, &shown)
                .unwrap_or_else(|| with_message("OSError", "builtins", &error.to_string()));
            Err(Box::new(raised).into())
        }
        (Err(error), _) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_base() -> (TempBase, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "tempdir-checks-{}-{}",
            std::process::id(),
            random_name().unwrap()
        ));
        std::fs::create_dir_all(&root).unwrap();
        (TempBase::fixed(root.clone()), root)
    }

    #[test]
    fn names_use_the_alphabet_and_eight_characters() {
        for _ in 0..50 {
            let name = random_name().unwrap();
            assert_eq!(name.len(), 8);
            assert!(name.bytes().all(|byte| NAME_CHARACTERS.contains(&byte)));
        }
        assert_ne!(random_name().unwrap(), random_name().unwrap());
    }

    #[test]
    fn the_directory_exists_with_mode_0700_inside_and_is_gone_afterwards() {
        let (base, root) = owned_base();
        let mut provenance = Provenance::default();
        let mut seen = None;
        let result: Result<u8, Box<PyException>> =
            with_temporary_directory(&base, &mut provenance, |directory, _| {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(directory).unwrap().permissions().mode() & 0o777;
                std::fs::write(directory.join("pkg.whl"), b"x").unwrap();
                seen = Some((directory.to_path_buf(), mode));
                Ok(7)
            });
        assert_eq!(result.unwrap(), 7);
        let (directory, mode) = seen.unwrap();
        assert_eq!(mode, 0o700);
        assert!(!directory.exists());
        let name = directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("tmp") && name.len() == 11);
        assert_eq!(
            provenance.temp_producers,
            vec![Producer {
                path: directory.to_string_lossy().into_owned(),
                created: true,
                cleanup_complete: true
            }]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_directory_is_removed_when_the_body_fails_and_the_failure_is_kept() {
        let (base, root) = owned_base();
        let mut provenance = Provenance::default();
        let result: Result<(), Box<PyException>> =
            with_temporary_directory(&base, &mut provenance, |directory, _| {
                std::fs::write(directory.join("partial"), b"half").unwrap();
                Err(Box::new(with_message(
                    "RuntimeError",
                    "builtins",
                    "body failed",
                )))
            });
        assert_eq!(
            result.unwrap_err().message.to_utf8().unwrap(),
            "body failed"
        );
        assert!(!Path::new(&provenance.temp_producers[0].path).exists());
        assert!(provenance.temp_producers[0].cleanup_complete);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_probe_takes_the_first_usable_candidate_and_leaves_no_probe_file() {
        let (_, root) = owned_base();
        let candidates = vec![
            "/definitely/not/here".to_string(),
            root.to_string_lossy().into_owned(),
        ];
        let base = TempBase::probe(&candidates).unwrap();
        assert_eq!(base.directory(), root.as_path());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let error = TempBase::probe(&["/definitely/not/here".to_string()]).unwrap_err();
        assert_eq!(error.class, "FileNotFoundError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "[Errno 2] No usable temporary directory found in ['/definitely/not/here']"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
