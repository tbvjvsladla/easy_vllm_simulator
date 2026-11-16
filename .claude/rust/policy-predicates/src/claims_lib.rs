//! Ported clause predicates whose original drives a Python module in-process that now has a Rust library owner: the
//! four-way topology parity predicate (`topology_parity.py` -> `role_contract::topology`). Same checks, same order,
//! same messages; only the call target moved.
use crate::support::*;
use role_contract::topology as tp;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// `_fixture_git(root, *args)`.
pub fn fixture_git(root: &Path, args: &[&str]) -> R<String> {
    let d = root.to_string_lossy().into_owned();
    let mut argv = vec!["git", "-C", d.as_str()];
    argv.extend_from_slice(args);
    let r = run(&argv, None, &GIT_T_ENV, None, None)?;
    req(r.code == 0, format!("fixture git {} failed: {}", args.join(" "), strip(&r.stderr)))?;
    Ok(strip(&r.stdout).to_string())
}

/// `_canonical_fixture_repo(tmp)`.
pub fn canonical_fixture_repo(tmp: &Path) -> R<PathBuf> {
    let root = tmp.to_path_buf();
    mkdirs(&root.join(".claude/rules"))?;
    mkdirs(&root.join(".claude/policies"))?;
    write(&root.join("CLAUDE.md"), "fixture constitution\n")?;
    write(&root.join(".claude/rules/workflow.md"), "fixture workflow\n")?;
    write(&root.join(".claude/policies/registry.yaml"), "{\"schema_version\": 2, \"policies\": []}\n")?;
    write(&root.join(".gitignore"), "*.log\n")?;
    fixture_git(&root, &["init", "-q", "-b", "single-node"])?;
    fixture_git(&root, &["add", "-A"])?;
    fixture_git(&root, &["commit", "-qm", "fixture"])?;
    Ok(root)
}

/// `_layer_fixture(tmp, header_topology, branch=)`.
pub fn layer_fixture(tmp: &Path, header_topology: &str, branch: &str) -> R<PathBuf> {
    let root = canonical_fixture_repo(tmp)?;
    if branch != "single-node" {
        fixture_git(&root, &["checkout", "-q", "-b", branch])?;
    }
    write(&root.join(".claude/rules/strategy.topology.md"), &format!("# s\n\n**topology: {header_topology}**\n"))?;
    fixture_git(&root, &["add", "-A"])?;
    Ok(root)
}

fn crash(c: tp::Crash) -> Fail {
    Fail::new("RuntimeError", format!("{c:?}"))
}

fn layer_file_names(root: &Path) -> R<Vec<String>> {
    let files = tp::layer_files(root.as_os_str().as_bytes()).map_err(crash)?;
    Ok(files.iter().map(|p| Path::new(std::ffi::OsStr::from_bytes(p)).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect())
}

pub fn predicate_BRANCH_CONSTITUTION_LAYERING_C1() -> R {
    let pathspec = tp::LAYER_PATHSPEC;
    let suffix = pathspec.strip_prefix('*').unwrap_or(pathspec);
    req(suffix == ".topology.md", format!("접미사 규약이 바뀌었다: {suffix}"))?;
    req(pathspec == "*.topology.md", format!("경로 규약은 선행 `**/` 없는 형태여야 한다: {pathspec}"))?;
    req(!pathspec.starts_with("**/"), "선행 `**/` 는 git pathspec 에서 루트 파일을 놓친다 -- 그 형태를 쓰면 안 된다")?;
    let exclude = crate::claims_text::layer_exclude_pathspec();
    req(exclude == format!(":(exclude){pathspec}"), format!("제외 pathspec 이 규약에서 파생되지 않는다: {exclude}"))?;
    let sync = read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")?;
    req(sync.contains(&exclude), "동기화가 특화층을 제외하지 않는다 -- 두 자리가 갈라지면 다음 sync 가 특화 파일을 반대 브랜치로 실어 두 브랜치를 다시 같게 만든다")?;
    // The original reads its owner's source; the owner of the enumeration is now the Rust library.
    let src = read(".claude/rust/role-contract/src/topology.rs")?;
    let body = &src[index(&src, "pub fn layer_files(")?..index(&src, "pub fn read_layer_header(")?];
    req(body.contains("ls-files") && !body.contains(".glob("), "특화층 열거는 git 이 해야 한다 -- pathlib 의 `**` 의미가 git 과 달라 두 집합이 갈린다")?;
    req(!body.contains("손등록"), "경로 규약 파생을 손등록 목록으로 되돌리면 안 된다")?;
    let tmp = TempDir::new("tmp")?;
    let root = layer_fixture(tmp.path(), "single", "single-node")?;
    let found = layer_file_names(&root)?;
    req(found == ["strategy.topology.md"], format!("규약에 맞는 파일이 자동으로 잡히지 않았다: {}", repr_list(&found)))?;
    write(&root.join(".claude/rules/plain.md"), "# plain\n")?;
    fixture_git(&root, &["add", "-A"])?;
    req(layer_file_names(&root)? == ["strategy.topology.md"], "규약 밖 파일이 특화층으로 잡혔다")
}

pub fn predicate_BRANCH_CONSTITUTION_LAYERING_C2() -> R {
    req(tp::HEADER_WINDOW >= 1, "헤더 창이 비었다")?;
    // The original reads this rationale from `topology_parity.py` (its HEADER comment). The Rust owner does not carry
    // it; the same rationale is stated in the self-declaration header paragraph of the specialised constitution itself
    // (both branches), which is what this port reads, so the clause no longer depends on the retired .py.
    let src = read(".claude/rules/strategy.topology.md")?;
    req(src.contains("frontmatter") && src.contains("문서화된 동작이 아니"), "본문 선언을 택한 사유가 파일에서 사라지면 다음 사람이 frontmatter 로 되돌린다")?;
    let tmp = TempDir::new("tmp")?;
    let root = tmp.path();
    let good = root.join("a.topology.md");
    write(&good, "# t\n\n**topology: multi** · layer\n")?;
    let (topo, why) = tp::read_layer_header(good.as_os_str().as_bytes());
    req(topo == Some("multi") && why.equals("ok"), "정상 헤더를 읽지 못했다")?;
    let late = format!("{}**topology: multi**\n", "\n".repeat(tp::HEADER_WINDOW + 2));
    let cases: [(&str, &str, &str); 6] = [
        ("none", "# t\n선언 없음\n", "선언 부재"),
        ("loose", "topology: multi\n", "닫히지 않은 형식"),
        ("html", "<!-- topology: multi -->\n", "HTML 주석(컨텍스트에서 제거된다)"),
        ("front", "---\ntopology: multi\n---\n", "frontmatter(처분이 문서화돼 있지 않다)"),
        ("dup", "**topology: multi**\n**topology: single**\n", "선언 2개"),
        ("late", &late, "창 밖 선언"),
    ];
    for (name, text, why) in cases {
        let f = root.join(format!("{name}.topology.md"));
        write(&f, text)?;
        req(tp::read_layer_header(f.as_os_str().as_bytes()).0.is_none(), format!("{why} 가 헤더로 받아들여졌다 -- 형식이 닫혀 있지 않다"))?;
    }
    Ok(())
}
