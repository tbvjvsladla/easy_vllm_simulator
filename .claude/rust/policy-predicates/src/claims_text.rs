//! Ported clause predicates whose original inspects tracked files (text, regex, bash function bodies) and executes
//! bash/git/rsync/node in hermetic temp dirs. Each function keeps the original's name, order of checks and messages.
use crate::support::*;
use std::path::Path;

const SYNC_TO_SUB: &str = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";

fn sync_to_sub_src() -> R<String> {
    read(SYNC_TO_SUB)
}

fn non_comment_lines(src: &str) -> Vec<&str> {
    splitlines(src).into_iter().filter(|l| !strip(l).starts_with('#')).collect()
}

/// `_stem_allowlist(src)`.
pub fn stem_allowlist(src: &str) -> R<Vec<String>> {
    let decl = extract_bash_array(src, "BAND2_RUNTIME_PATCH_STEMS")?;
    let inner = &decl[index(&decl, "(")? + 1..rindex(&decl, ")")?];
    let stems: Vec<String> = inner.split_whitespace().map(String::from).collect();
    req(!stems.is_empty(), "runtime patch stem allowlist must be a non-empty explicit list")?;
    req(stems.iter().all(|s| fullmatch("[a-z0-9][a-z0-9.-]*", s)),
        format!("every runtime patch stem must be a plain identifier (no glob/path chars): {}", repr_list(&stems)))?;
    Ok(stems)
}

