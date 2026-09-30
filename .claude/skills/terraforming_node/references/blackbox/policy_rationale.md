# 블랙박스 — 정책 절 근거 서사 (HOST_SAFETY_LAYERED_DEFENSE)

> terraforming_node 스킬 reference — `registry.yaml` 은 판정 가능한 문장만 들고, 그 문장이 **왜** 그런지는 여기 있다(plan_26093022 P4).
> 이관 전 원문: `git show dcb713a:.claude/policies/registry.yaml` 의 같은 clause_id statement.

## C1 — `HOST_SAFETY_LAYERED_DEFENSE.C1`

원문(개정 전 clause 전체):

> Layered defense = a systemd-resident mem_watchdog (broad @vllm mode) plus a harness-scoped watchdog instance auto-started by run_trial/multinode_serve_smoke.sh, container oom_score_adj=800, earlyoom as a process-level backstop, and a pre-load RAM gate that compares checkpoint total_size/TP plus a floor against MemAvailable. Broad mode selects kill targets by matching vllm against the container name and the image *repository* name only -- the registry and organisation path segments are outside the matching plane, so a measurement tool published under a vllm-named organisation is not killed alongside the server it is measuring, while servers whose container name carries no vllm are still caught through the image.

## C7 — `HOST_SAFETY_LAYERED_DEFENSE.C7`

원문(개정 전 clause 전체):

> Post-mortem capture on unified-memory nodes is efi_pstore (NVRAM), not kdump: node-blackbox level L3 removes crashkernel and ramoops and disarms kdump-tools, because with kdump armed panic() jumps to __crash_kexec before kmsg_dump (crash_kexec_post_notifiers=N) so pstore can never be written, while the reservation costs 2.25 GiB of the very resource whose exhaustion causes the hard-down; on this platform ramoops additionally cannot survive a reset (an identical reserve_mem address across boots still yields 'error in header' after a clean reboot) and makedumpfile does not support kernel 6.17 so vmcore is structurally unavailable. L3 requires one reboot and stays an explicit HITL step, the legacy installer's --with-kdump is refused with that reason rather than silently removed, multi-node additionally cross-streams kmsg to the peer via netconsole, and capture is claimed only after verify_node_blackbox.sh --crash-test/--post-crash writes capture_verified.

## C9 — `HOST_SAFETY_LAYERED_DEFENSE.C9`

원문(개정 전 clause 전체):

> Removal of the legacy host-safety installation is a separate, non-installing script (node_blackbox/purge_host_safety.sh) so no node is ever left in a partially applied purge-plus-reinstall state: it defaults to dry-run and destroys only when a human re-invokes it with --apply as root; it refuses to run before the Phase 0 journal harvest unless the operator explicitly waives the gate with --no-require-seed or names a stored seed with --seed-dir, because the mem_watchdog journal is the envelope's only initial data and is unrecoverable once the unit is removed and its journal vacuumed; every destructive step propagates its rc into a single FAIL verdict so a failed removal can never print a clean purge verdict that a reinstall would then build on; stray narrow-scope watchdog processes are terminated by PID resolved through argv field anchors only, never by a blanket pgrep -f/pkill -f match; and the GRUB config is copied to a verified backup before regeneration, with residue judged only from a readable grub.cfg -- an unreadable file is reported as undecidable, never as clean.

## 날짜 사례 (LIBRARY_GROUNDING_FAIL_CLOSED.C4 에서 이관)

> 2026-09-08 실측: 서가가 닷새·147건 늦어 있었고 관련 자료는 디스크에 실재했다 — 정지한 서가의 "없음"이 거짓이 된 사례.
