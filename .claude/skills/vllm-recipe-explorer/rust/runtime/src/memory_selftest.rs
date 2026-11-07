//! `preload_ram_gate.py --self-test`: the self-check of the mmap-offload derivation and its mirror cross-check.
//!
//! The check writes small safetensors-shaped fixtures into private temporary directories and asks the real
//! [`offloadable_bytes_for`] about them: which tensor names each mechanism counts, that a missing index is `None` (never `0`), that a
//! model without such tensors is an honest `0`, and that an unknown mechanism is not corrected. Then it looks for the build patch
//! that carries a copy of the tensor pattern (`output/{multi,single}/build_patches_src/62-*.sh` under the repository) and checks that
//! the copy still says the same thing; with no patch in this working tree it says so (`SKIP`) instead.
//!
//! Each check prints `  [PASS] name` or `  [FAIL] name detail` as it runs; the last line is `[preload_ram_gate] PASS` or
//! `FAIL (n)`. The status is 0 when nothing failed, otherwise 1. An exception the derivation raises ends the run uncaught, after
//! the lines already printed.

use crate::feedback::normpath;
use crate::memory::offloadable_bytes_for;
use crate::memory_cli::CliHost;
use crate::model_config::matches_offload_pattern;
use crate::model_files::{join, os_failure, read_text, PrivateDirectory};
use crate::pyexpr::Res;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_traits::Zero;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const PLE_TENSOR: &str =
    "model.language_model.layers.1.ple.ple_embedding.ngram_embedding.shard_0.weight";
/// The literal the patch's own copy of the pattern must contain.
const PATCH_PATTERN_COPY: &str = r"\.ple\.ple_embedding\.ngram_embedding\.shard_";
const SHARD: &str = "s.safetensors";
const INDEX: &str = "model.safetensors.index.json";
/// Where the patches that carry a copy of the pattern live, under the repository root.
const PATCH_DIRECTORIES: [&str; 2] = [
    "output/multi/build_patches_src",
    "output/single/build_patches_src",
];

/// One tensor of a fixture header: name, dtype, shape and byte range.
struct Tensor(&'static str, &'static str, &'static [i64], [i64; 2]);

fn integers(numbers: &[i64]) -> PyValue {
    PyValue::List(numbers.iter().map(|&n| PyValue::int(n)).collect())
}

fn header_text(tensors: &[Tensor]) -> String {
    let mut header = PyDict::new();
    for Tensor(name, dtype, shape, offsets) in tensors {
        let mut entry = PyDict::new();
        entry.set("dtype", PyValue::str(dtype));
        entry.set("shape", integers(shape));
        entry.set("data_offsets", integers(offsets));
        header.set(name, PyValue::Dict(entry));
    }
    PyValue::Dict(header)
        .dumps_ascii()
        .expect("a header of integers and text is JSON")
}

fn io_failure(error: std::io::Error, path: &Path) -> agent_runtime_support::failure::Failure {
    os_failure(
        &error,
        Some(&PyStr::from_surrogateescape(path.as_os_str().as_bytes())),
    )
}

/// Writes the shard (`struct.pack("<Q", len(raw))`, the header, `payload` zero bytes) and the index that maps every tensor to it.
fn write_model(directory: &Path, tensors: &[Tensor], payload: usize) -> Res<()> {
    let raw = header_text(tensors).into_bytes();
    let mut shard = (raw.len() as u64).to_le_bytes().to_vec();
    shard.extend_from_slice(&raw);
    shard.extend(std::iter::repeat_n(0u8, payload));
    let shard_path = directory.join(SHARD);
    std::fs::write(&shard_path, shard).map_err(|error| io_failure(error, &shard_path))?;
    let mut map = PyDict::new();
    for Tensor(name, ..) in tensors {
        map.set(name, PyValue::str(SHARD));
    }
    let mut index = PyDict::new();
    index.set("weight_map", PyValue::Dict(map));
    let text = PyValue::Dict(index)
        .dumps_ascii()
        .expect("an index of text is JSON");
    let index_path = directory.join(INDEX);
    std::fs::write(&index_path, text).map_err(|error| io_failure(error, &index_path))
}

fn path_value(directory: &Path) -> PyValue {
    PyValue::Str(PyStr::from_surrogateescape(
        directory.as_os_str().as_bytes(),
    ))
}

fn is_integer(value: &PyValue, expected: i64) -> bool {
    matches!(value, PyValue::Int(number) if *number == expected.into())
}

/// The state of one run: what it prints and which checks failed.
struct Checks<'a> {
    host: &'a mut dyn CliHost,
    failures: usize,
}