// =============================================================================
// HOST_SAFETY_LAYERED_DEFENSE
// =============================================================================

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C2() -> R {
    let src = read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")?;
    req(src.contains("APPLY=0; EARLYOOM_DEB=\"\""), "APPLY must default to 0 (dry-run) unless --apply is passed")?;
    req(src.contains("--apply) APPLY=1 ;;"), "--apply is the sole toggle to 1")?;
    req(src.contains("if [ \"$APPLY\" = \"1\" ] && [ \"$(id -u)\" -ne 0 ]; then"), "--apply must require root (HITL sudo), never a passwordless/automatic escalation")?;
    req(src.contains("say \"DRY-RUN 종료"), "default path must end in an informative DRY-RUN message, not an install")?;
    let final_if_idx = rindex(&src, "if [ \"$APPLY\" = \"1\" ]; then")?;
    let summary_idx = index_from(&src, "say \"설치 요약", final_if_idx)?;
    let else_idx = index_from(&src, "else", summary_idx)?;
    let dryrun_idx = index_from(&src, "say \"DRY-RUN 종료", else_idx)?;
    req(final_if_idx < summary_idx && summary_idx < else_idx && else_idx < dryrun_idx, "the DRY-RUN message must live in the else-branch of the APPLY guard, after the apply summary")?;
    req(search("\\brole\\b", &src).is_none(), "predicate requirement failed at original line 250")?;
    let skill_src = read(".claude/skills/terraforming_node/SKILL.md")?;
    req(skill_src.contains("## 2.6 호스트 안전체계 — 세션 최종 선택조항"), "host-safety must be documented as the session-FINAL opt-in step")?;
    req(skill_src.contains("Flag 발급 이후") && skill_src.contains("독립 Y/N 선택조항"), "must be documented as coming strictly AFTER Flag issuance, as an independent choice")?;
    req(skill_src.contains("single 은 §1S manifest+Flag 기입 직후") && skill_src.contains("이 절로 온다"), "predicate requirement failed at original line 265")
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C5() -> R {
    let ihs = read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")?;
    req(ihs.contains("systemctl enable --now easy-vllm-memwatch.service"), "the daemon must be a persistent, auto-starting systemd unit (the exception itself)")?;
    let mw = read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")?;
    for banned in ["docker build", "docker pull", "docker run", "docker compose"] {
        req(!mw.contains(banned), format!("mem_watchdog.sh must never itself trigger {}", repr(banned)))?;
    }
    req(mw.contains("docker kill"), "the daemon's only destructive action must be a protective kill")
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C6() -> R {
    let ihs = read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")?;
    req(count(&ihs, "NOPASSWD: /usr/local/sbin/vllm-drop-caches") == 1, "exactly one fixed sudoers NOPASSWD helper path")?;
    req(count(&ihs, "/usr/local/sbin/vllm-drop-caches") >= 2, "predicate requirement failed at original line 378")?;
    let mw = read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")?;
    for line in non_comment_lines(&mw) {
        req(!line.contains("pkill -f"), "blanket process-name kill is forbidden (self-referential exit144 class)")?;
    }
    let mn = read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")?;
    for line in non_comment_lines(&mn) {
        req(!line.contains("pkill -f"), "predicate requirement failed at original line 391")?;
    }
    req(mn.contains("kill \"$WD_MAIN_PID\""), "watchdog shutdown must be PID-based")?;
    let nbi = read(".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh")?;
    req(count(&nbi, "NOPASSWD: %s/vllm-drop-caches") == 1, "the node_blackbox installer must grant exactly one fixed NOPASSWD helper path")?;
    req(nbi.contains("SUDOERS_F=/etc/sudoers.d/easy-vllm-host-safety"), "the sudoers fragment path must be a single named constant, not re-typed per use")?;
    req(nbi.contains("if run install -m 0440 \"$T\" \"$SUDOERS_F\" \\"), "the fragment must be PLACED through run() so a failed placement reaches the FAIL verdict")?;
    req(nbi.contains("&& [ \"$(stat -c %a \"$SUDOERS_F\" 2>/dev/null)\" = \"440\" ] \\"), "the placed fragment must be re-read for mode 0440 — sudo silently ignores any other mode")?;
    req(nbi.contains("&& visudo -cf \"$SUDOERS_F\" >/dev/null 2>&1; then"), "the PLACED fragment (not merely the mktemp candidate) must re-pass visudo")?;
    req(!nbi.contains("install -m 0440 \"$T\" /etc/sudoers.d/easy-vllm-host-safety"), "the pre-repair unconditional placement (rc discarded, ✓ printed regardless) must not return")?;
    req(!nbi.contains("say \"   ✓ sudoers(visudo 검증 통과)\""), "a sudoers success line must never be printed without re-reading the placed fragment")?;
    let sud_place = index(&nbi, "if run install -m 0440 \"$T\" \"$SUDOERS_F\"")?;
    let sud_ok = index_from(&nbi, "say \"   ✓ sudoers(visudo 검증 통과 · $SUDOERS_F 0440 배치 확인)\"", sud_place)?;
    let sud_bad = index_from(&nbi, "say \"   ✗ sudoers 배치 실패: $SUDOERS_F", sud_ok)?;
    let sud_fail = index_from(&nbi, "FAIL=1", sud_bad)?;
    req(sud_place < sud_ok && sud_ok < sud_bad && sud_bad < sud_fail, "the ✓ line must sit inside the verified branch and the ✗ branch must raise FAIL")
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C7() -> R {
    let nb = read(".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh")?;
    let vb = read(".claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh")?;
    let ihs = read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")?;
    req(nb.contains("USE_KDUMP=0"), "L3 must set USE_KDUMP=0 (disarm), never 1")?;
    req(nb.contains("systemctl disable --now kdump-tools"), "L3 must disable kdump-tools")?;
    req(nb.contains("kdump-config unload"), "L3 must unload any already-loaded kexec image")?;
    let m = re("(?s)cat > /etc/default/grub\\.d/zz-easy-vllm-blackbox\\.cfg <<'?EOF'?\\n(.*?)\\nEOF").captures(&nb).ok().flatten();
    req(m.is_some(), "L3 must write the GRUB drop-in via a heredoc we can inspect")?;
    let emitted = m.and_then(|c| c.get(1).map(|g| g.as_str().to_string())).unwrap_or_default();
    for tok in ["crashkernel=", "reserve_mem=", "ramoops."] {
        req(!emitted.contains(tok), format!("L3 GRUB drop-in must emit no {tok} (kdump blocks pstore; ramoops cannot survive reset on this platform)"))?;
    }
    req(!emitted.contains("GRUB_CMDLINE_LINUX_DEFAULT"), "L3 GRUB drop-in must not re-append kernel parameters at all")?;
    req(nb.contains("rm -f /etc/modules-load.d/easy-vllm-ramoops.conf"), "L3 must withdraw the ramoops autoload config")?;
    req(nb.contains("systemctl enable systemd-pstore"), "systemd-pstore archival required (else EFI NVRAM fills and capture fails)")?;
    req(nb.contains("kernel.hung_task_panic=1") && nb.contains("kernel.softlockup_panic=1"), "hang->panic promotion retained so silent hangs reach kmsg_dump")?;
    req(nb.contains("재부팅 1회"), "L3 is documented as requiring exactly one reboot")?;
    req(nb.contains("다음 단계(사람)"), "L3 reboot must be documented as a human-performed step, never unattended")?;
    req(search("(?m)^\\s*(sudo\\s+)?(systemctl\\s+)?reboot\\b", &nb).is_none(), "the install script must never itself execute a reboot — HITL only")?;
    req(ihs.contains("--with-kdump)"), "legacy flag must remain recognised so the refusal is reachable")?;
    req(ihs.contains("거부: --with-kdump 는 폐지됐습니다"), "legacy flag must refuse explicitly")?;
    req(!ihs.contains("KDUMP_CRASHKERNEL="), "legacy installer must no longer reserve crashkernel")?;
    req(vb.contains("capture_verified"), "verifier owns the capture_verified verdict file")?;
    req(vb.contains("--crash-test") && vb.contains("--post-crash"), "crash-test/post-crash modes required")?;
    req(vb.contains("Kernel panic"), "post-crash must confirm record CONTENT, not record count")?;
    req(nb.contains("_KL=\"$(cat /sys/kernel/kexec_crash_loaded 2>/dev/null || echo '')\""), "kdump disarm must be judged from kexec_crash_loaded, not from the disable command rc")?;
    req(nb.contains("if [ -z \"$_KL\" ]; then") && nb.contains("elif [ \"$_KL\" = \"0\" ]; then"), "an unreadable kexec_crash_loaded must be undecidable (FAIL), never reported as disarmed")?;
    req(!nb.contains("say \"   ✓ kdump 무장 해제(USE_KDUMP=0 · 서비스 disable · kexec unload)\""), "the pre-repair unconditional kdump ✓ (result never read) must not return")?;
    req(nb.contains("_got=\"$(sysctl -n \"$_k\" 2>/dev/null)\""), "panic-promotion must be judged by re-reading each key, not by the `sysctl -p` rc alone")?;
    req(nb.contains("done < /etc/sysctl.d/99-easy-vllm-panic-promote.conf"), "expected values must be DERIVED from the file just written (hand-restating them splits the two)")?;
    req(nb.contains("if [ -n \"$_PP_BAD\" ]; then") && nb.contains("elif [ \"$_SP_RC\" -ne 0 ]; then"), "both a value mismatch and a failed `sysctl -p` (reboot persistence) must raise FAIL")?;
    req(!nb.contains("say \"   ✓ hang→panic 승격 sysctl (이제 efi_pstore 를 먹인다)\""), "the pre-repair unconditional sysctl ✓ must not return")?;
    req(nb.contains("_PS_EN=\"$(systemctl is-enabled systemd-pstore 2>/dev/null)\""), "systemd-pstore must be judged by is-enabled, never by the enable rc alone")?;
    req(nb.contains("case \"$_PS_EN\" in") && nb.contains("enabled|enabled-runtime|static|indirect|generated)"), "the is-enabled verdict must enumerate the accepting states explicitly")?;
    req(!nb.contains("say \"   ✓ systemd-pstore 아카이브 활성(NVRAM 누적 방지)\""), "the pre-repair unconditional systemd-pstore ✓ must not return")?;
    req(nb.contains("CKS_NOW=\"$(cat /sys/kernel/kexec_crash_size 2>/dev/null || echo '')\""), "the reservation must be read into a named result, not interpolated into a say-line")?;
    req(nb.contains("elif [ \"$CKS_NOW\" -eq 0 ] 2>/dev/null; then"), "the reservation must be JUDGED == 0, not merely printed for a human to read")?;
    req(!nb.contains("현재 커널의 kdump 예약: $(cat /sys/kernel/kexec_crash_size"), "the pre-repair print-only reservation line (no verdict) must not return")?;
    for undecidable in ["say \"   ✗ /sys/kernel/kexec_crash_loaded 를 읽지 못했다", "say \"  ✗ /sys/kernel/kexec_crash_size 를 읽지 못했다"] {
        let head: String = undecidable.chars().take(60).collect();
        req(nb.contains(undecidable), format!("an unreadable kernel fact must be reported as undecidable: {head}"))?;
    }
    Ok(())
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C9() -> R {
    let rel = ".claude/skills/terraforming_node/scripts/node_blackbox/purge_host_safety.sh";
    let src = read(rel)?;
    req(src.contains("APPLY=0;") || search("(?m)^APPLY=0\\b", &src).is_some(), "--apply must default to off (APPLY=0)")?;
    req(search("run\\(\\)\\{\\s*\\n\\s*if \\[ \"\\$APPLY\" = \"1\" \\]", &src).is_some(), "the destructive wrapper run() must execute only under --apply")?;
    req(src.contains("say \"FAIL: --apply 는 root 필요"), "--apply must refuse to proceed as non-root")?;
    for line in splitlines(&src) {
        let stripped = strip(line);
        if stripped.starts_with('#') {
            continue;
        }
        req(!stripped.contains("install_node_blackbox.sh") && !stripped.contains("install_host_safety.sh"), "the purger must never invoke an installer — purge and install stay separate scripts")?;
    }
    req(src.contains("if [ \"$_rc\" -ne 0 ]; then say \"   ✗ 실패(rc=$_rc): $*\"; FAIL=1; fi"), "run() must propagate a non-zero rc into the single FAIL verdict")?;
    req(src.contains("if [ \"$FAIL\" = \"0\" ]; then say \"PURGE PASS") && src.contains("exit \"$FAIL\""), "the summary verdict and the exit code must both be derived from FAIL")?;
    let invocation = re("(?:^|[;&|(]|\\$\\(|\\bthen\\b|\\belse\\b|\\bdo\\b)\\s*(?:pkill|pgrep)\\b");
    for line in splitlines(&src) {
        if strip(line).starts_with('#') {
            continue;
        }
        req(!invocation.is_match(line).unwrap_or(false), "blanket process-name matching is forbidden on executable lines (2026-08-02 false positive)")?;
    }
    req(src.contains("awk '$2 ~ /(^|\\/)bash$/ && $3 ~ /(^|\\/)mem_watchdog\\.sh$/ {print $1}'"), "wd_pids must resolve PIDs by argv field anchors, not by a whole-line substring match")?;
    req(src.contains("for _i in 0 1 2 3 4 5; do"), "the disappearance probe must sample t=0..5 — six checkpoints for a 5s horizon")?;
    req(src.contains("[ \"$_i\" = \"0\" ] || sleep 1"), "the first probe must precede any sleep and the last must FOLLOW the 5th sleep (the old check→sleep order wasted the final second: real horizon 4s, not 5s)")?;
    req(src.contains("kill -0 \"$p\" 2>/dev/null || { _gone=1; break; }"), "disappearance must be observed with kill -0, not inferred from the kill rc")?;
    req(src.contains("case \"$(ps -o stat= -p \"$p\" 2>/dev/null)\" in Z*) _gone=1; break ;; esac"), "a zombie must count as gone — kill -0 succeeds on zombies (H83)")?;
    let gone_ok = index(&src, "if [ \"$_gone\" = \"1\" ]; then say \"   ✓ PID $p 종료 확인\"")?;
    let gone_bad = index_from(&src, "✗ PID $p 가 kill 후에도 살아 있다", gone_ok)?;
    req(gone_ok < gone_bad && gone_bad < index_from(&src, "FAIL=1", gone_bad)?, "a survivor must raise FAIL; the ✓ line must be reachable only from _gone=1")?;
    req(!src.contains("    run kill \"$p\""), "the pre-repair `run kill` (rc-based verdict, no disappearance check) must not return")?;
    req(src.contains("_st=\"$(dpkg-query -W -f='${Status}' \"$1\" 2>/dev/null)\""), "package removal must be re-read from dpkg-query, not trusted from the apt-get rc")?;
    req(src.contains("*\"install ok installed\"*) say \"   ✗ 패키지 잔존: $1 (status=$_st)\"; FAIL=1 ;;"), "a still-installed package must raise FAIL")?;
    req(src.contains("_v=\"$(sysctl -n \"$1\" 2>/dev/null)\""), "sysctl revert must be re-read from the live key, not trusted from the `sysctl -w` rc")?;
    req(src.contains("if [ -z \"$_v\" ]; then say \"   ✗ $1 을 읽지 못했다 — 되돌림 여부를 **판정할 수 없다**\"; FAIL=1"), "an unreadable sysctl key must be undecidable (FAIL), never reported as reverted")?;
    req(src.contains("GRUB_BAK=/boot/grub/grub.cfg.easy-vllm-purge-backup"), "the GRUB backup path must be fixed")?;
    req(src.contains("cmp -s /boot/grub/grub.cfg \"$GRUB_BAK\""), "the GRUB backup must be byte-verified against the CURRENT grub.cfg, not merely created")?;
    req(src.contains("if [ \"$GRUB_OK\" != \"1\" ]; then"), "GRUB regeneration must be skipped when no verified backup was secured")?;
    let (bak_idx, regen_idx) = (index(&src, "GRUB_BAK=/boot/grub")?, index(&src, "GRUB 재생성 (crashkernel")?);
    req(bak_idx < regen_idx, "the backup must be taken before regeneration, in source order")?;
    req(src.contains("if [ ! -r /boot/grub/grub.cfg ]; then") && src.contains("crashkernel 잔재를 **판정할 수 없다**"), "an unreadable grub.cfg must be reported as undecidable (FAIL), never as clean")?;

    let root = repo_root();
    let script = root.join(rel).to_string_lossy().into_owned();
    let gated = run(&["bash", &script, "--node-id=predicate-probe", "--seed-dir=/nonexistent-predicate-probe"], Some(&root), &[], None, Some(180))?;
    req(gated.code == 1, format!("missing Phase 0 seed must refuse with rc 1 (got {})", gated.code))?;
    req(format!("{}{}", gated.stdout, gated.stderr).contains("저널 수확 전 제거 금지"), "the seed gate must state why it refuses (envelope's only initial data)")?;
    req(!gated.stdout.contains("(dry-run)"), "the seed gate must refuse BEFORE any removal step is even enumerated")?;
    let dry = run(&["bash", &script, "--node-id=predicate-probe", "--no-require-seed"], Some(&root), &[], None, Some(180))?;
    req(dry.code == 0, format!("a waived dry-run must succeed without touching the host (rc {})", dry.code))?;
    req(dry.stdout.contains("모드: DRY-RUN"), "the run must announce itself as DRY-RUN")?;
    req(dry.stdout.contains("DRY-RUN 종료") && !dry.stdout.contains("PURGE PASS"), "a dry-run must end at the DRY-RUN exit and never emit a purge verdict")?;
    let emitted: Vec<&str> = splitlines(&dry.stdout).into_iter().filter(|l| l.starts_with("        ")).collect();
    req(!emitted.is_empty(), "the dry-run must actually enumerate removal steps (else the probe proves nothing)")?;
    for ln in emitted {
        let head: String = strip(ln).chars().take(80).collect();
        req(ln.contains("(dry-run) "), format!("every enumerated removal step must be marked (dry-run), got: {head}"))?;
    }
    Ok(())
}

/// `_assert_bench_tool_resolution(resolver_argv)`.
fn assert_bench_tool_resolution(resolver_argv: &[String]) -> R {
    let work = TempDir::new("bench-tool-policy-")?;
    let fake_bin = work.path().join("bin");
    mkdirs(&fake_bin)?;
    let docker = fake_bin.join("docker");
    write(&docker, "#!/bin/sh\nprintf \"%s\\n\" \"$@\" >> \"$BENCH_TOOL_PROBE_LOG\"\nprintf \"%s\\n\" \"$BENCH_TOOL_PROBE_REPLY\"\nexit \"$BENCH_TOOL_PROBE_RC\"\n")?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    let pin = work.path().join("pin.json");
    write(&pin, "{\"tools\": {\"fixture\": {\"default_version\": \"1\", \"versions\": {\"1\": {\"image_ref\": \"fixture/tool:1\", \"index_digest\": \"sha256:recorded\", \"platform_digests\": {\"linux/arm64\": \"sha256:platform\"}, \"version_source\": \"policy fixture\"}}}}}")?;
    let log = work.path().join("docker-argv");
    let expected_argv = ["image", "inspect", "fixture/tool:1", "--format", "{{json .RepoDigests}}"];
    // (name, engine_rc, reply, rc, status, observed, matches, diagnostic)
    let cases: [(&str, i32, &str, i32, &str, Option<&str>, Option<bool>, &str); 3] = [
        ("platform-match", 0, "[\"fixture/tool@sha256:platform\"]", 0, "resolved", Some("sha256:platform"), Some(true), ""),
        ("absent", 17, "null", 3, "absent", None, None, "ABSENT"),
        ("drift", 0, "[\"fixture/tool@sha256:drift\"]", 0, "resolved", Some("sha256:drift"), Some(false), "NOTE"),
    ];
    let path_env = format!("{}:{}", fake_bin.display(), std::env::var("PATH").unwrap_or_default());
    let pin_s = pin.to_string_lossy().into_owned();
    let log_s = log.to_string_lossy().into_owned();
    for (name, engine_rc, reply, rc, status, observed, matches, diagnostic) in cases {
        if log.exists() {
            let _ = std::fs::remove_file(&log);
        }
        let rc_s = engine_rc.to_string();
        let env = [("PATH", path_env.as_str()), ("BENCH_TOOL_PROBE_LOG", log_s.as_str()), ("BENCH_TOOL_PROBE_REPLY", reply),
                   ("BENCH_TOOL_PROBE_RC", rc_s.as_str()), ("PYTHONDONTWRITEBYTECODE", "1")];
        let mut argv: Vec<&str> = resolver_argv.iter().map(String::as_str).collect();
        argv.extend_from_slice(&["--pin", &pin_s, "--tool", "fixture", "--platform", "linux/amd64", "--json"]);
        let proc = run_env_removed(&argv, &repo_root(), &env, "RESOLVE_BENCH_TOOL_BIN")?;
        req(proc.code == rc, format!("bench tool {name}: expected rc={rc}, got {}: {}", proc.code, repr(&proc.stderr)))?;
        let value = json_loads(&proc.stdout).map_err(|_| Fail::predicate(format!("bench tool {name}: invalid JSON output {}", repr(&proc.stdout))))?;
        let matches_ok = match (jget(&value, "matches_record").ok(), matches) {
            (Some(agent_runtime_support::pyvalue::PyValue::Bool(b)), Some(m)) => *b == m,
            (Some(agent_runtime_support::pyvalue::PyValue::None), None) => true,
            _ => false,
        };
        req(matches_ok, format!("bench tool {name}: matches_record must retain its boolean/null type"))?;
        let expected = format!(
            "{{\"tool\": \"fixture\", \"version\": \"1\", \"version_source\": \"policy fixture\", \"image_ref\": \"fixture/tool:1\", \"recorded_digest\": \"sha256:recorded\", \"observed_digest\": {}, \"digest_source\": \"measured(docker image inspect .RepoDigests)\", \"matches_record\": {}, \"platform_note\": \"linux/amd64\", \"status\": \"{}\"}}",
            observed.map(json_str).unwrap_or_else(|| "null".into()),
            matches.map(|b| if b { "true" } else { "false" }).unwrap_or("null"), status);
        let expected = json_loads(&expected)?;
        req(dict_equal(&value, &expected), format!("bench tool {name}: actual digest provenance differs: {}", value.repr()))?;
        req(if diagnostic.is_empty() { proc.stderr.is_empty() } else { proc.stderr.contains(diagnostic) },
            format!("bench tool {name}: missing or unexpected diagnostic {}", repr(&proc.stderr)))?;
        let logged = if log.exists() { read_path(&log)? } else { String::new() };
        req(log.exists() && splitlines(&logged) == expected_argv, format!("bench tool {name}: only one local image-inspect is permitted; no implicit pull"))?;
    }
    Ok(())
}

/// Python dict equality (order-insensitive, recursive through PyValue equality).
fn dict_equal(a: &agent_runtime_support::pyvalue::PyValue, b: &agent_runtime_support::pyvalue::PyValue) -> bool {
    use agent_runtime_support::pyvalue::PyValue;
    match (a.peel(), b.peel()) {
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && dict_equal(v, v2)))
        }
        (x, y) => x == y,
    }
}

/// `subprocess.run(argv, cwd=..., env=dict(os.environ, **env) minus one key)`.
fn run_env_removed(argv: &[&str], cwd: &Path, env: &[(&str, &str)], removed: &str) -> R<Proc> {
    let saved = std::env::var_os(removed);
    // The child inherits this process environment; drop the one operator override for this call only.
    if saved.is_some() {
        std::env::remove_var(removed);
    }
    let out = run(argv, Some(cwd), env, None, Some(30));
    if let Some(v) = saved {
        std::env::set_var(removed, v);
    }
    out
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C10() -> R {
    let script = ".claude/skills/adversarial-benchmark/scripts/run_bench.sh";
    let src = read(script)?;
    let budget_mark = "--bench-budget-mib <양의 정수> 가 필수다";
    let bk = "--backend openai";
    let q = shlex_quote(script);
    // `_run_bash` runs with the predicate process's cwd: the original is started from the repository root.
    let refused = run_bash_in(&format!("bash {q} _probe {bk} --tool guidellm 2>&1; echo \"rc=$?\""))?;
    req(refused.stdout.contains(budget_mark) && refused.stdout.contains("rc=2"),
        format!("--tool guidellm without a declared memory budget must refuse; got {}", repr(&tail_chars(&refused.stdout, 300))))?;
    let accepted = run_bash_in(&format!("bash {q} _probe {bk} --tool guidellm --bench-budget-mib 4096 2>&1; echo \"rc=$?\""))?;
    req(!accepted.stdout.contains(budget_mark),
        format!("a declared budget must get past the budget check -- otherwise the refusal above is passing for an unrelated reason; got {}", repr(&tail_chars(&accepted.stdout, 300))))?;
    req(src.contains("BACKEND=\"\"") && !src.contains("BACKEND=\"openai-chat\""), "run_bench must not carry a default request format -- the axis goes silent when a default wins")?;
    req(src.contains("--backend 는 필수다"), "run_bench must refuse an unspecified request format with a fail-loud message")?;
    let sweep_src = read(".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh")?;
    req(sweep_src.contains("BACKEND=\"\"") && !sweep_src.contains("BACKEND=\"openai-chat\""), "sweep_bench must not re-introduce the default the downstream script dropped -- a default wins from whichever layer still holds it")?;
    req(src.contains("--memory \"${BENCH_BUDGET_MIB}m\" --memory-swap \"${BENCH_BUDGET_MIB}m\""), "the declared budget must be enforced by the kernel, not merely recorded")?;
    req(src.contains("docker run --rm --name \"$GLNAME\""), "the measurement container must be launched with --rm")?;
    req(src.contains("trap 'docker rm -f \"$GLNAME\" >/dev/null 2>&1 || true' RETURN"), "a teardown trap must remove the measurement container on every exit path -- ownership is defined by cleanup, not by who started it")?;
    let resolver = ".claude/skills/adversarial-benchmark/scripts/resolve_bench_tool.sh";
    req(src.contains("bash \"$SDIR/resolve_bench_tool.sh\""), "the measurement image must be resolved through the installed digest resolver")?;
    assert_bench_tool_resolution(&["bash".to_string(), repo_root().join(resolver).to_string_lossy().into_owned()])?;
    req(!src.contains("docker start") && !src.contains("docker stop"), "the benchmark skill must not start or stop the inference server")
}

/// `_run_bash` with the repository root as cwd (the original runner is started from REPO_ROOT by every caller).
fn run_bash_in(script: &str) -> R<Proc> {
    run(&["bash", "-c", script], Some(&repo_root()), &[], None, Some(30))
}

// =============================================================================
// VARIANT_IMAGE_BUILD_VS_SERVE_PLANE / MODEL_TRIPLET_NO_SUB_PROPAGATION
// =============================================================================

fn service_block(compose: &str, pattern: &str) -> Option<String> {
    search(pattern, compose).map(|m| m.2)
}

pub fn predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C2() -> R {
    let compose = rendered("compose")?;
    let slave_block = service_block(&compose, "(?s)vllm-slave-serve:.*?(?=\\n  vllm|\\z)");
    req(slave_block.is_some(), "vllm-slave-serve service block not found")?;
    let slave_block = slave_block.unwrap();
    req(slave_block.contains("envs/.env.interconnect") && slave_block.contains("envs/.env.cluster"), "predicate requirement failed at original line 570")?;
    req(!slave_block.contains("envs/.env.${CONFIG_FILE"), "slave must never load a model-keyed env_file path off CONFIG_FILE")?;
    let m2 = service_block(&compose, "(?s)vllm-master-serve:.*?(?=\\n  vllm-slave|\\z)")
        .ok_or_else(|| Fail::new("AttributeError", "'NoneType' object has no attribute 'group'"))?;
    req(m2.contains("envs/.env.${CONFIG_FILE"), "only the master gets the model-keyed env file")
}

fn line_with<'a>(src: &'a str, needle: &str) -> R<&'a str> {
    splitlines(src).into_iter().find(|l| l.contains(needle)).ok_or_else(|| Fail::new("StopIteration", ""))
}

