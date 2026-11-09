//! Runs the own Python check of the importer adapter against the real `scan_node` executable of this build (own fake commands, no Original,
//! no `--self-test`). The adapter and the launcher are the ones next to the Rust package in this tree.

use std::process::Command;

/// The own check of the importer adapter (run with `python3 -B -c`; no file of its own, no Original, no `--self-test`).
const ADAPTER_CHECK: &str = r####""""OWN check of the Python importer adapter `scan_node.py` for the P2/P3 names: the adapter marshals, the native `scan_node` executable decides and
acts. Real processes only: the adapter is imported, the real executable (SCAN_NODE_BIN) serves the bridge, the fake commands are OWN scripts written
here. The `--self-test` route (the original's own harness) is NOT started. Usage: scan_node_adapter_p2_check.py <scripts dir> <executable>"""
import contextlib, io, json, os, stat, subprocess, sys, tempfile
sys.dont_write_bytecode = True
SCRIPTS, ELF = sys.argv[1:3]
tmp = tempfile.mkdtemp(prefix="sn-adapter-")
res = []
def ck(name, ok, detail=""):
    res.append((name, bool(ok))); print(("PASS " if ok else "FAIL ") + name, "" if ok else detail)
def fake(directory, name, body):
    os.makedirs(directory, exist_ok=True); path = os.path.join(directory, name)
    open(path, "w").write("#!/bin/sh\n" + body); os.chmod(path, 0o755)
good, bad = os.path.join(tmp, "good"), os.path.join(tmp, "bad")
fake(good, "nvcc", "printf 'Cuda compilation tools, release 13.2, V13.2.51\\n'\n")
fake(good, "nvidia-smi", "case \"$1\" in -L) printf 'GPU 0: NVIDIA GB10\\n';; *name*) printf 'NVIDIA GB10\\n';; *) printf '[N/A], 12.1\\n';; esac\n")
fake(bad, "ssh", "printf 'Host key verification failed.\\n' >&2\nexit 255\n")
os.environ.update(SCAN_NODE_BIN=ELF, PATH=good + ":/usr/bin:/bin")
os.environ.pop("SCAN_NODE_BRIDGE", None)
sys.path.insert(0, SCRIPTS)
import scan_node as sn
here = os.path.join(SCRIPTS, "scan_node.py")
ck("the P2 names are the adapter's own (code file = adapter), the unported ones stay the original's", all(getattr(sn, n).__code__.co_filename == here for n in ("_run", "scan_cuda_version", "detect_interconnect", "collect_peer_hw", "peer_reachable", "scan_egress", "git_branch")) and sn.build_local_scan_result.__code__.co_filename.endswith("scan_node_original.py"))
ck("function docs and signatures are the original's (`_run(cmd, timeout=10)`)", sn._run.__doc__ == sn._SN_LEGACY._run.__doc__ and str(__import__("inspect").signature(sn._run)) == str(__import__("inspect").signature(sn._SN_LEGACY._run)))
ck("scan_cuda_version() through the real executable and the own fake nvcc", sn.scan_cuda_version() == "132", sn.scan_cuda_version())
ck("scan_gpu_memory / scan_gpus_per_node on the own fake nvidia-smi", sn.scan_gpu_memory() == {"gpu_memory_model": "unified", "gpu_vram_mib": None, "compute_capability": "12.1"} and sn.scan_gpus_per_node() == 1)
ck("the original's `_run` with a STRING command looks up its first character (cmd[0]) - the closed rows' measured call", sn._run("nvcc", "--version") == "")
os.environ["PATH"] = bad + ":/usr/bin:/bin"
sink = io.StringIO()
with contextlib.redirect_stderr(sink):
    answer = sn.collect_peer_hw("user@203.0.113.11")
ck("a failed ssh probe returns {} and THIS process prints the two diagnostic lines (effects performed by the adapter)", answer == {} and sink.getvalue() == "[scan] 서브 HW 프로브 실패: host-key 미등록/불일치 — 메인 known_hosts 에 서브 키를 등록하라(ssh-keyscan)\n[scan]   ssh stderr: Host key verification failed.\n", repr(sink.getvalue()))
ck("PEER_SSH_LAST_ERROR is the ORIGINAL's module dictionary and was updated in it", sn.PEER_SSH_LAST_ERROR is sn._SN_LEGACY.PEER_SSH_LAST_ERROR and sn.PEER_SSH_LAST_ERROR.get("HW", "").startswith("host-key"), sn.PEER_SSH_LAST_ERROR)
try:
    sn.peer_reachable("bad host\0", 22, 1); ck("a NUL host is an explicit NotImplementedError", False)
except NotImplementedError as exc:
    ck("a NUL host is an explicit NotImplementedError (never guessed, never retried in Python)", "NUL" in str(exc), str(exc))
try:
    sn._run([], 10); ck("an empty command raises IndexError", False)
except IndexError as exc:
    ck("an empty command raises the original's IndexError with CPython's text", str(exc) == "list index out of range", str(exc))
# command line: the native executable answers; the observer records the route
obs = os.path.join(tmp, "obs.jsonl")
env = dict(os.environ, SCAN_NODE_ROUTE_OBSERVER=obs, COLUMNS="80", LC_ALL="C.UTF-8")
run = lambda *a: subprocess.run([sys.executable, "-B", here, *a], env=env, capture_output=True, text=True, cwd=tmp)
helped = run("-h")
ck("scan_node.py -h: exit 0, the native usage on stdout", helped.returncode == 0 and helped.stdout.startswith("usage: scan_node.py [-h]") and helped.stderr == "", helped.stderr[:200])
wrong = run("--topology", "bogus")
ck("scan_node.py --topology bogus: exit 2 and argparse's text on stderr", wrong.returncode == 2 and wrong.stdout == "" and "invalid choice: 'bogus'" in wrong.stderr, wrong.stderr[:200])
gate = run("--emit-manifest")
ck("scan_node.py --emit-manifest: the interview gate, exit 3, before any probe", gate.returncode == 3 and gate.stdout == "")
events = [json.loads(line) for line in open(obs) if line.strip()]
ck("the route observer names NativeCli for the three command lines (and not CompatOriginalCli)", [e["route"] for e in events if e["event"] == "end" and e["route"] in ("NativeCli", "CompatOriginalCli")] == ["NativeCli"] * 3, events[-3:])
print(sum(ok for _, ok in res), "passed", sum(not ok for _, ok in res), "failed")
sys.exit(0 if all(ok for _, ok in res) else 1)
"####;

#[test]
fn the_python_adapter_marshals_p2_and_routes_the_command_line_to_the_native_executable() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scripts = manifest.join("../../terraforming_node/scripts");
    let output = Command::new("python3")
        .arg("-B")
        .arg("-c")
        .arg(ADAPTER_CHECK)
        .arg(&scripts)
        .arg(env!("CARGO_BIN_EXE_scan_node"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("python3");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "adapter check failed:\n{text}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains(" 0 failed"), "{text}");
}