impl Checks<'_> {
    /// `ck(name, cond, detail)`.
    fn check(&mut self, name: &str, passed: bool, detail: &str) {
        let mut line = String::from(if passed { "  [PASS] " } else { "  [FAIL] " });
        line.push_str(name);
        if !passed {
            line.push(' ');
            line.push_str(detail);
            self.failures += 1;
        }
        self.host.print_stdout(&PyStr::new(&line));
    }
}

/// Whether a file name is a patch of the mirror check (`62-*.sh`).
fn is_patch_name(name: &[u8]) -> bool {
    name.starts_with(b"62-") && name.ends_with(b".sh")
}

/// The patch files under `top` in `os.walk` order: a directory's own files in the order the system lists them, then each
/// subdirectory in that order, links to directories listed but not entered; a directory that cannot be listed contributes nothing.
fn collect_patches(top: &Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(listing) = std::fs::read_dir(top) else {
        return;
    };
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in listing {
        let Ok(entry) = entry else {
            return;
        };
        let is_directory = std::fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir());
        if is_directory {
            directories.push(entry.path());
        } else {
            files.push(entry.path());
        }
    }
    for file in files {
        if file
            .file_name()
            .is_some_and(|name| is_patch_name(name.as_bytes()))
        {
            found.push(file);
        }
    }
    for directory in directories {
        let is_link = std::fs::symlink_metadata(&directory).is_ok_and(|meta| meta.is_symlink());
        if !is_link {
            collect_patches(&directory, found);
        }
    }
}