pub fn predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C3() -> R {
    let mn = read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")?;
    let master_up = line_with(&mn, "--profile master up")?;
    let slave_up = line_with(&mn, "--profile slave up")?;
    req(count(master_up, "--env-file") == 2, "master receives BOTH cluster (EFC) and model (EF) env files")?;
    req(master_up.contains("--env-file \"$EF\""), "predicate requirement failed at original line 584")?;
    req(count(slave_up, "--env-file") == 1, "slave receives ONLY the cluster env file (EFC)")?;
    req(slave_up.contains("$EFC") && !slave_up.contains("\"$EF\"") && !slave_up.contains("'$EF'"), "predicate requirement failed at original line 586")
}

pub fn predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C1() -> R {
    let src = sync_to_sub_src()?;
    let band2_configs = extract_bash_array(&src, "BAND2_CONFIGS")?;
    let band2_envs = extract_bash_array(&src, "BAND2_ENVS")?;
    let patch_stems = extract_bash_array(&src, "BAND2_RUNTIME_PATCH_STEMS")?;
    let band2_top = extract_bash_array(&src, "BAND2_TOP")?;
    let patch_dirs = extract_bash_array(&src, "BAND2_PATCH_DIRS")?;
    let excluded_top = extract_bash_array(&src, "BAND2_EXCLUDED_TOP")?;
    let payload_decl = search("(?m)^declare -A BAND2_PATCH_DIR_PAYLOAD=\\([^)]*\\)", &src);
    req(payload_decl.is_some(), "post payload declaration must accompany the real filter")?;
    let payload_array = payload_decl.unwrap().2;
    let exclusion_fn = extract_bash_function(&src, "effective_band2_exclusions")?;
    let module_kind_fn = extract_bash_function(&src, "script_modules_active")?;
    let module_files_fn = extract_bash_function(&src, "script_module_files")?;
    let stems = stem_allowlist(&src)?;
    let f = extract_bash_function(&src, "_band2_filters")?;
    req(!f.contains("'/configs/*_patch.py'"), "runtime patch wildcard authority is forbidden")?;
    let tmp = TempDir::new("tmp")?;
    let srcdir = tmp.path().join("src/output/multi");
    let dstdir = tmp.path().join("dst");
    mkdirs(&srcdir.join("configs"))?;
    mkdirs(&srcdir.join("envs"))?;
    mkdirs(&dstdir)?;
    for rel in ["configs/serve_runner.sh", "configs/mymodel.sh", "configs/mymodel.yaml", "envs/.env.interconnect", "envs/.env.mymodel"] {
        write(&srcdir.join(rel), "x")?;
    }
    write(&srcdir.join(format!("configs/{}_patch.py", stems[0])), "x")?;
    write(&srcdir.join(format!("configs/{}_patch.provenance.json", stems[0])), "x")?;
    let script = format!(
        "\nset -e\n{band2_configs}\n{band2_envs}\n{patch_stems}\n{band2_top}\n{patch_dirs}\n{payload_array}\n{excluded_top}\nSRC=\"{}/\"\nTARGETS=(multi)\n{module_kind_fn}\n{module_files_fn}\n{exclusion_fn}\n{f}\n_band2_filters multi\nrsync -a --itemize-changes \"${{FILT[@]}}\" \"{}/\" \"{}/\"\n",
        tmp.path().join("src").display(), srcdir.display(), dstdir.display());
    let proc = run_bash(&script)?;
    req(proc.code == 0, proc.stderr.clone())?;
    let delivered = rglob_files(&dstdir);
    let has = |s: &str| delivered.iter().any(|d| d == s);
    req(has("configs/serve_runner.sh"), "predicate requirement failed at original line 632")?;
    req(has("envs/.env.interconnect"), "predicate requirement failed at original line 633")?;
    req(!has("configs/mymodel.sh"), "model triplet .sh leaked to sub")?;
    req(!has("configs/mymodel.yaml"), "model triplet .yaml leaked to sub")?;
    req(!has("envs/.env.mymodel"), "model env leaked to sub")?;
    req(has(&format!("configs/{}_patch.py", stems[0])), "owner-allowlisted runtime patch was not delivered")?;
    req(has(&format!("configs/{}_patch.provenance.json", stems[0])), "runtime patch provenance was not delivered")
}

