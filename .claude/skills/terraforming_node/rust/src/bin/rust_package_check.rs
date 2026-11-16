//! `rust_package_check METADATA_JSON CRATE_DIR` — the post-build check of `build_rust_packages.sh` (formerly a Python
//! heredoc): the crate's root package in `cargo metadata` output must be unique; every normal binary target (no
//! `required-features`) must exist under `target/release` as an executable ELF; every library target must have its
//! `.rlib`. Prints `{"package": ..., "normal_bins": [...], "normal_bin_count": N, "libraries": [...]}`; a failure is one
//! message on standard error and status 1.

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use terraforming_runtime::py;

fn fail(m: &str) -> ! {
    eprintln!("{m}");
    std::process::exit(1)
}

fn arr(v: Option<&PyValue>) -> Vec<PyValue> {
    match v.map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.clone(),
        _ => vec![],
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        fail("usage: rust_package_check METADATA_JSON CRATE_DIR");
    }
    let text =
        std::fs::read_to_string(&args[0]).unwrap_or_else(|e| fail(&format!("{}: {e}", args[0])));
    let data = pyjson::loads(&text).unwrap_or_else(|e| fail(&format!("metadata: {e}")));
    let krate: PathBuf =
        std::fs::canonicalize(&args[1]).unwrap_or_else(|e| fail(&format!("{}: {e}", args[1])));
    let want = krate.join("Cargo.toml");
    let packages: Vec<PyValue> = arr(py::get(&data, "packages"))
        .into_iter()
        .filter(|p| {
            py::get_str(p, "manifest_path")
                .is_some_and(|m| std::fs::canonicalize(&m).is_ok_and(|c| c == want))
        })
        .collect();
    if packages.len() != 1 {
        fail("metadata root package is ambiguous");
    }
    let package = &packages[0];
    let targets = arr(py::get(package, "targets"));
    let has_kind = |t: &PyValue, k: &str| {
        arr(py::get(t, "kind"))
            .iter()
            .any(|x| matches!(x, PyValue::Str(s) if s.equals(k)))
    };
    let mut bins: Vec<String> = targets
        .iter()
        .filter(|t| has_kind(t, "bin") && !py::truthy(py::get(t, "required-features")))
        .filter_map(|t| py::get_str(t, "name"))
        .collect();
    bins.sort();
    for name in &bins {
        let b = krate.join("target/release").join(name);
        let exec = b.is_file()
            && b.metadata()
                .is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
        if !exec {
            fail(&format!(
                "normal binary missing/non-executable: {}",
                b.display()
            ));
        }
        let mut magic = [0u8; 4];
        let ok = std::fs::File::open(&b)
            .and_then(|mut f| f.read_exact(&mut magic))
            .is_ok()
            && &magic == b"\x7fELF";
        if !ok {
            fail(&format!("normal binary is not ELF: {}", b.display()));
        }
    }
    let libraries: Vec<String> = targets
        .iter()
        .filter(|t| has_kind(t, "lib"))
        .filter_map(|t| py::get_str(t, "name"))
        .collect();
    for name in &libraries {
        if !Path::new(&krate.join("target/release").join(format!("lib{name}.rlib"))).is_file() {
            fail(&format!("library missing: {name}"));
        }
    }
    let mut d = PyDict::new();
    d.set(
        "package",
        py::get(package, "name").cloned().unwrap_or(PyValue::None),
    );
    d.set("normal_bins", py::list_str(&bins));
    d.set("normal_bin_count", PyValue::int(bins.len() as i64));
    d.set("libraries", py::list_str(&libraries));
    println!("{}", PyValue::Dict(d).dumps_ascii().unwrap_or_default());
}