/// `--self-test`. `skill_root` is the root of the skill this program belongs to; the repository is three directories above it.
pub fn self_test(host: &mut dyn CliHost, skill_root: &PyStr) -> Res<i32> {
    let mut checks = Checks { host, failures: 0 };
    let ple = |name: &str| matches_offload_pattern("ple_mmap", &PyStr::new(name));
    checks.check("G1 패턴이 실제 텐서 이름을 잡는다", ple(PLE_TENSOR), "");
    checks.check(
        "★G2 음성대조: weight_scale 은 잡지 않는다(오프로드 대상이 아니다 — 상주한다)",
        !ple("model.language_model.layers.1.ple.ple_embedding.ngram_embedding.weight_scale"),
        "",
    );
    checks.check(
        "★G3 음성대조: 일반 MoE 전문가 텐서를 잡으면 예산이 통째로 틀어진다",
        !ple("model.language_model.layers.3.mlp.experts.w13_weight"),
        "",
    );

    let first = PrivateDirectory::create("tmp").map_err(|error| os_failure(&error, None))?;
    let path = path_value(first.path());
    let plain = PyValue::str("ple_mmap");
    let mut warnings = PyValue::List(Vec::new());
    let missing = offloadable_bytes_for(&path, &plain, Some(&mut warnings))?;
    let warned = matches!(&warnings, PyValue::List(items) if !items.is_empty());
    checks.check(
        "G4 index 부재는 None(=보정 ✗)이지 0 이 아니다",
        matches!(missing, PyValue::None) && warned,
        "",
    );
    let mut tensors = vec![Tensor(
        "model.layers.0.mlp.w1.weight",
        "F8_E4M3",
        &[2, 2],
        [0, 4],
    )];
    write_model(first.path(), &tensors, 4)?;
    let mut warnings = PyValue::List(Vec::new());
    let counted = offloadable_bytes_for(&path, &plain, Some(&mut warnings))?;
    checks.check(
        "G5 해당 텐서가 없으면 0 이다(부재와 결측을 가른다)",
        is_integer(&counted, 0),
        "",
    );
    tensors.push(Tensor(
        "model.language_model.layers.1.ple.ple_embedding.ngram_embedding.shard_0.weight",
        "F8_E4M3",
        &[4, 2],
        [4, 12],
    ));
    write_model(first.path(), &tensors, 12)?;
    let counted = offloadable_bytes_for(&path, &plain, None)?;
    checks.check(
        "G6 오프로드 바이트를 헤더 offsets 로 정확히 센다",
        is_integer(&counted, 8),
        "",
    );
    let unknown = offloadable_bytes_for(&path, &PyValue::str("nonexistent"), None)?;
    checks.check(
        "G7 알 수 없는 기구는 보정하지 않는다(None)",
        matches!(unknown, PyValue::None),
        "",
    );
    drop(first);

    let second = PrivateDirectory::create("tmp").map_err(|error| os_failure(&error, None))?;
    let path = path_value(second.path());
    write_model(
        second.path(),
        &[
            Tensor("layers.1.engram.embed.weight", "F8_E4M3", &[8, 2], [0, 16]),
            Tensor("layers.1.engram.embed.scale", "U8", &[8, 1], [16, 24]),
            Tensor("layers.1.engram.wkv.weight", "F8_E4M3", &[2, 2], [24, 28]),
            Tensor("layers.1.engram.q_weight", "BF16", &[2], [28, 32]),
            Tensor("layers.3.ffn.experts.0.w1.trellis", "I16", &[4], [32, 40]),
            Tensor("layers.3.embed_tokens.weight", "BF16", &[2], [40, 44]),
        ],
        44,
    )?;
    let engram = offloadable_bytes_for(&path, &PyValue::str("engram_file"), None)?;
    checks.check(
        "G9 engram_file: embed.weight+embed.scale 만 센다(24B) — wkv·q_weight·전문가·embed_tokens 는 상주",
        is_integer(&engram, 24),
        "",
    );
    let mixed = offloadable_bytes_for(&path, &plain, None)?;
    checks.check(
        "★G10 기구가 서로 섞이지 않는다: ple_mmap 은 Engram 텐서를 0 으로 센다",
        matches!(&mixed, PyValue::Int(number) if number.is_zero()),
        "",
    );
    drop(second);
    checks.check(
        "★G11 engram_file 패턴은 PLE 패턴과 다른 말을 한다(교차 오매칭 ✗)",
        !matches_offload_pattern("engram_file", &PyStr::new(PLE_TENSOR)),
        "",
    );

    let repository = normpath(&join(skill_root, &PyStr::new("../../..")));
    let mut patches = Vec::new();
    for directory in PATCH_DIRECTORIES {
        let top = join(&repository, &PyStr::new(directory));
        let bytes = top.to_surrogateescape_bytes().unwrap_or_default();
        collect_patches(Path::new(std::ffi::OsStr::from_bytes(&bytes)), &mut patches);
    }
    if let Some(patch) = patches.first() {
        let body = read_text(&PyStr::from_surrogateescape(patch.as_os_str().as_bytes()))?;
        let copied = body
            .code_points()
            .windows(PATCH_PATTERN_COPY.chars().count())
            .any(|window| {
                window
                    .iter()
                    .copied()
                    .eq(PATCH_PATTERN_COPY.chars().map(u32::from))
            });
        checks.check(
            "★G8 교차검증: 패치의 텐서 패턴이 이 사본과 같은 말을 한다",
            copied,
            "→ 패치가 움직였다. MMAP_OFFLOADABLE_PATTERNS 를 따라가라",
        );
    } else {
        checks.host.print_stdout(&PyStr::new(
            "  [SKIP] G8 교차검증 — 62-*.sh 가 이 워크트리에 없다(브랜치 추적물 · 워킹트리 부재 ≠ 증거 부재)",
        ));
    }
    let verdict = if checks.failures == 0 {
        String::from("PASS")
    } else {
        format!("FAIL ({})", checks.failures)
    };
    checks
        .host
        .print_stdout(&PyStr::new(&format!("[preload_ram_gate] {verdict}")));
    Ok(i32::from(checks.failures != 0))
}