pub fn predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C3() -> R {
    let src = sync_to_sub_src()?;
    let band2_configs = extract_bash_array(&src, "BAND2_CONFIGS")?;
    let band2_envs_line = extract_bash_array(&src, "BAND2_ENVS")?;
    let band2_top = extract_bash_array(&src, "BAND2_TOP")?;
    let before_paren = band2_envs_line.split(')').next().unwrap_or("");
    let names = findall("\\.env\\.\\S+", before_paren);
    let set: std::collections::BTreeSet<&str> = names.iter().map(String::as_str).collect();
    req(set == [".env.cluster", ".env.interconnect"].into_iter().collect(), "predicate requirement failed at original line 676")?;
    req(!band2_envs_line.splitn(2, '=').nth(1).unwrap_or("").contains('*'), "BAND2_ENVS must contain no wildcard entries")?;
    let f = extract_bash_function(&src, "_band2_filters")?;
    req(f.trim_end().ends_with("--exclude='/*')") || f.contains("--exclude='/envs/*'"), "predicate requirement failed at original line 680")?;
    let classify_fn = extract_bash_function(&src, "assert_band_classification")?;
    let tmp = TempDir::new("tmp")?;
    let odir = tmp.path().join("output/multi");
    let (cdir, edir) = (odir.join("configs"), odir.join("envs"));
    mkdirs(&cdir)?;
    mkdirs(&edir)?;
    for f in ["serve_runner.sh", "debug-init.sh", "arm_patch.sh", "goodmodel.sh", "goodmodel.yaml"] {
        write(&cdir.join(f), "x")?;
    }
    for f in [".env.interconnect", ".env.cluster", ".env.goodmodel"] {
        write(&edir.join(f), "x")?;
    }
    let run_classify = || -> R<Proc> {
        let script = format!("SRC=\"{}/\"\n{band2_configs}\n{band2_envs_line}\n{band2_top}\n{classify_fn}\nassert_band_classification multi\necho \"RC=$?\"\n", tmp.path().display());
        run_bash(&script)
    };
    let clean = run_classify()?;
    req(clean.stdout.contains("RC=0"), format!("a fully-classified Band2+Band3 tree must pass: {} {}", clean.stdout, clean.stderr))?;
    write(&edir.join(".env.roguemodel"), "x")?;
    let bad = run_classify()?;
    req(bad.stdout.contains("RC=1"), "an orphan model env (the direct-rsync-workaround signature) must be refused, not admitted")?;
    req(bad.stderr.contains("FAIL(S4 미분류)") && bad.stderr.contains(".env.roguemodel"), "predicate requirement failed at original line 712")
}

// =============================================================================
// LAST_GOOD_ROLLBACK_ANCHOR
// =============================================================================

pub fn predicate_LAST_GOOD_ROLLBACK_ANCHOR_C1() -> R {
    let workflow_md = read(".claude/rules/workflow.md")?;
    let gate_idx = index(&workflow_md, "HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)")?;
    let s4_idx = index(&workflow_md, "S4 commit   → 스모크 통과분만 로컬 last-good 커밋")?;
    req(gate_idx < s4_idx, "the smoke-before-commit HITL gate must be documented as preceding the S4 last-good commit step -- never a partially-applied/uncommitted state getting anchored first")?;
    req(workflow_md.contains("스모크 통과분만 로컬 last-good 커밋"), "only smoke-passed work may become a local last-good commit")?;
    let tmp = TempDir::new("tmp")?;
    let t = tmp.path();
    git(t, &["init", "-q"])?;
    write(&t.join("f.txt"), "smoke-passed-state")?;
    git(t, &["add", "f.txt"])?;
    git(t, &["commit", "-q", "-m", "last-good commit"])?;
    let anchor_sha = git(t, &["rev-parse", "HEAD"])?;
    git(t, &["tag", "-a", "last-good-test", "-m", "anchor"])?;
    write(&t.join("f.txt"), "uncommitted dirty change")?;
    let dirty = git(t, &["status", "--porcelain"])?;
    req(!dirty.is_empty(), "the working tree must be dirty after this edit")?;
    let resolved = git(t, &["rev-parse", "last-good-test^{commit}"])?;
    req(resolved == anchor_sha, "the anchor tag must still resolve to the committed SHA, not the dirty tree")
}

pub fn predicate_LAST_GOOD_ROLLBACK_ANCHOR_C2() -> R {
    let src = read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")?;
    req(!src.contains("git tag"), "predicate requirement failed at original line 970")?;
    req(!src.contains("git reset --hard"), "predicate requirement failed at original line 971")?;
    req(src.contains("CUR_BRANCH=\"$(git rev-parse --abbrev-ref HEAD)\""), "predicate requirement failed at original line 972")?;
    req(src.contains("if [ \"$CUR_BRANCH\" != \"$DST_BRANCH\" ]; then"), "the script must refuse to operate on any branch other than its declared destination")?;
    let checkout_line = splitlines(&src).into_iter().find(|l| strip(l).starts_with("git checkout \"$SRC_BRANCH\"")).ok_or_else(|| Fail::new("StopIteration", ""))?;
    req(strip(checkout_line) == "git checkout \"$SRC_BRANCH\" -- \"${PATHS[@]}\"", "the only cross-branch content transfer must be a pathspec-scoped checkout (working-tree/index only), never a ref-moving operation")?;
    let tmp = TempDir::new("tmp")?;
    let t = tmp.path();
    git(t, &["init", "-q", "-b", "single-node"])?;
    write(&t.join("shared.txt"), "v1")?;
    git(t, &["add", "shared.txt"])?;
    git(t, &["commit", "-q", "-m", "single last-good"])?;
    let single_anchor = git(t, &["rev-parse", "HEAD"])?;
    git(t, &["checkout", "-q", "-b", "multi-node"])?;
    write(&t.join("shared.txt"), "v2-broken-multi-attempt")?;
    git(t, &["add", "shared.txt"])?;
    git(t, &["commit", "-q", "-m", "multi last-good (pre-failure)"])?;
    let multi_anchor = git(t, &["rev-parse", "HEAD"])?;
    git(t, &["checkout", "single-node", "--", "shared.txt"])?;
    req(git(t, &["rev-parse", "--abbrev-ref", "HEAD"])? == "multi-node", "a pathspec-scoped checkout must never switch the current branch")?;
    req(git(t, &["rev-parse", "single-node"])? == single_anchor, "single-node's own last-good anchor must be untouched by a sync run against multi-node")?;
    req(git(t, &["rev-parse", "multi-node"])? == multi_anchor, "multi-node's own committed anchor must also stay untouched (only the working tree/index changed -- the human commits deliberately, per the S4 HITL gate)")?;
    req(!git(t, &["status", "--porcelain"])?.is_empty(), "the pathspec checkout must genuinely stage a change, not silently no-op")
}

// =============================================================================
// RUNTIME_PATCH_NO_CARRY_FORWARD (C2, C3)
// =============================================================================

pub fn predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C2() -> R {
    let src = sync_to_sub_src()?;
    req(src.contains("deliver_build") && src.contains("deliver_overlay"), "predicate requirement failed at original line 1503")?;
    let executable: Vec<&str> = splitlines(&src).into_iter().filter(|l| !strip(l).is_empty() && !lstrip(l).starts_with('#')).collect();
    let executable = executable.join("\n");
    for banned in ["fetch_sub_docs", "git pull", "collect_patch"] {
        req(!executable.contains(banned), "predicate requirement failed at original line 1507")?;
    }
    req(src.contains("BAND2_RUNTIME_PATCH_STEMS"), "explicit owner-local runtime patch allowlist is required")?;
    let fetch_src = read(".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh")?;
    let fetch_exec: Vec<&str> = splitlines(&fetch_src).into_iter().filter(|l| !strip(l).is_empty() && !lstrip(l).starts_with('#')).collect();
    req(!fetch_exec.join("\n").contains("configs"), "the sole upward (sub->main) script must never execute a configs/ transfer -- proving the runtime patch has no path back from a sub, i.e. it can only be authored main-side")?;
    let dryrun_call = search("rsync -an[^\\n]*\"\\$SUB_HOST:\\$SUB_WORK_DIR/docs/\"[^\\n]*\"\\$DEST/\"", &fetch_src);
    let apply_call = search("rsync -az[^\\n]*\"\\$SUB_HOST:\\$SUB_WORK_DIR/docs/\"[^\\n]*\"\\$DEST/\"", &fetch_src);
    req(dryrun_call.is_some() && apply_call.is_some(), "both the dry-run and --apply rsync invocations must pull FROM the sub's docs/ subtree only")?;
    // The production validator (`validate_runtime_patch.py`'s command line) is the Rust `validate_runtime_patch` behind its launcher.
    let validator = repo_root().join(".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.sh").to_string_lossy().into_owned();
    let resolution = repo_root().join(".claude/skills/upstream-version-watch/assets/current-production-resolution.json").to_string_lossy().into_owned();
    let f = extract_bash_function(&src, "_band2_filters")?;
    {
        let tmp = TempDir::new("tmp")?;
        let (configs, envs) = (tmp.path().join("configs"), tmp.path().join("envs"));
        mkdirs(&configs)?;
        mkdirs(&envs)?;
        let patch = configs.join("modelA_patch.py");
        write(&patch, "# freshly derived patch\n")?;
        write(&configs.join("modelA.yaml"), "model: A\nversion: current\n")?;
        write(&envs.join(".env.modelA"), "MODEL=A\n")?;
        let (p, c, e) = (patch.to_string_lossy().into_owned(), configs.to_string_lossy().into_owned(), envs.to_string_lossy().into_owned());
        let common = ["--patch", p.as_str(), "--topology", "multi", "--config-dir", c.as_str(), "--env-dir", e.as_str(), "--resolution", resolution.as_str()];
        let call = |action: &str| -> R<Proc> {
            let mut argv = vec!["bash", validator.as_str(), action];
            argv.extend_from_slice(&common);
            run(&argv, None, &[], None, None)
        };
        let stamped = call("stamp")?;
        req(stamped.code == 0, stamped.stderr.clone())?;
        let verified = call("verify")?;
        req(verified.code == 0, verified.stderr.clone())?;
        write(&patch, "# stale carried-forward patch with no current derivation\n")?;
        let stale = call("verify")?;
        req(stale.code != 0, "stale patch bytes must fail closed despite a generic *_patch.py name")?;
    }
    req(src.contains("validate_runtime_patches \"$1\""), "predicate requirement failed at original line 1551")?;
    req(f.contains("_patch.provenance.json"), "rsync filter must transfer provenance with each allowlisted patch")
}

pub fn predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C3() -> R {
    let src = shared_asset("arm_patch.sh")?;
    let guard = index(&src, "if [ -f \"$_patch\" ]; then")?;
    let purelib = index(&src, "sysconfig.get_paths()[\"purelib\"]")?;
    let writable = index(&src, "[ -w \"$_sp\" ]")?;
    let emit = index(&src, "> \"${_sp}/zzz_${_cfg}_patch.pth\"")?;
    req(guard < purelib && purelib < writable && writable < emit, "predicate requirement failed at original line 1569")?;
    req(src.contains("spec_from_file_location('__cfgpatch'"), "predicate requirement failed at original line 1570")?;
    req(src.contains("s.loader.exec_module(m)"), "the emitted .pth must load and execute the exact runtime patch at Python site-init")?;
    let runner = shared_asset("serve_runner.sh")?;
    let arm_guard_idx = index(&runner, "if [ -f /app/configs/arm_patch.sh ]; then")?;
    let arm_source_idx = index(&runner, "source /app/configs/arm_patch.sh")?;
    let master_branch_idx = index(&runner, "if [ \"${NODE_ROLE}\" = \"master\" ]; then")?;
    let master_ray_start_idx = index(&runner, "ray start --head")?;
    let master_model_source_idx = index(&runner, "source \"${MODEL_SH}\"")?;
    let slave_branch_idx = index(&runner, "elif [ \"${NODE_ROLE}\" = \"slave\" ]; then")?;
    let slave_ray_start_idx = index(&runner, "exec ray start --address=")?;
    req(arm_guard_idx < arm_source_idx && arm_source_idx < master_branch_idx, "arming must be sourced before the master/slave role branch splits at all -- both roles must inherit it unconditionally")?;
    req(arm_source_idx < master_ray_start_idx && master_ray_start_idx < master_model_source_idx, "on master, arming must run before ray head start AND before the model script is sourced")?;
    req(arm_source_idx < slave_branch_idx && slave_branch_idx < slave_ray_start_idx, "on slave, arming must run before the ray worker itself joins the cluster")?;
    let compose = rendered("compose")?;
    let group1 = |pat: &str| -> R<String> {
        re(pat).captures(&compose).ok().flatten().and_then(|c| c.get(1).map(|g| g.as_str().to_string()))
            .ok_or_else(|| Fail::new("AttributeError", "'NoneType' object has no attribute 'group'"))
    };
    let master_cmd = group1("(?s)vllm-master-serve:.*?command:\\s*(\\[.*?\\])")?;
    let slave_cmd = group1("(?s)vllm-slave-serve:.*?command:\\s*(\\[.*?\\])")?;
    req(master_cmd.contains("/app/configs/serve_runner.sh") && slave_cmd.contains("/app/configs/serve_runner.sh"), "docker-compose must actually invoke serve_runner.sh as the container command on both master and slave")
}

// =============================================================================
// SUB_SYNC_DIRTY_AUTOSAVE
// =============================================================================

pub fn predicate_SUB_SYNC_DIRTY_AUTOSAVE_C1() -> R {
    let src = sync_to_sub_src()?;
    if search("(?m)^sub_dirty\\s*\\(\\)", &src).is_some() {
        extract_bash_function(&src, "sub_dirty")?;
    }
    req(src.contains("sub_dirty() { sub_run \"git status --porcelain\"; }"), "sub_dirty must wrap exactly `git status --porcelain` for remote execution")?;
    req(!extract_bash_function(&src, "sub_dirty")?.contains("2>/dev/null"), "sub_dirty must not discard stderr -- an unreadable sub tree must surface its cause, not be folded into \"clean\"")?;
    let tmp = TempDir::new("tmp")?;
    let t = tmp.path().to_string_lossy().into_owned();
    let init = run(&["git", "init", "-q", &t], None, &[], None, None)?;
    if init.code != 0 {
        return Err(Fail::new("CalledProcessError", format!("Command '['git', 'init', '-q', '{t}']' returned non-zero exit status {}.", init.code)));
    }
    write(&tmp.path().join("f"), "untracked")?;
    let out = run(&["git", "-C", &t, "status", "--porcelain"], None, &[], None, None)?;
    req(!strip(&out.stdout).is_empty(), "an untracked/dirty file must be reported by the exact command sub_dirty wraps")
}

pub fn predicate_SUB_SYNC_DIRTY_AUTOSAVE_C2() -> R {
    let src = sync_to_sub_src()?;
    let b1 = &src[index(&src, "# ── B1 per-branch")?..];
    let dirt_idx = index(b1, "DIRT=\"$(sub_dirty)\"")?;
    let stage_idx = index_from(b1, "sub_run \"git add -A\"", dirt_idx)?;
    let commit_idx = index_from(b1, "sub_commit \"[improve] pre-sync autosave", stage_idx)?;
    req(dirt_idx < stage_idx && stage_idx < commit_idx, "the dirty branch must stage then commit the sub work, in that order")?;
    let branch = &b1[dirt_idx..index_from(b1, "보존 완료", commit_idx)?];
    let stash = re("(?i)(^|sub_run\\s+[\"\\u0027])git\\s+stash\\b");
    let commands: Vec<&str> = splitlines(branch).into_iter().filter(|l| !strip(l).is_empty() && !lstrip(l).starts_with('#')).map(strip).collect();
    req(!commands.iter().any(|l| stash.is_match(l).unwrap_or(false)), "the dirty-handling branch must never stash -- preservation is by commit, so the work survives in history")
}

pub fn predicate_SUB_SYNC_DIRTY_AUTOSAVE_C3() -> R {
    let src = sync_to_sub_src()?;
    let lp = &src[index(&src, "# ── B1 per-branch 증분 싱크 ──")?..];
    let dirt_idx = index(lp, "DIRT=\"$(sub_dirty)\"")?;
    let if_idx = index_from(lp, "if [ -n \"$DIRT\" ]; then", dirt_idx)?;
    let stage_idx = index_from(lp, "sub_run \"git add -A\"", if_idx)?;
    let commit_idx = index_from(lp, "sub_commit \"[improve] pre-sync autosave", stage_idx)?;
    let checkout_idx = index_from(lp, "sub_run \"git checkout -q $t\"", commit_idx)?;
    let build_idx = index_from(lp, "deliver_build \"$t\" 0", checkout_idx)?;
    req(dirt_idx < if_idx && if_idx < stage_idx && stage_idx < commit_idx && commit_idx < checkout_idx && checkout_idx < build_idx,
        "preservation (stage+commit) must live inside the dirty branch and strictly precede checkout, which must strictly precede delivery -- otherwise sub work could be clobbered before it is recorded")?;
    let reverify_idx = index_from(lp, "RE_DIRT=\"$(sub_dirty)\"", commit_idx)?;
    req(commit_idx < reverify_idx && reverify_idx < checkout_idx, "the post-preservation cleanliness re-check must sit between the commit and any checkout")?;
    let case_start = index(&src, "case \"$1\" in")?;
    let case_end = index_from(&src, "esac", case_start)?;
    let arg_parser = &src[case_start..case_end];
    req(search("(?i)force|skip|dirty|override|bypass", arg_parser).is_none(), "the closed CLI flag case-block must contain no dirty-handling bypass flag of any spelling")?;
    req(arg_parser.contains("*) echo \"[sync] FAIL: unknown argument: $1\" >&2; exit 2 ;;"), "an unrecognized flag must itself be rejected fail-closed -- proving the enumerated flag set is genuinely closed")?;
    req(findall("(?<![A-Z_])DIRT=\"\\$\\(sub_dirty\\)\"", &src).len() == 1, "the initial dirty probe must appear exactly once, computed live against the sub tree")?;
    req(findall("RE_DIRT=\"\\$\\(sub_dirty\\)\"", &src).len() == 1, "the post-preservation re-verification probe must appear exactly once, recomputed live (never reusing the pre-preservation result, which would mask a failed preservation)")
}

pub fn predicate_SUB_SYNC_DIRTY_AUTOSAVE_C4() -> R {
    let src = sync_to_sub_src()?;
    let lp = &src[index(&src, "# ── B1 per-branch 증분 싱크 ──")?..];
    let if_idx = index(lp, "if [ -n \"$DIRT\" ]; then")?;
    let checkout_idx = index_from(lp, "sub_run \"git checkout -q $t\"", if_idx)?;
    let dirty_branch = &lp[if_idx..checkout_idx];
    let exits: Vec<usize> = dirty_branch.match_indices("exit 8").map(|(i, _)| i).collect();
    req(exits.len() == 4, "the dirty branch must have exactly four refusal paths: stage failure, commit failure, residual dirt after preservation, and an unreadable post-preservation re-probe")?;
    for pos in exits {
        let line_start = dirty_branch[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let stmt = &dirty_branch[line_start..index_from(dirty_branch, "\n", pos)?];
        req(stmt.contains("STOP") && (stmt.contains("보존") || stmt.contains("배달 거부")), "each refusal inside the dirty branch must be a preservation-failure stop, not a consent demand")?;
    }
    req(dirty_branch.contains("보존 완료"), "the dirty branch must have a success path that proceeds to delivery after preserving")
}

// =============================================================================
// SUB_GIT_LOCAL_ONLY
// =============================================================================

pub fn predicate_SUB_GIT_LOCAL_ONLY_C1() -> R {
    let src = sync_to_sub_src()?;
    req(src.contains("sub_run \"git init -q\""), "predicate requirement failed at original line 1767")?;
    req(!src.contains("git remote add"), "sync_to_sub.sh must never itself add a remote to the sub")?;
    let m = search("REMOTES=\"\\$\\(sub_run 'git remote'\\)\"", &src);
    req(m.is_some(), "the origin-zero attestation must read `git remote` for real -- a swallowed read failure must never be able to print the local-only confirmation")?;
    let confirm_idx = index(&src, "origin 0 (로컬 전용 확증)")?;
    req(confirm_idx > m.unwrap().0, "predicate requirement failed at original line 1772")
}

pub fn predicate_SUB_GIT_LOCAL_ONLY_C2() -> R {
    let tmpl = read(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")?;
    let denylist = "단 **`git push`·`git pull`·`git fetch`·`git remote` 금지**(origin 영구 없음).";
    let allowlist = "`git add/commit/checkout/switch/branch/status/diff/log/stash` **허용**";
    req(tmpl.contains(denylist), "the exact push/pull/fetch/remote denial must be verbatim in the sub persona")?;
    req(tmpl.contains(allowlist), "the exact local-only allowlist must be verbatim in the sub persona")?;
    // inspect.getsource(render_sub_env.render_tree) -> the renderer that runs: the Rust `render_tree` of the terraforming
    // runtime (`render_sub_env.sh` starts it; the `.py` is a reference original pending deletion).
    let rse = read(RENDER_SUB_ENV_RS)?;
    let tree_src = rust_top_fn(&rse, "render_tree")
        .ok_or_else(|| Fail::new("AttributeError", "module 'render_sub_env' has no attribute 'render_tree'"))?;
    req(tree_src.contains("render_file(\"CLAUDE.template.md\", \"CLAUDE.md\", \"md\", &mut produced)"), "CLAUDE.template.md must be the exact file render_tree writes as the sub's real CLAUDE.md")?;
    let src = sync_to_sub_src()?;
    let mut dispatched = findall("sub_run\\s+\"([^\"]*)\"", &src);
    dispatched.extend(findall("sub_run\\s+'([^']*)'", &src));
    req(dispatched.len() >= 10, "expected many real git/shell dispatches routed through sub_run")?;
    let mutating = re("git\\s+(push|pull|fetch|clone)\\b");
    let remote_mutating = re("git\\s+remote\\s+(add|set-url|rename|remove|rm)\\b");
    let offenders: Vec<&String> = dispatched.iter().filter(|c| mutating.is_match(c).unwrap_or(false) || remote_mutating.is_match(c).unwrap_or(false)).collect();
    req(offenders.is_empty(), format!("sub_run dispatched a banned git subcommand: {}", repr_list(&offenders)))?;
    let remote_dispatches: Vec<&String> = dispatched.iter().filter(|c| search("git\\s+remote\\b", c).is_some()).collect();
    req(remote_dispatches.len() == 1 && remote_dispatches[0] == "git remote", format!("the only git-remote dispatch must be the bare read-only zero-remote confirmation, got: {}", repr_list(&remote_dispatches)))
}

pub fn predicate_SUB_GIT_LOCAL_ONLY_C4() -> R {
    let tmpl = read(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")?;
    let grant = "**외부 저장소 `git clone` 허용**(코드·레시피·패치 참조 · 작업공간 git 에 원격을 붙이지 않는다 · 빌드·서빙 입력으로 쓰면 커밋 SHA 로 고정해 보고)";
    let exclusion = "**가중치 저장소 clone(`huggingface.co`·`hf.co`)·`git lfs` 금지**";
    req(tmpl.contains(grant), "the external-clone grant must be verbatim in the sub persona")?;
    req(tmpl.contains(exclusion), "the weight-repo/git-lfs exclusion must be verbatim in the sub persona")?;
    let perms = json_load(".claude/skills/terraforming_node/sub_node/settings.local.template.json")?;
    let perms = jget(&perms, "permissions")?;
    let (allow, deny) = (jstrs(jget(perms, "allow")?), jstrs(jget(perms, "deny")?));
    let has = |v: &Vec<String>, s: &str| v.iter().any(|x| x == s);
    req(has(&allow, "Bash(git clone:*)"), "the permission template must allow `git clone`")?;
    req(!has(&deny, "Bash(git clone:*)"), "the blanket `git clone` deny must be gone (deny beats allow)")?;
    let weight = ["Bash(git clone https://huggingface.co:*)", "Bash(git clone https://hf.co:*)", "Bash(git clone *huggingface.co*)", "Bash(git clone *hf.co*)", "Bash(git lfs:*)"];
    for d in weight {
        req(has(&deny, d), format!("the permission template must deny {d}"))?;
    }
    let managed = json_load(".claude/skills/terraforming_node/sub_node/managed-settings.template.json")?;
    let managed = jstrs(jget(jget(&managed, "permissions")?, "deny")?);
    req(!has(&managed, "Bash(git clone:*)"), "the managed-settings template must not blanket-deny `git clone`")?;
    let remote = ["Bash(git push:*)", "Bash(git pull:*)", "Bash(git fetch:*)", "Bash(git remote:*)"];
    for d in weight.iter().chain(remote.iter()) {
        req(has(&managed, d), format!("the managed-settings template must deny {d}"))?;
    }
    for d in remote {
        req(has(&deny, d), format!("workspace-remote deny {d} must survive the clone grant"))?;
    }
    Ok(())
}

pub fn predicate_SUB_GIT_LOCAL_ONLY_C3() -> R {
    let src = sync_to_sub_src()?;
    for literal in ["sub_run \"git init -q\"", "sub_run \"git branch -m multi\"", "sub_run \"git branch single\"", "sub_run \"git checkout -q multi\"",
                    "sub_branch_current() { sub_run \"git rev-parse --abbrev-ref HEAD\"; }"] {
        req(src.contains(literal), format!("B0 bootstrap must issue the exact literal: {literal}"))?;
    }
    {
        let tmp = TempDir::new("tmp")?;
        let t = tmp.path();
        let checked = |args: &[&str]| -> R {
            let d = t.to_string_lossy().into_owned();
            let mut argv = vec!["git", "-C", d.as_str()];
            argv.extend_from_slice(args);
            let p = run(&argv, None, &GIT_T_ENV, None, None)?;
            if p.code != 0 {
                return Err(Fail::new("CalledProcessError", format!("Command '{}' returned non-zero exit status {}.", repr_list(&argv), p.code)));
            }
            Ok(())
        };
        let head = || -> R<String> {
            let d = t.to_string_lossy().into_owned();
            Ok(strip(&run(&["git", "-C", &d, "rev-parse", "--abbrev-ref", "HEAD"], None, &GIT_T_ENV, None, None)?.stdout).to_string())
        };
        checked(&["init", "-q"])?;
        write(&t.join("f"), "x")?;
        checked(&["add", "f"])?;
        checked(&["commit", "-q", "-m", "x"])?;
        checked(&["branch", "-m", "multi"])?;
        checked(&["branch", "single"])?;
        checked(&["checkout", "-q", "multi"])?;
        req(head()? == "multi", "predicate requirement failed at original line 1847")?;
        checked(&["checkout", "-q", "single"])?;
        req(head()? == "single", "the real B0 sequence must switch strictly between multi and single")?;
    }
    req(src.contains("case \"$BRANCH\" in multi|single|both) ;; *) echo \"[sync] FAIL: --branch must be multi|single|both\" >&2; exit 6 ;; esac"), "predicate requirement failed at original line 1853")?;
    req(src.contains("TARGETS=(); case \"$BRANCH\" in multi) TARGETS=(multi);; single) TARGETS=(single);; both) TARGETS=(multi single);; esac"), "predicate requirement failed at original line 1855")?;
    let deliver_build_fn = extract_bash_function(&src, "deliver_build")?;
    let deliver_overlay_fn = extract_bash_function(&src, "deliver_overlay")?;
    req(deliver_build_fn.contains("\"$src\" \"$SUB_HOST:$dst\""), "predicate requirement failed at original line 1862")?;
    req(deliver_overlay_fn.contains("\"$st/\" \"$SUB_HOST:$DEST\""), "predicate requirement failed at original line 1863")?;
    req(search("\"\\$SUB_HOST:[^\"]*\"\\s+\"\\$(src|st)\"", &format!("{deliver_build_fn}{deliver_overlay_fn}")).is_none(), "SUB_HOST must never appear as the rsync SOURCE (reversed direction) in either delivery function")?;
    let fetch_src = read(".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh")?;
    req(search("(?i)\\bgit\\b", &fetch_src).is_none(), "the sole sub-to-main upward channel must never mention git -- recovery is doc-rsync only")?;
    req(count(&fetch_src, "\"$SUB_HOST:$SUB_WORK_DIR/docs/\" \"$DEST/\"") == 2, "both dry-run and apply paths must rsync ONLY the sub's docs/ subtree back to main")?;
    let docs_md = read(".claude/rules/docs.md")?;
    req(docs_md.contains("**상향 회수(서브→메인) = 문서기반 only**"), "predicate requirement failed at original line 1877")?;
    let tmpl = read(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")?;
    req(tmpl.contains("코드/설정 patch 를 보내지 마라(회수는 문서기반 only)"), "predicate requirement failed at original line 1879")
}

// =============================================================================
// A2A_IDENTITY_PROOF_FAIL_CLOSED
// =============================================================================

/// The Rust owners that now carry the A2A/sub-plane behaviour these clauses read (plan_26111615: the `.py` modules
/// are reference originals pending deletion -- never the evidence).
pub const RENDER_SUB_ENV_RS: &str = ".claude/skills/terraforming_node/rust/src/render_sub_env.rs";
pub const SCAN_NODE_PROBE_RS: &str = ".claude/rust/role-contract/src/scan_node_probe.rs";
pub const AGENT_CONTROL_RS: &str = ".claude/skills/terraforming_node/rust/src/agent_control.rs";
pub const RELAY_RS: &str = ".claude/skills/terraforming_node/rust/src/relay.rs";

pub fn predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C1() -> R {
    // The SSH probe of scan_node (`collect_peer_hw` / `_peer_ssh_diag`) is the role-contract Rust probe.
    let scan = read(SCAN_NODE_PROBE_RS)?;
    req(scan.contains("BatchMode=yes") && scan.contains("host-key 미등록/불일치") && scan.contains("키 인증 거부"),
        "terraforming must fail closed on SSH host/client key authentication failures")
}

/// The completion gate's owner source (`completion_gate.py` was ported to `policy-gate`; the clause reads the gate that runs).
const COMPLETION_GATE_OWNER: &str = ".claude/rust/policy-gate/src/completion_gate.rs";

pub fn predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C2() -> R {
    let gate = read(COMPLETION_GATE_OWNER)?;
    req(gate.contains("approval_atoms") && gate.contains("approved_by:") && gate.contains("plan_sha256"),
        "work authorization must bind exact approver atoms and plan bytes")
}

pub fn predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C3() -> R {
    let control = read(AGENT_CONTROL_RS)?;
    let gate = read(COMPLETION_GATE_OWNER)?;
    req(control.contains("role/transport mismatch") && gate.contains("allowed_actions"), "SSH transport must not imply arbitrary action authorization")?;
    req(gate.contains("EXECUTION_APPROVAL_ACTION_NOT_ALLOWED"), "out-of-scope side effects must fail closed")
}

pub fn predicate_A2A_IDENTITY_PROOF_FAIL_CLOSED_C4() -> R {
    let render = read(RENDER_SUB_ENV_RS)?;
    req(render.contains("Agent_Card.template.json"), "Agent Card capability metadata must still be rendered")?;
    let forbidden = ["signing_key", "a2a_signing", "trusted_keys.json", "runtime/a2a"];
    req(!forbidden.iter().any(|t| render.contains(t)), "renderer must not discover, render, or deliver retired signing/trust runtime material")
}

// =============================================================================
// LIBRARY_GROUNDING_FAIL_CLOSED (C5, C6)
// =============================================================================

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C5() -> R {
    let root = repo_root();
    let tn = root.join(".claude/skills/terraforming_node");
    for gone in ["scripts/library_exchange.py", "scripts/library_relay.py", "sub_node/library-exchange.schema.json"] {
        req(!tn.join(gone).exists(), format!("은퇴한 A2A 서고 교환 부품이 남았다: {gone}"))?;
    }
    // The relay that runs is the Rust `relay` module (relay.sh); its self-test plane may name the retired keys on purpose.
    let relay = rust_module_text(RELAY_RS, &["selftest.rs"])?;
    req(!relay.contains("serve_library_requests") && !relay.contains("library_request"), "릴레이가 아직 서고 질의를 나른다 — 서브는 접근자로 직접 묻는다(병존 폴백 ✗ · D2)")?;
    let wd = root.join(".claude/skills/wiki-desk/src");
    let cli = read_path(&wd.join("cli.ts"))?;
    req(cli.contains("resolveAccessor") && cli.contains("forward(") && cli.contains("intake("), "CLI 가 접근자를 읽어 원격(ssh)으로 넘기지 않는다 — 서브와 메인이 같은 명령이 아니다")?;
    let acc = read_path(&wd.join("accessor.ts"))?;
    req(acc.contains("\"local\"") && acc.contains("\"ssh\"") && acc.contains("manifest.yaml"), "접근자 권위가 manifest library 블록이 아니다")?;
    let tmpl = read_path(&root.join("manifest.template.yaml"))?;
    req(search("(?m)^library:", &tmpl).is_some(), "manifest 템플릿에 library 접근자 블록이 없다(메인·서브 동일 스키마)")?;
    let check = root.join(".claude/skills/wiki-desk/checks/concurrency.check.ts").to_string_lossy().into_owned();
    let gw = run(&["node", "--test", "--test-name-pattern", "게이트웨이", &check], None, &[], None, None)?;
    req(gw.code == 0, format!("게이트웨이 허용 목록 시험 실패(셸·목록 밖 명령·남의 미러를 거부해야 한다): {}", tail_chars(&gw.stdout, 600)))
}

pub fn predicate_LIBRARY_GROUNDING_FAIL_CLOSED_C6() -> R {
    let root = repo_root();
    let check = root.join(".claude/skills/wiki-desk/checks/concurrency.check.ts").to_string_lossy().into_owned();
    let r = run(&["node", "--test", &check], None, &[], None, None)?;
    req(r.code == 0, format!("동시성 규정 시험 실패: {}", tail_chars(&r.stdout, 1200)))?;
    let reference = read_path(&root.join(".claude/skills/wiki-desk/references/concurrency.md"))?;
    for tech in ["C06", "C12", "C15", "C16"] {
        req(reference.contains(tech), format!("동시성 규정 문서가 h_report 기법 {tech} 의 채택·기각 근거를 적지 않았다"))?;
    }
    Ok(())
}

// =============================================================================
// BRANCH_CONSTITUTION_LAYERING (C3 is text-only; C1/C2 read the Rust topology_parity library -- see claims_lib.rs)
// =============================================================================

/// `tp.LAYER_EXCLUDE_PATHSPEC`, derived from the owner's pathspec exactly as the original derives it.
pub fn layer_exclude_pathspec() -> String {
    format!(":(exclude){}", role_contract::topology::LAYER_PATHSPEC)
}

pub fn predicate_BRANCH_CONSTITUTION_LAYERING_C3() -> R {
    let sync = read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")?;
    let mirror = &sync[index(&sync, "MIRROR_DIRS=(")?..index(&sync, "ROOT_RELOCATION_TOMBSTONES=(")?];
    let executable: Vec<&str> = splitlines(mirror).into_iter().map(|l| l.splitn(2, '#').next().unwrap_or("")).collect();
    req(!executable.join("\n").contains("docs/report"), "docs/report 가 미러 삭제 목록으로 돌아왔다 -- append-only 공지는 미러가 될 수 없다")?;
    req(sync.contains("REPORT_CONFLICT") && sync.contains("REPORT_ADD"), "report 합집합 판정이 사라졌다")?;
    req(sync.contains("exit 7"), "같은 경로 다른 내용에서 멈추는 종료코드가 없다")?;
    for token in ["git rev-parse --verify --quiet \"$SRC_BRANCH:$f\"", "git rev-parse --verify --quiet \"HEAD:$f\""] {
        req(sync.contains(token), format!("report 비교가 git-대-git 이 아니다 -- 워킹트리 해시는 eol 변환이 끼어든다: {token}"))?;
    }
    req(sync.contains("docs/report/*) ;;"), "삭제 루프의 report 차단 arm 이 사라졌다 -- 미러에서 뺀 지금 이것이 유일한 차단막이다")?;
    req(sync.contains(&layer_exclude_pathspec()), "특화층 제외가 동기화에 없다")?;
    let checkout = splitlines(&sync).into_iter().find(|l| strip(l).starts_with("git checkout \"$SRC_BRANCH\"")).ok_or_else(|| Fail::new("StopIteration", ""))?;
    req(strip(checkout) == "git checkout \"$SRC_BRANCH\" -- \"${PATHS[@]}\"",
        format!("제외는 배열 안에서 해결해야 한다 -- 이 줄에 인자를 덧붙이면 정책 술어와 배포검증이 동시에 깨진다: {}", strip(checkout)))
}
