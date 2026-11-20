# 소비자 인벤토리 (고정 표면 · 영속 데이터 · A2A)

- 정본: plan 26111805 「보존 계약」·「cell 세부」 c20, interview D8. 쓴 cell: c20-contract (base `ead7087d`). 읽는 cell: c21(bin별 계약 시험), c30~c60.
- 3회차(g-c0 검토 1회차 지적, 사용자 결정 h15·h16)에서 누락 소비자·A2A 통로·E2E 계수 경계를 고쳤다. 4회차(검토 2회차 지적, h18)에서 닫힌 스키마의 거부 보존, resolved.json·samples CSV·manifest 줄 모양 독자, 「소스 본문·경로 결합」, E2E 경계 3판을 더했다.
  c21 a·b·c가 다시 열리면 맨 끝 「계약 시험에 반영할 것」을 읽는다.
- **이전 cell의 재조사 의무(h18):** c30~c60 이전 cell은 자기가 바꾸는 bin의 소비자를 같은 cell에서 다시 훑는다. 훑기는 아래 「뽑은 규칙」의 갈래 모두다.
  빠진 소비자를 찾으면 `contract/`에 행을 더하고(추가만 한다. 있는 행을 지우거나 약하게 고치지 않는다) 그 bin의 계약 시험을 보강한다. 이 추가만으로는 c20·c21을 다시 열지 않는다.
  이 문서가 놓친 소비자를 검토가 새로 찾았는데 그 bin을 바꾸는 cell이 c30 뒤이면, 비차단으로 보고 그 cell 메모로 넘긴다.
- 기계 표: `consumers.tsv`(bin × 소비자 파일 한 행). 고정 표면의 수준(L1 바이트·L2 값·L3 자유)은 `types.md` 1절이다.
- 빠진 것을 찾으면 행을 더하고 이 문서의 해당 절을 고친다. c20을 다시 열면 c21도 다시 연다(plan 「진행 규칙」).

## consumers.tsv 열

| 열 | 뜻 |
|---|---|
| `bin` | Cargo bin 이름(16 크레이트, `[[bin]]`과 `src/bin/*.rs`, 201개) |
| `crate` | T `crates.tsv`의 키 |
| `launcher` | 그 bin을 `exec`하는 추적 `.sh`(`;`로 이음, 없으면 `-`) |
| `launcher_env` | 런처의 실행체 덮어쓰기 변수(`<NAME>_BIN`, `TERRAFORMING_BIN_DIR`, `EASY_VLLM_UPSTREAM_RUNTIME_ROOT` 등)와 런처가 `export`하는 변수(`(export)`) |
| `consumer` | bin·런처를 부르거나 이름을 적은 추적 파일, 또는 bin이 쓴 데이터를 읽는 추적 파일 |
| `class` | `script`(셸), `hook`(pre-commit), `rust`(다른 크레이트의 src), `other-test`(다른 크레이트의 checks), `ts`(wiki-desk), `same-crate`·`same-crate-other`(같은 크레이트의 다른 bin·lib·스크립트), `own-test`·`test-data`(자기 시험·데이터), `parity`(parity cases), `skill-doc`(SKILL.md·references·규칙 문서), `ledger`(원장 JSON), `other`(템플릿·도움말 텍스트 등) |
| `lines` | 그 파일 안에서 이름이 나온 줄 |
| `invocation` | 소비자가 넘기는 서브커맨드·플래그(`env:NAME` = 그 호출에 주는 환경 변수) |
| `reads` | 소비자가 읽는 것: `exit:<구분하는 코드>`, `stdout-json:<키>`, `stdout-text:<무엇>`, `stderr:<무엇>`, `merged-output`, `file:<bin이 쓴 경로>`, `source-text:<토큰>`(bin의 소스 파일 본문을 대조), `exec`, `path-only`(경로만 적음), `test`, `comment`, `agent-instruction`(에이전트가 읽는 명령), `golden(…)`, `path-record` |
| `fixed` | `yes`(기계 소비 — 고정 표면), `argv`(문서가 적은 명령행 — 서브커맨드·플래그가 고정), `path`(경로 이름만 고정), `golden`(재기준선 가능), `?` |
| `basis` | `checked` = 사람(검토 에이전트)이 소비자 코드를 읽고 채운 행, `auto` = 아래 규칙으로 뽑은 행(`invocation`·`reads`는 어림) |
| `note` | 기계로 파싱되는 문자열(reason code, 표 머리, 표식 줄) 등. 3회차에 더하거나 고친 행은 `r3:`로 시작한다 |

뽑은 규칙(재현용):
- 1회차 자동 규칙은 셋이다.
  - bin 목록은 각 `Cargo.toml`에서 뽑았다.
  - 런처는 `exec`하는 변수에 `/<bin>` 경로를 대입하는 추적 `.sh`다.
  - 소비자는 추적 텍스트 파일에서 런처 파일 이름·`target/release/<bin>`·`CARGO_BIN_EXE_<bin>`이 나오는 줄이다.
- 3회차에 검토가 찾은 네 갈래를 다시 훑었다(분석 스크립트는 캠페인 c20 cell 폴더 `runs/r3-sweep/`). 찾은 쌍을 손으로 확인해 `checked`·`r3:` 행으로 더했다.
  1. 설치 사본: `install … <ELF>`와 systemd `ExecStart=`(블랙박스 데몬).
  2. 변수 경로: `$VAR/<bin>`, `'$SUB_ELF_REL/<bin>'`, 배열 `SUB_ELF_NAMES=(…)`(서브 배달 ELF).
  3. `.py` 이름의 실행 상수 + `owner_launcher`(hint-publisher `REL_EVIDENCE_PUBLISHER`).
  4. bin이 쓴 데이터를 읽는 TS·데이터 평면 독자: wiki-desk `accessor.ts`·`gateway.ts`, hint-publisher의 블랙박스 세션 데이터·pip freeze 파일 독자.
  - 같은 훑기에서 걸린 나머지는 런처 자신, 빌드 스크립트, 문서 산문의 "install"이었다. 행으로 적지 않았다.
- 4회차에 갈래 둘과 데이터 독자를 더했다(아래 「소스 본문·경로 결합」에 명령과 결과 수).
  5. 소스 본문·경로 결합: `git grep -n '\.claude/[^ ]*\.rs'`(시험 밖 src·스크립트)와 `include_str!`·`read_to_string(…\.rs`·`"<이름>.rs"` 훑기. 다른 코드가 하네스 `.rs`의 토큰·문구·경로에 묶인 자리다.
  6. 형제 실행체: `git grep -n 'current_exe'`에서 실행 파일 옆의 다른 bin을 찾아 부르는 자리.
  - 영속 데이터 독자 셋(resolved.json, 블랙박스 표본 CSV, manifest 줄 모양)과 줄 모양으로 YAML을 읽는 독자(`budget_preflight`)는 검토가 찾은 것을 손으로 확인해 `r4:` 행으로 더했다.
- `docs/report/*`, `PORTING.md`, `notices/`, parity `data/`, 빌드 스크립트(`build_rust*.sh`, `rust_bootstrap.sh`)는 소비자에서 뺐다(빌드 스크립트는 bin 목록을 나열할 뿐이다).
- 다른 크레이트·셸·훅·TS 소비자 행은 `checked`다. 나머지(같은 크레이트, 문서, 시험, parity, 원장)는 `auto`다.

## 고정 표면 — 주요 기계 계약

다른 bin·셸·훅·노드가 기계로 읽는 자리다. 대상은 `consumers.tsv`의 `checked` 행 중 `reads`가 `stdout-json`·`stdout-text`·`stderr`·`file:`·`merged-output`·`source-text`·`exec`인 것이다.
계약 시험은 아래 계약을 bin별로 고정한다. 근거 줄은 `consumers.tsv`에 있다.

| 생산 bin(런처) | 소비자 | 고정할 것 |
|---|---|---|
| `completion_gate`(`.claude/policies/runtime/completion_gate.sh`) | `sync_branches.sh`·`sync_to_sub.sh`(`authorize --action …`), hint-publisher `src/evidence/e1.rs`(`authorize --mode promotion`), policy-selftest `src/suite.rs:468`(`verify`), policy-publisher `src/publisher.rs`(종료 코드·stdout 그대로 전달), `inline_json`(같은 모양의 게이트 JSON) | 서브커맨드 `authorize`·`verify`와 플래그, stdout JSON `allowed`, `propagation_scope.{status,topology,node_id,ssh_host,work_dir,planes}`, `state`, `eligible_for_promotion`, `reason_codes`(예: `HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE`, `*_GATE_OUTPUT_UNREADABLE`), 종료 코드 0/비0 |
| `runtime_selftest`(`runtime_selftest.sh --tripwires-only`) | pre-commit 훅, policy-gate `src/completion_gate.rs`(stderr 마지막 줄) | 종료 코드, stderr 마지막 줄(사유), `[tripwire] FAIL …` 줄 |
| `antipattern_scan`(tripwire ⑥) | policy-selftest `src/tripwires.rs:853`(`tripwire_cli` → `(rc, stderr)`) | `--root --tripwire`, rc, stderr 꼬리. 입력은 terraforming SKILL.md `## 4. 공개 표면` 표(`policy-selftest/src/antipattern.rs:528` `SURFACE_HEADING`) — 표 머리와 열은 L1 |
| `hint`(`hint.sh`) | policy-distribution `src/checks.rs`(`match … --json`), policy-selftest `src/hint_suite.rs`(publish·lint·continue·verify·push), policy-predicates `claims_hint.rs`, upstream `branch_sync_selftest.rs`(`catalog derive`) | 서브커맨드·플래그, JSON `[].tag`·`[].arch_match`·`allowed`·`action`·`exit_code`·`reason_codes`, stderr `[hint] FAIL <CODE>:` 접두, 종료 코드 이름(`EXIT_LOCAL_TAG_OBJECT_MISSING` 등) |
| `evidence_publisher`(`evidence_publisher.sh`) | hint-publisher `src/evidence/e4.rs:2271-2300` `_publisher`(`:2419`·`:2436` `drive_publisher`, `:2819`). `core.rs:32` `REL_EVIDENCE_PUBLISHER`(`.py`)를 `core.rs:667-669`가 `.sh`로 바꾼다 | 서브커맨드 7개(`init`·`append-raw`·`set-narrative`·`publish-benchmark`·`publish-lite-report`·`set-promotion-target`·`finalize`)와 `--repo-root`를 앞에 둔 인자. 플래그 24개는 `--repo-root`(`_publisher`가 붙인다)와 `--anchor`·`--author`·`--bench-report-src`·`--benchmark-mode`·`--benchmark-verdict`·`--certificate-src`·`--dest-name`·`--downgrade-from`·`--downgrade-reason`·`--generated-utc`·`--identity-json`·`--kind`·`--narrative-file`·`--pii-scan-json`·`--recorded-utc`·`--runtime-json`·`--src`·`--tag`·`--task-class`·`--topic`·`--topology`·`--verdict`·`--verdict-json-src`(`e4.rs:2400-2800`)다. 받아들이는 종료 코드 집합, stdout JSON `reason_codes`·`messages`(빈 stdout = `{}`, JSON이 아니면 `HINT_PUBLISHER_OUTPUT_UNREADABLE`), 런처 경로가 `.py` 이름 옆 |
| `topology_parity`(`topology_parity.sh`) | `sync_to_sub.sh`·`closing_sequence.sh`(런처를 `bash` 없이 직접 실행 — 실행 비트 L1), `render_sub_env` | `evaluate --repo --expect --format json`, JSON `verdict`·`topology`·`reason_codes`(`EXPECT_MISMATCH`)·`legs_judged`·`legs_checked`·`reasons` |
| `node_role_contract` | terraforming `src/bootstrap_canary.rs`, `sync_to_sub.sh` | `evaluate --manifest --topology --field <delivery_plane|tool_plane|sub_mode> --format value|json`, 값 `active`, JSON `<field>.value`·`<field>.source` |
| `manifest_contract`(Flag 게이트) | `run_bench.sh`·`lite_bench.sh`, policy-distribution `checks.rs`, policy-predicates `claims_tf.rs`·`claims_owner.rs`, `native_health_probes.sh`(사설 브리지) | `--topology --repo --require-flag`(`--json`), 종료 0 = Flag 발행·3 = manifest 없음, `MANIFEST_CONTRACT_VALIDATE_ONLY=1`(인자 없음, 종료 코드 그대로), 브리지 `"protocol": "manifest-contract-bridge/1"`와 typed wire `{"t": "int", "sign": "+", "hex": "1"}`(공백까지) |
| `resolve_topology` | `run_bench.sh`·`lite_bench.sh` 등 벤치 스크립트 6개 | `--repo`, stdout `single`|`multi`, 종료 3 = manifest 없음(다른 비0과 구분) |
| `resolve_bench_tool` | policy-predicates `claims_text.rs` | `--json`, 정확히 10키 사전, `status` resolved(0)/absent(3), stderr `ABSENT`·`NOTE` |
| `classify_cell_api`·`parse_guidellm`·`bench_reporting_api` | recipe `classify.rs`·`escalation_bridge.rs`, hint `e4.rs`, policy-gate `doc_naming.rs`, policy-selftest `suite.rs` | `evaluate-jsonl` stdin/stdout 규약: 요청 한 줄 `id`·`operation`·`arguments_json`·`result_json`, 응답 한 줄 `id`·`encoding`=`json-values-v1`·`kind`(value|exception)·`value_json`·`type`·`message`, 종료 비0 또는 stderr 출력 = 실패, `constants` 호출의 `DOWNGRADE_REASONS`·`BENCH_MODE_LITE`·`KILL_EVENT_KINDS`·`EVENTS_RECOVERED_PARTS` |
| `bench_preset`·`repeat_axis`·`verdict_rule`·`sweep_stop`·`bench_shell` | `run_bench.sh`·`sweep_bench.sh`·`lite_bench.sh`·`judge_bench.sh` | `\x1f`로 나눈 stdout 줄(`bench_preset resolve` = `KEY\x1fVALUE`, `default`·`repeat_axis resolve` = `N\x1fSRC\x1fKIND`, `verdict_rule spec-row` 4칸, `sweep_stop report declared-repeats` 2칸), `bench_shell`의 위치 인자 서브커맨드 약 30개와 입력 환경 변수, 판정 낱말 `pass`·`server_failed`·`passed`·`refused`·`not_applicable`·`fresh`·`PASS`·`explicit`, `sweep_stop` 종료 0/3 |
| `render_report`(산출 Markdown) | hint-publisher `render_bench_section.rs`, policy-gate completion_gate 판정 표, `evidence_publisher publish-benchmark` | 스윕 표 머리 `| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |`, `★판정점`, `## 측정 구성 — bench_mode · 도구 · 반복 …`(허용 키 21개), `mode: lite`, `## lite 지표`, `## 판정 (표시만`의 `verdict`·`루브릭 권한` 행 |
| `blackbox_session`(세션 도구) | `single_serve_up.sh`·`single_serve_down.sh`·`multinode_serve_smoke.sh`(메인, 서브는 ssh로 서브 런처)·`budget_renew_loop.sh`·`run_bench.sh`, recipe `run_trial_session.rs`, upstream `native_multinode/up.rs:425-450`(서브는 고정 러너), hint-publisher `src/evidence/e2.rs`(이벤트·세션 데이터) | 서브커맨드 `declare-budget`·`renew-budget`·`clear-budget`·`budget-status`·`budget-defaults --field ttl_s`·`budget-block`·`budget-skip`·`start`·`stop`, `ttl_s` 숫자만, `budget-status` JSON `label`·`floor_mib`, renew 종료 3 = 선언 교체, `serve_budget.env`의 `floor_mib`·`expires_epoch`·`label`, 이벤트 `kind` `serve_start`·`serve_stop`과 `session_id`·`started_utc`·`stopped_utc`·`source` = `blackbox_session`(e2.rs:482-535) |
| `budget_preflight` | `multinode_serve_smoke.sh`·`single_serve_up.sh` | `--json` 출력의 `"floor_mib"`·`"arm_ceiling_mib"`·`"overhead_max_mib"`·`"decl_min_ceiling_mib"`, 종료 4 = 감시기가 거절할 선언, `--render gmu|reasons|summary`(stdin JSON) |
| `check_smoke_model` | 같은 두 스크립트 | `<CONFIG> --emit-gate-params`, 합친 출력의 `required_mib=<N>`·`BUDGET_PARAMS … ckpt_mib= tp= kv_mib=` 줄, 종료 0/2(모델 없음)/7(RAM 게이트 거절) |
| `layer_ledger`·`slave_forward`·`blackbox_eta`·`blackbox_thermal` | `sync_branches.sh`(`inline_json divergence-summary` 경유), policy-predicates `claims_owner.rs`, hint `a2.rs`·`a3/collect.rs`, 감시기 스크립트 | `divergence` JSON `status`·`basis`·`base`·`sync_point.status`·`absorbed`(stdout/stderr 분리), `slave_forward prefix|json`의 `forwards[].key/group/value`·`sub_recipe`·stderr `[slave-forward] FAIL(<code>): <msg>`, `--emit-params`가 쓰는 `BB_*`·`BB_TP_*` 파일(감시기가 `source`). `layer_ledger append`·`merge`는 원장의 모르는 키를 보존한다(`types.md` 4절) |
| `blackbox_collect`·`blackbox_events`·`logs_lifecycle`·`regen_envelope`(설치 사본) | systemd 유닛(`install_node_blackbox.sh:332`·`:383`·`:406`·`:407`), `verify_node_blackbox.sh:232-245` | 설치 이름 `/usr/local/sbin/easy-vllm-bb-{collect,events,lifecycle,regen-envelope}`, 유닛 argv(위 「런처와 환경 변수」), 런처 없는 실행 맥락(cwd `/`, argv[0] 설치 경로, owner 경로 없음), 설치본 `--self-test` 종료 0 |
| `sub_runner`(서브에서, 메인이 배달) | `sync_to_sub.sh:886-891`(`sub_runner_run`, ssh `cd '<SUB_WORK_DIR>' && '<SUB_ELF_REL>/sub_runner' <args %q>`), `:1090`·`:1111`·`:2001`, policy-distribution `src/checks.rs:698-699`·`:735-739`(소스 토큰) | `prune-empty-dirs ROOT SKIP POST`·`validate-names ROOT SKIP POST`(SKIP·POST는 `:`로 이은 이름, stdin `/dev/null`, stdout = 줄마다 상대 경로), `retirement-scan STALE`(stdin = 줄마다 활성 표면 상대 경로, stdout 첫 줄 = 분류한 파일 수 N, 다음 줄부터 `<rel>:<줄 번호>:<줄>`), 종료 코드(위 「종료 코드」). 소스 토큰 `let owner = format!(`·`while lo > 0 && is_chars(cs[lo - 1])`·`if token != owner`가 `sub_runner.rs`에 그대로 있어야 한다(distribution 검사 `sub_retirement_consumer_audit_before_tombstone`) |
| `native_remote`(서브에서, 메인이 배달) | upstream `native_multinode.rs:739-760` `Ssh::call`(메인 `native_multinode_serve`) | ssh `ssh -o BatchMode=yes -o ConnectTimeout=8 <host> '<SUB_WORK_DIR>/<SUB_REMOTE_REL> -'`, stdin = 한 줄 base64(빽빽한 JSON 요청: 호출 인자 + `op`·`root`·`run_id`·`marker`·`created_by`), `op` 11종(`init`·`put`·`run`·`start`·`stop`·`text`·`tree`·`gpu`·`ports`·`exists`·`remove_owned_tree`, `:740` 허용 목록과 `remote.rs:108-213`), stdout = 결과 JSON 객체(`run` = `rc`·`out`·`err`, `tree`·`gpu` = `processes`, `ports`, `exists`), 종료 비0 = 거절 |
| `library_access`(`library_access.sh`) | wiki-desk `src/accessor.ts:39-61`(manifest `library` 블록), `src/gateway.ts:76-78`(강제명령 argv) | manifest `library` 블록(`library_access.rs:348-393`의 키 `transport`·`wiki_dir`·`repo_root`·`access_points.<id>.{identity_file,known_hosts,host_of}`, 서브 블록은 `host`·`user`·`identity_file`·`known_hosts`·`node_id`, 경로 값은 JSON 문자열 인용). `accessor.ts`는 자체 YAML 부분집합 파서(`frontmatter.ts` `parseYaml`)와 정규식으로 읽는다. authorized_keys 줄은 `command="<node> <repo>/.claude/skills/wiki-desk/src/gateway.ts --node <id>",restrict ssh-ed25519 <key> wiki-desk-gateway:<id>`(`:256-282`)이고, 표지 `wiki-desk-gateway:<id>`(`:284`)로 바꾸거나 거둔다 |
| `resolve_torch_pin` | `multinode_serve_smoke.sh:440-441` | 위치 인자 `<vllm 버전>`. pretty JSON의 `"torch_prefix": "<v>"` 줄을 줄 시작 sed로 읽는다(L1). 압축 출력이면 빈 값이 되어 ABI 검사가 조용히 생략된다 |
| `render_dockerfile`·`rope_scaling_translate`·`escalation_predicate`·`render_sub_env` | hint `a1.rs`·`a2.rs`, upstream `smoke_model.rs`, `multinode_serve_smoke.sh`, adversarial-benchmark `campaign_write.rs`, upstream `branch_sync_selftest.rs` | `--nccl-envfile`·`--cluster-envfile`·`--env-tier`(JSON `tier`·`detail`), `--json`(`needed`·`hf_overrides`·`native_max_position_embeddings`·…), `predicate_1`·`model_plane_*`, 종료 5/3과 stderr `TOPOLOGY_RUL…` |
| `campaign_init`(`campaign_init.sh`) | terraforming `relay/dispatch.rs:1231-1241`(`--sub-echo <camp> --node --control --utc …`, reslice면 `--rev`), hint-publisher `cli/selftest.rs:640`(`--evidence-relocate --kind --path --to`)와 기존 행 | 서브커맨드 플래그, stdout `[campaign_init] APPLIED ` 접두 줄, 종료 0/2. 캠페인 상태의 유일한 writer다. 열린 자리의 모르는 키는 보존하고, 닫힌 자리는 검증기의 거부를 보존한다(`types.md` 4절 「영속 문서」). **`--rev`는 base 결함**이다: `campaign_init`에 `--rev`로 시작하는 플래그가 다섯이라 모호한 옵션으로 거부된다(종료 2). 별칭을 만들지 않는다. 처분 owner는 c44(파서·reslice 검사)와 c40(`relay` 호출 쪽), 의미는 그때 사용자 HITL이다(`types.md` 1절) |
| `yaml11_oracle` | wiki-desk `checks/frontmatter.check.ts` | stdin YAML → stdout JSON `[[key, value], …]` (c45가 바꾼다) |
| `crate_allow`(`crate_allow.sh`, c20) | 캠페인 검사 `deps-allow` | `check`, 마지막 줄 `DEPS-ALLOW-PASS`, `DEPS-VIOLATION <kind> <crate>@<version> …` 줄(kind: `source`·`unlisted`·`license-denied`·`license-invalid`·`license-mismatch`·`license-undeclared`·`no-source`·`no-notice`·`notice-license`·`path-unknown`), 종료 0/1/2 |

**텍스트로 읽는 JSON(L1 주의):** 셸 소비자 일부가 JSON을 `sed`·`grep`으로 읽는다.
- `budget_renew_loop.sh:145`는 `"label": *"…"`를 한 줄 정규식으로 읽는다.
- `multinode_serve_smoke.sh:883-885`는 `"floor_mib"` 등의 키를 줄 단위로 찾는다.
- `multinode_serve_smoke.sh:440-441`은 `resolve_torch_pin`의 `"torch_prefix": "…"`를 줄 시작 정규식으로 찾는다.
- 이전한 bin이 이 출력을 `serde_json` pretty로 쓰면 지금처럼 키와 값이 한 줄에 남는다. 압축(한 줄 전체) 출력이나 키 뒤 줄바꿈으로 바꾸지 않는다. 계약 시험은 이 줄들을 L1로 고정한다.
- (YAML, 4회차) `budget_preflight --declared-gmu-yaml <서빙 yaml>`은 서빙 YAML을 YAML 로더가 아니라 줄 모양으로 읽는다(`budget_preflight.rs:231` `declared_gmu_from_yaml`: 들여쓰기 뒤 `gpu[-_]memory[-_]utilization` 키 줄의 값).
  그 파일을 쓰는 recipe `gen_recipe_set`은 그 키를 한 줄 `key: value`로 쓴다(L1, `kv-clamp.md:36-39`). YAML 직렬화기로 바꿀 때 그 줄 모양을 지킨다.

## 종료 코드

모든 bin 공통: 0 통과, 1 실패·위반, 2 사용법·입력 오류(`types.md` 3절). 소비자가 따로 구분하는 코드(`consumers.tsv` `reads`의 `exit:`):

| bin | 코드와 뜻(소비자가 구분하는 것) |
|---|---|
| `manifest_contract`, `resolve_topology`, `resolve_bench_tool`, `sweep_stop` | 3 = 사실 없음(manifest·도구 부재) — 다른 비0(정하지 못함)과 다르게 처리 |
| `blackbox_session` | 3 = 예산 선언이 바뀜(renew) |
| `budget_preflight` | 4 = 감시기가 거절할 선언 |
| `check_smoke_model` | 2 = 모델 없음, 7 = RAM 게이트 거절 |
| `render_sub_env` | 5·3(토폴로지 규칙 위반, stderr `TOPOLOGY_RUL…`) |
| `render_dockerfile`, `recipe` | 4(소비자가 구분) |
| `campaign_init` | 0/2 |
| `escalation_predicate` | 0/1 = 판정 결과, 그 밖 = 거절 |
| `lite_bench`·`sweep_bench` 경로(`lite_verdict`) | 6 = 측정 경로 불성립, 7 = 서버 응답 실패(`.claude/rules/workflow.md` §실패 라우팅) |
| `hint` | 이름 붙은 종료 코드(`EXIT_LOCAL_TAG_OBJECT_MISSING` 등, hint-publisher 소스의 상수) |
| `sub_runner` | `prune-empty-dirs` 9 = prune 실패, `retirement-scan` 1 = 스캐너 실패(빈 목록·심볼릭 링크), 2 = 사용법. 소비자 `sync_to_sub.sh`는 비0을 각각 9·98로 바꿔 멈춘다 |
| `native_remote` | 0 = 요청 처리(결과 JSON), 비0 = 거절(stderr 꼬리를 메인이 사유로 싣는다, `native_multinode.rs:754-757`) |
| `crate_allow` | 0 통과, 1 위반, 2 입력 오류(추적 lock 0개·비UTF-8 인자·읽을 수 없는 lock 키 포함) |

## 런처와 환경 변수

- 런처 127개(bin 127개, 런처 하나에 bin 하나): "ELF가 없으면 종료 2 + 안내, 있으면 `exec`" 형식이다. 실행체 덮어쓰기 변수는 88개 런처에 있고(`<NAME>_BIN` 62개, `TERRAFORMING_BIN_DIR` 26개),
  terraforming 런처 26개는 `TERRAFORMING_OWNER_PATH`를 export한다. 그 밖의 export(`BENCH_VERIFY_REPO_ROOT`, `*_REPO_ROOT`, `*_SCRIPTS_DIR`, `HINT_SKILL_DIR` 등)는 `consumers.tsv` `launcher_env`에 있다.
- upstream 런처 22개(`inline_json.sh`·`layer_ledger.sh`·`native_multinode_serve.sh`·`native_wheelhouse.sh`·`resolve_torch_pin.sh` 등, terraforming `node_role_contract.sh` 포함)는
  `EASY_VLLM_UPSTREAM_RUNTIME_ROOT`로 실행체 루트를 바꾼다. 설정했는데 비면 오류다(`inline_json.sh:8-15`). hint-publisher `cli/selftest_e2e.rs:34`와 terraforming
  `render_sub_env/selftest.rs:467`, upstream `branch_sync_selftest.rs:123`·`:879`가 이 변수를 준다. `consumers.tsv` `launcher_env`에 3회차에 더했다.
  - **빈 값 해석이 둘이다(I-N7):** 런처는 설정했는데 비면 오류다(`inline_json.sh:8-15`). 그런데 upstream `native_multinode.rs:1109-1115` `wheel_elf_path`는 빈 값을 설정 안 됨으로 보고 저장소 경로로 간다(`filter(|v| !v.is_empty())`).
    지금 동작이 그렇다(L1, 둘 다 그대로 옮긴다). 하나로 맞추는 것은 `surface-change`이고 c43 몫이다.
- 소비자가 직접 설정하는 덮어쓰기 변수: `COMPLETION_GATE_BIN`, `RUNTIME_SELFTEST_BIN`, `PY_BOUNDARY_GUARD_BIN`, `CAMPAIGN_READ_BIN`, `CAMPAIGN_VALIDATOR_BIN`, `MANIFEST_CONTRACT_BIN`,
  `TOPOLOGY_PARITY_BIN`, `BENCH_SHELL_BIN`, `TURN_BUDGET_BIN`, `HINT_BIN`, `CAMPAIGN_READ_COMPAT_OWNER`, `CAMPAIGN_READ_VALIDATE_ONLY`, `MANIFEST_CONTRACT_VALIDATE_ONLY`. 이름은 L1이다.
- **런처 없이 도는 설치 사본(L1 실행 맥락):** 블랙박스 데몬은 런처를 거치지 않는다.
  - 설치기 `install_node_blackbox.sh:255-267`이 terraforming ELF를 `/usr/local/sbin/easy-vllm-bb-*`로 복사한다. 대상은 collect·eta·events·lifecycle·regen-envelope·thermal이다.
  - systemd 유닛이 그 사본을 직접 띄운다.
    - collect(`:332`): `--node-dir <NODE_DIR> --interval 1`, `Restart=always`.
    - events(`:383`): `--node-dir <NODE_DIR>`, oneshot과 timer.
    - lifecycle(`:406`): `/bin/sh -c '… --node-dir <NODE_DIR> --now <UTC> --apply'`.
    - regen-envelope(`:407`): `--node-dir <NODE_DIR> regen --now <UTC>`. lifecycle 뒤 같은 유닛에서 차례로 돌고, 앞이 실패하면 돌지 않는다.
  - 실행 맥락: cwd는 `/`, argv[0]은 설치 경로이고, `TERRAFORMING_OWNER_PATH`가 없다. 그래서 `owner.rs:13-19`가 `current_exe()`의 7단계 위로 추정하는 저장소 경로가 저장소에 닿지 않는다.
    이 bin들은 owner 경로 없이 동작해야 한다(L1).
  - `verify_node_blackbox.sh:232-245`는 설치본 5개(eta·collect·events·lifecycle·regen-envelope)의 ELF 매직을 보고 `cd / && <설치본> --self-test`를 돌린다(종료 0 = 통과).
  - eta·thermal 사본은 설치만 확인된다. 유닛과 감시기가 사본을 띄우는 줄은 없고, 감시기는 `--emit-params`가 쓴 env 파일을 `source`한다(`:299`·`:315`, 저장소 런처로 돈다).
  - E2E strace는 이 데몬을 보지 못한다(systemd 자손, 「라이브 E2E 계수 경계」 사각).
- 런처 51개는 `exec -a "<런처 경로>"`로 argv[0]을 런처 경로로 둔다. 일부 bin은 그 경로에서 저장소 루트를 찾는다(예: `verify_distribution.sh` 머리 주석). argv[0] 해석은 런처↔bin 내부 계약(L1)이다.
  clap으로 바꿀 때 `prog` 이름(도움말 L3)과 루트 탐색(L1)을 섞지 않는다.
- 메인이 서브로 배달해 런처 없이 띄우는 ELF 4개(`sub_runner`·`regen_build_patches_src`·`inline_json`·`native_remote`)는 아래 「A2A 와이어 형식」에 적는다.
- `.py` 이름의 실행 상수: hint-publisher `core.rs:32` `REL_EVIDENCE_PUBLISHER`(`evidence_publisher.py`)는 `core.rs:667-669` `owner_launcher`가 같은 자리의 `.sh`로 바꿔 실행한다.
  그래서 런처 경로 `.claude/policies/runtime/evidence_publisher.sh`가 `.py` 이름 옆에 있어야 한다(L1). 같은 꼴의 실행 상수는 `REL_RENDER_DOCKERFILE_PRE_PORT`(`core.rs:42`, 기존 행)뿐이다.
  role-contract·adversarial-benchmark의 `OWNER_REL`·`MODULE_REL` 등 `.py` 상수는 정체 비교(검토본 바이트 대조)와 `__file__` 흉내 경로로만 쓰고 실행하지 않는다. 행으로 적지 않았다.
- 문서 교차 확인: 런처 16개는 어떤 추적 문서에도 이름이 없다(코드 소비자만 있다): `bench_reporting_api`, `blackbox_collect`, `blackbox_events`, `budget_preflight`, `check_spec_layout`,
  `classify_cell_api`, `conformance_full_superset_lite_api`, `dataset_prep_api`, `hint_api`, `inline_json`, `inline_python_scan`, `lite_metrics_api`, `render_native_triplet`, `resolve_bench_tool_api`,
  `resolve_guidellm_api`, `rope_scaling_translate`. 추적 MCP 서버 설정은 없다(`.mcp.json` 없음, 추적 파일의 MCP 언급은 CLAUDE.md:121 설치 규칙뿐).

## entrypoints.tsv (결정론 진입점)

- 열: `id`, `cmd`(M 루트에서 `bash -c`로 돈다), `class`(`M 가능`|`W 전용`), `note`(출처·bin·문서 교차 확인·c20 probe 결과). T `pyexec.sh`가 `M 가능` 행을 safe_run(strace) 아래에서
  돌려 python 성공 execve를 센다(D17 ①). 종료 코드는 기록만 한다.
- 행 180개(`M 가능` 153, `W 전용` 27): 런처 127개마다 한 행(자기시험 런처는 자기시험 실행, 나머지는 `--help`, 도움말이 없는 `*_api` 10개는 `native-help`, `claim_predicates`는
  `--list`), 요청서 26111719 §3의 W 게이트·pre-commit 훅·가드·`crate_allow check`·wiki-desk(Node)·툴체인 런처의 실제 모드, 오케스트레이션·호스트·컨테이너 스크립트.
- `M 가능`의 근거: c20이 M HEAD `000a9c79`의 쌍둥이(공유 클론, `target/`·`.claude/runtime` 연결)에서 모든 후보를 T safe_run으로 돌렸다. 격리 위반 0, 쌍둥이 작업 트리 변화 0,
  python 성공 execve 0이었다. 실행 기록은 캠페인 c20 cell 폴더 `runs/probe*/`에 있다.
- 여기서 `M 가능`은 "M에서 격리 위반 없이 돌고 M을 더럽히지 않는다"는 뜻이다(종료 0이 아니어도 된다 — pyexec는 종료 코드를 판정하지 않는다). c19 `wgates.tsv`의 `M 가능`(M에서 종료 0)과
  다르다: `verify_distribution`·`harness_verify`는 여기서 `M 가능`(M에서는 가지·shim 때문에 종료 1·4)이고 `wgates.tsv`에서는 `W 전용`이다.
- `W 전용`: 실행하면 호스트·컨테이너·서브에 작용하는 스크립트 26개(`cmd`는 구문 확인 `bash -n`뿐 — Python 기동은 `inline_python_scan`(정적)과 E2E strace가 센다)와,
  strace 아래에서 너무 긴 `parity compare --stage all`(W pyexec에서 센다).
- 스킬·MCP·런처 교차 확인: 추적 MCP 서버 설정은 없다. 스킬 문서가 부르는 명령은 런처(`.sh`)와 `node .claude/skills/wiki-desk/src/cli.ts`뿐이며 모두 표에 있다.
  런처 16개는 문서에 이름이 없다(위 「런처와 환경 변수」).

## 소비자 없는 bin

`consumers.tsv` `class` = `none` 또는 `parity`·문서만: `upstream_ngc_api`, `upstream_wheel_api`, `bench_preset_bench`, `bench_reporting_bench`, `lite_metrics_bench`,
`parse_guidellm_bench`, `repeat_axis_bench`, `sweep_stop_bench`(parity `cases/tb.json`의 추적 파일 목록에만 나온다). 고정 표면이 없으므로 c21 계약 시험 대상이 아니다.
이전 cell은 이 bin들을 퇴역(`delete.txt`)할지 정한다.

## 영속 데이터

하네스 bin이 쓰고 다른 bin·노드·사람이 나중에 다시 읽는 파일이다. 형식은 고정 표면과 같은 규칙(L1·L2)을 따른다. 이전한 cell이 쓰는 바이트가 바뀌어도 읽는 쪽이 같은 값을 얻어야 한다.
모르는 키를 조용히 지우지 않는다(`types.md` 4절 「영속 문서」). 열린 문서를 다시 쓰는 writer는 모르는 키를 보존하고(키 보존 시험), base가 닫힌 스키마로 거부하는 문서는 거부를 보존한다(같은 종료 코드·reason code, 기록 바이트 불변, 거부 보존 시험).

| 데이터 | 경로 | 쓰는 쪽(근거) | 읽는 쪽(근거) | 형식·수준 |
|---|---|---|---|---|
| 브랜치 레이어 원장 | `.claude/policies/branch_layer_ledger.json` | upstream `layer_ledger`(`src/layer_ledger.rs`, 정식 writer, plan U4. `append`·`merge`가 파일 전체를 다시 쓴다) | `sync_branches.sh`·`closing_sequence.sh`, tripwire(`policy-selftest/src/tripwires.rs`·`antipattern.rs`) | JSON, L2. 손편집 금지(요청서 §1). 항목의 스키마 밖 키 52개(`_reach`·`_execution_note`·`_note`·`_approval_source`) 보존 |
| 아치 변종 원장 | `.claude/policies/arch_variant_ledger.json` | 사람 + upstream escalation | policy-audit `src/arch.rs:61`(JSON 로더), policy-predicates `src/claims_upstream.rs`, hint-publisher `src/artifacts/a1.rs`, recipe `src/recipe_set.rs` | JSON, L2 |
| 스킬 간선·루트 등록부·claim 바인딩·정책 등록부 | `.claude/policies/{base_skill_edges,root_registry,claim_bindings}.json`, `.claude/policies/registry.yaml` | 사람(개정 plan) | policy-audit `src/cli.rs:92-101`(registry.yaml 도 JSON 로더), policy-selftest `src/tripwires.rs`(⑦ root registry·간선)·`src/suite.rs` | JSON, L2. 손대지 않음(요청서 §1) |
| 노드 manifest | `output/<topology>/manifest.yaml`, `output/<topology>/sub_manifest.yaml` | terraforming(인터뷰·스캔), `render_sub_env`(서브용), `library_access`(`library` 블록 텍스트, `library_access.rs:348-393`) | YAML 로더: role-contract `src/load.rs:70`·`src/topology.rs:874`, terraforming `src/staleness_gate.rs:825`, hint-publisher `src/evidence/e1.rs:792`, upstream `src/manifest.rs:152`. **줄 모양 독자**(아래 줄): `node_identity.sh`, role-contract `identity.rs`·`identity_scan.rs`, wiki-desk `accessor.ts` | YAML(1.1 → 1.2, `yaml_inputs.tsv`), L2. 줄 모양은 아래 줄이 L1이다 |
| manifest 줄 모양 | 같은 두 파일 | 같음. writer가 YAML 직렬화기를 바꾸면 이 줄 모양이 바뀔 수 있다 | `node_identity.sh:66`(`grep -E '^self_role:'` 첫 줄, 값의 주석·따옴표를 sed로 벗긴다)·`:92`(`grep -Ec '^[[:space:]]*-[[:space:]]*role:[[:space:]]*main([[:space:]]|#|$)'`, 정확히 1개)·`:107-137`(`ni_resolve_node_id` 해소 순서: 명시 `--node-id` → `self_role:` → 유일한 `- role: main` → 실패). role-contract `identity.rs`(그 셸 파이프라인의 바이트 이식, `:205-288`)·`identity_scan.rs`. wiki-desk `accessor.ts:42`(manifest **전체**를 `frontmatter.ts` `parseYaml` 부분집합 파서로 읽는다. 못 읽으면 `:81` throw), `:43-61`(`library` 블록) | **L1:** 최상위(들여쓰기 0) `self_role:` 줄, `- role: <slug>` 목록 항목 줄의 모양, `library` 블록 줄 모양(키 들여쓰기·JSON 문자열 인용). 그리고 파일 **전체**가 wiki-desk 부분집합 파서를 통과해야 한다. 모양이 바뀌면 서브가 main으로 해소되거나(로그 섞임) 접근자 해소가 throw한다 |
| compose | `output/<topology>/docker-compose.yaml`(추적) | 사람·upstream 렌더 | docker compose, terraforming `harness_permission_probe.rs:214-216` `build_files`(하네스 YAML 로더) | YAML, `<<` 병합 사용 — 변환층이 h15대로 병합 |
| ssh 강제명령 | 메인 `~/.ssh/authorized_keys`의 `wiki-desk-gateway:<node_id>` 줄 | `library_access`(`library_access.rs:256-282`) | sshd → wiki-desk `src/gateway.ts:76-78`(`--node`, `SSH_ORIGINAL_COMMAND`) | 줄 모양·표지 L1 |
| 캠페인 상태 | `campaigns/<id>/campaign.yaml`, `phases/<node>/<phase>.status.json`, `cells/<cell>/{config.yaml,lockset.json,cell.status.json}`, `campaigns/<id>/relay/<ctx>.json`, `relay/pending_hitl.json` | `campaign_init.sh`(유일한 writer, workflow.md §캠페인), `relay.sh`(릴레이 원장) | adversarial-benchmark `src/campaign_write.rs:5019`·`src/campaign_validator.rs:650`, recipe `--config`, `relay.sh --continue`, hint-publisher `src/evidence/e4.rs`(셀 상태) | YAML·JSON, L2. `campaigns/_template/**`는 추적 뼈대. `campaign.yaml`은 **닫힌 스키마**다(`campaign.schema.json` 최상위·`node`·`matrix`·`hint_target`·`hint_approval`·`assignment`·`assignment_revision`이 `additionalProperties: false`, 검증기 `campaign_validator.rs:727`·`:976-977`) — 닫힌 자리는 거부 보존. 열린 자리(`_*` 최상위 안내 키, `budgets`·`topology_sections`, 단계 상태 파일 최상위)와 릴레이 원장은 키 보존. `--assign-set`·`--phase-set`의 통째 교체 객체는 `types.md` 4절 3 (a) |
| 벤치 인증서 | `docs/benchmark/benchmark_<…>.yaml` | `publish_benchmark_record.sh`·`lite_bench.sh --publish-report` | policy-gate `src/completion_gate.rs:499`(flat 파서), policy-publisher `src/gate.rs:102`, hint-publisher `src/evidence/e1.rs:811`, tripwire `policy-selftest/src/tripwires.rs:683` | **flat `key: value` 문자열**, L1(키·값 문자열). YAML 로더가 읽지 않는다 |
| 벤치 리포트·스윕 지도 | `docs/benchmark/bench_report_<…>.md`, `sweep_map_<…>.{md,json}` | adversarial-benchmark `render_report`·`render_sweep_map` | `evidence_publisher publish-benchmark`(측정 구성 표 파싱, docs.md §publisher 계약), hint-publisher lite 자격 | Markdown 표(측정 구성 표 행 L1), JSON L2 |
| 발행 기록·work manifest | evidence_publisher 의 publication record·work manifest | `evidence_publisher.sh init|append-raw|…|finalize`(hint-publisher `e4.rs`가 대신 부르는 자리 포함) | `completion_gate.sh verify`·`authorize`, hint-publisher `src/evidence/e4.rs:2796` `publisher_inputs` | JSON, L2(키·reason_codes L1). 발행 기록은 **닫힌 스키마**다(`publisher.rs:530-577` `validate_record_shape`, 읽을 때마다 `:830`): 모르는 최상위 키 → `PUBLICATION_RECORD_UNKNOWN_FIELDS`, 항목 안 → `PUBLICATION_RECORD_SCHEMA_INVALID`, 종료 2·기록 불변. 거부 보존(`types.md` 4절) |
| hint 태그·카탈로그 | `hint/*` 태그 페이로드, `hints/` 카탈로그·`hints/.drafts/` | `hint.sh publish|continue` | `hint.sh match|verify`(hint-publisher) | 페이로드 바이트(봉인)·태그 이름 L1 |
| 해소 결과 | `output/<topology>/resolved.json`(최상위 `resolved.json`도 recipe가 본다) | S1 해소 bin의 stdout JSON 조각을 에이전트가 병합한다(`business.rs:192` "the fragment merged into resolved.json", upstream SKILL.md §Outputs). `judge_version_delta --write-resolved`가 RMW로 `upstream_delta`를 쓴다(`judge_delta.rs:1205-1229`, 손저작 `upstream_delta_*` → `_superseded_manual_*`, 나머지 키 보존) | hint-publisher `src/evidence/e1.rs:1980-2027` `_bind_resolve`(`vllm_version`·`upstream_delta.{to_sha,to_ref,from_ref,decision}`·`vllm_ref`·`cuda_version`·`torch_cuda_arch`·`build_version`·`image`·`tag`·`pin`·`generated_kst` 등; 사전이 아니면 `{}`로 읽어 낮은 출처로 간다 `:1996`), `src/evidence/e3.rs:1657`(`resolve` 없음 → `HINT_MISSING_RESOLVE`)·`:2299-2343`(`resolve`·`build_identity`·`ngc`·`attestation` 대조)·`:2411-2413`(봉인 payload 후보 `output/<topo>/resolved.json`), recipe `cleanup_docker.rs:853-874`(이미지 모양 문자열을 지울 이미지에서 뺀다, 못 읽으면 건너뛴다), 같은 크레이트 `render_template.rs:145-150`(`upstream_delta` 블록 필수) | JSON. **읽히는 키는 L2**(위 키와 `upstream_delta` 블록), 봉인 payload로 들어가는 파일은 **바이트**. writer 둘 다 키 보존(`types.md` 4절 3 (c)) |
| 블랙박스 표본 CSV | `docs/logs/<node_id>/samples/<UTC일자>.csv`(`<node_dir>/samples/`) | terraforming `blackbox_collect.rs:22-23` `CSV_HEADER`(`ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone`), `:449-496` `SampleWriter`(날마다 새 파일, 머리 한 번, 줄마다 write·flush) | `thermal_watchdog.sh:127-143`(꼬리 8 KiB를 awk로: 1열 `ts`가 정수이고 `NF>=5`인 마지막 줄의 5열 `gpu_pwr`)·`:503`(오늘 파일)로 kill 판정, `install_node_blackbox.sh:659-664`(오늘 파일이 비지 않음)·`verify_node_blackbox.sh:315-320`(4초 사이 바이트 증가), 같은 크레이트 `logs_lifecycle.rs:58-73`·`:377`(rollup·압축), `regen_envelope.rs:63`, `blackbox_session.rs:1112` | **L1:** 머리 줄 전체, 열 순서, `ts` 정수(epoch 초), 파일 이름 규칙 `<UTC일자>.csv`, 줄 단위 flush(증분 검사가 본다) |
| native 서빙 기록 | 실행 simlog의 `pip-freeze-main.txt`·`pip-freeze-sub.txt`, proof `pip_freeze_paths.{main,sub}` | upstream `native_multinode/up.rs:721`(보존 파일) | hint-publisher `src/evidence/e1.rs:2226-2244`(정확히 main·sub 두 경로), `src/artifacts/a3/collect.rs:621-629`(봉인 payload 이름) | 파일 이름·proof 키 L1, 내용은 봉인 바이트(pip freeze 텍스트, h16: 런타임 환경 산출물) |
| 블랙박스 데이터 | `docs/logs/<node_id>/{envelope.json,campaign_brief.json,events…,rollup/…}` | terraforming `regen_envelope`·`logs_lifecycle`·`blackbox_*`(설치 사본 포함), `blackbox_session`(세션·이벤트) | 에이전트 폴링(envelope), 메인의 서브 관측(campaign_brief 회수 미러), hint-publisher `src/evidence/e2.rs`(세션·`serve_start`/`serve_stop` 이벤트) | JSON, L2. rollup의 비유한 수는 `types.md` 4절 |
| simlog·testlog | `docs/simlog/<run>/…`, `docs/testlog/…` | recipe·upstream·벤치 | 사람·에이전트, `completion_gate`(필수 증거 경로) | 원시 vault(편집 금지), 경로·이름은 L1 |
| 서브 회수 미러 | `sync_staging/sub_docs/…` | `fetch_sub_docs.sh` | 사람·에이전트, campaign_brief 관측 | 서브 문서의 사본 |

## A2A 와이어 형식 (메인 ↔ 서브)

정본 절차는 `.claude/skills/terraforming_node/SKILL.md` §2.7과 `references/orchestration/a2a_relay.md`다. 이 표는 이전 때 바이트·값이 지켜져야 하는 자리만 적는다.
경로는 `upstream-version-watch/` 기준이다(`sync_to_sub.sh` = `scripts/sync_to_sub.sh`, `native_multinode.rs` = `rust/runtime/src/native_multinode.rs`).

| 통로 | 실행자 → 상대 | 실려 가는 것 | 고정 수준 |
|---|---|---|---|
| 턴제 릴레이 `relay.sh` | 메인 → 서브 하네스 CLI(ssh) | 과업 텍스트·예산 인자, 응답 스트림 안의 리포트 JSON(`parse_report`, a2a_relay.md:127), 원장 `campaigns/<id>/relay/<ctx>.json`. 리포트는 `sub_node/task-report.schema.json`(`additionalProperties: false`)으로 검사한다 | 리포트 JSON 키·`reason_codes` L1·L2 |
| 서브 진행 요약 | 서브 → 메인(문서 회수) | `docs/logs/<node_id>/campaign_brief.json`(a2a_relay.md:33) | 키 L2 |
| 오버레이·빌드킷 배달 | `sync_to_sub.sh`, `render_sub_env` | 헌법·스킬·블랙박스 오버레이, 서브 manifest, `[sync]` 커밋, `Agent_Card.json`(A2A 1.0.1, unsigned) | 파일 바이트(rsync), 서브 manifest YAML L2, 카드 JSON L2 |
| 서브 러너 ELF 배달 | 메인 `sync_to_sub.sh` → 서브 작업 트리 `.claude/skills/upstream-version-watch/rust/runtime/target/release/` | 메인이 빌드한 ELF 4개 `sub_runner`·`regen_build_patches_src`·`inline_json`·`native_remote`(`sync_to_sub.sh:886-887` `SUB_ELF_NAMES`, `:893-918` `deliver_sub_runner_elfs`). 배달마다(`:2367` bootstrap, `:2445` 토폴로지별 트랜잭션 전) ELF 매직·아키텍처를 대조하고 rsync한다. gitignored라 서브 git·체크섬 표면에 없다 | ELF 이름·자리 L1. **메인 판의 ELF가 서브에서 돈다** |
| 서브 정비 호출 | 메인 → 서브 ELF(ssh `cd '<SUB_WORK_DIR>' && '<SUB_ELF_REL>/<bin>' …`) | `sub_runner` 세 서브커맨드(위 「고정 표면」), `regen_build_patches_src unbundle|verify --root …`(`sync_to_sub.sh:1475`·`:1490`·`:1494`) | argv·stdin·stdout 줄·종료 코드 L1 |
| native multinode 고정 러너 | 메인 `native_multinode_serve` → 서브 `native_remote`(ssh, 위 배달 ELF) | stdin base64 JSON 요청과 `op` 11종, stdout 결과 JSON(위 「고정 표면」, `native_multinode.rs:739-760`, `native_multinode/remote.rs:108-235`). `put` op로 메인이 서브 실행 루트에 `native_wheelhouse` ELF를 둔다(`native_multinode/up.rs:693-695` — 실행 루트에 두는 ELF는 이것 하나다). `run`·`start` op로 서브 런처를 띄운다(아래 줄) | 요청 키·`op` 이름·결과 키 L1, 결과 값 L2 |
| 서브 런처 ssh 통로 | 메인 → 서브 저장소 런처(`bash <SUB_WORK_DIR>/.claude/runtime/…/<launcher>.sh`) | `blackbox_session.sh`(`multinode_serve_smoke.sh:635`·`:686`·`:726`·`:967`·`:1001`, `native_multinode.rs:1038-1040`의 `session_py`), `budget_renew_loop.sh`(`renew_sh`, `:1120-1123` `renew_argv`), `mem_watchdog.sh`(`watchdog_sh`), `regen_envelope.sh`(`multinode_serve_smoke.sh:640` `SUB_REGEN_SH`), `up.rs:427`·`:566` | 서브커맨드·플래그 L1. 실행체는 **서브가 빌드한** ELF다(서브 판) |
| 문서 회수 | `fetch_sub_docs.sh` | 서브 `docs/` → `sync_staging/sub_docs/` | 경로 L1 |
| 서고 접근자 | 서브 wiki-desk(`remote.ts`) → 메인 서고(ssh 강제명령 → `gateway.ts`, `library_access.sh`가 발급) | 서브 manifest `library` 블록, authorized_keys 줄(위 「고정 표면」), 질의·반출 결과 | 블록·줄 모양 L1, 요청·응답 형식 L1 |
| 파생 선언 | `campaign_init --emit-slice <node>` → 릴레이 stdin | 서브용 캠페인 조각 | 키 L2 |

## 메인/서브 버전 혼재 정책

단계적 배달(w-* → n-*) 동안 메인과 서브의 하네스 판이 다를 수 있다. 서브는 자기 최초 온라인 빌드로 ELF를 만든다(D2·D9). 다만 다음은 예외다.
- **메인 판으로 도는 ELF:** 서브 러너 ELF 4개(`sub_runner`·`regen_build_patches_src`·`inline_json`·`native_remote`)와 서브 실행 루트의 `native_wheelhouse`는 메인이 빌드해 배달마다 둔다.
  - 그래서 이 다섯은 **배달 직후부터 서브가 그 크레이트를 다시 빌드하기 전까지** 메인과 같은 판이다. 서브의 저장소 런처·데이터와는 판이 다를 수 있다.
  - 단서(F1): 서브가 `build_rust_packages.sh`(upstream runtime을 빌드한다, `:27`·`:52`)로 자기 판을 빌드하면 서브 러너 ELF 4개가 있는 `target/release/`가 서브 판으로 바뀐다. 다음 배달이 다시 메인 판으로 덮는다.
    그래서 "늘 같은 판"은 보증이 아니다. 배달과 호출 사이에 서브 빌드가 끼면 서브 판이 돈다. 판 확인 방법(예: 배달한 ELF의 build-id를 메인이 기억하고 호출 전에 대조)은 c43이 정한다(제안).
  - `native_wheelhouse`는 실행 루트에 두므로(`up.rs:693-695`) 서브 빌드가 덮지 않는다.
- **서브 판으로 도는 런처:** ssh로 부르는 서브 런처(`blackbox_session`·`budget_renew_loop`·`regen_envelope`·`mem_watchdog`)는 서브가 빌드한 ELF로 돈다.
  메인이 새 플래그·서브커맨드를 쓰기 시작해도 서브 판은 아직 모를 수 있다.

1. 위 A2A 표의 형식은 캠페인 동안 **추가만** 한다. 키·플래그·종료 코드의 삭제·개명·의미 변경은 `fixed-surface` divergence이고, 메인·서브를 같은 배달과 그 뒤 노드 정비(n-*)에서
   함께 바꾼 다음에만 쓴다.
   - 메인 판 ELF(위 다섯)의 입력 형식(`native_remote` 요청 키, `sub_runner` 인자)은 메인 배달 한 번에 함께 바뀐다.
   - 그 ELF가 읽는 서브 쪽 데이터(서브 작업 트리의 파일·경로)는 서브 판이다. 그래서 그 데이터 형식은 추가만 한다.
2. 모르는 키를 조용히 지우지 않는다(`types.md` 4절 「영속 문서」).
   - 열린 데이터: 읽기 전용 소비자는 모르는 키를 무시하고, 새 키가 없으면 옛 의미로 읽는다. 이전한 크레이트의 serde 구조체는 `deny_unknown_fields`를 쓰지 않는다. 다시 쓰는 writer는 모르는 키를 보존한다.
   - **예외(닫힌 스키마 — base가 모르는 키를 거부한다):** 이 자리는 무시도 보존도 아니고 거부를 보존한다(같은 종료 코드·reason code, 기록 불변).
     - 서브 리포트 `task-report.schema.json`(`additionalProperties: false`, 최상위와 객체 여럿). 서브가 새 키를 더하면 옛 판 메인 스키마가 리포트를 거부한다.
       리포트 키 추가는 메인 스키마를 먼저 배달하고, 서브 판을 그 뒤에 올린다.
     - evidence_publisher 발행 기록(`publisher.rs:530-577`).
     - 캠페인 상태 검증의 닫힌 자리(`campaign.schema.json`, `campaign_validator.rs:727`·`:976-977`).
     - 닫힌 자리에 키를 더하려면 스키마를 먼저 바꿔 배달하고(`surface-change`), 그 뒤에 쓰는 쪽을 올린다.
3. 메인은 n-* 회수로 서브의 `git log -1`과 빌드 로그(`exit=0`)를 확인하기 전에는 서브 런처가 새 형식을 말한다고 가정하지 않는다.
   - "서브 빌드 exit=0"은 서브가 빌드한 ELF만 보증한다. 메인이 배달하는 ELF 다섯은 보증하지 않는다.
   - 그 다섯은 배달 때의 ELF 매직·아키텍처 대조(`deliver_sub_runner_elfs`)와 그 배달 뒤의 첫 호출 성공으로 확인한다.
4. YAML 1.1 → 1.2 전환(c30·c22) 뒤에도 서브 데이터 이전(wd-yaml) 전까지 서브 쪽 로더는 옛 판일 수 있다. 같은 파일을 두 판이 읽어도 같은 값이 되도록 이전 모드는
   의미가 같은 표기(`true`/`false`)로만 바꾼다(`yaml_compat compare3`의 구 파서 = 새 파서 조건).
5. 버전 혼재 중 실패는 노드 정비 정지선(plan 「정지선」 "서브 온라인 빌드를 워커가 회수하지 못한다")으로 사람에게 넘긴다.

## 세션 도구와 남은 Python 기동 판정 (D16·D17 ④, h12 → h16)

- **블랙박스 세션 도구는 이미 Rust다.** `upstream-version-watch/rust/runtime/src/native_multinode.rs:1842` `session_interpreter()`는 늘 `bash`를 돌려준다.
  - 도구는 Rust 런처 `.claude/skills/terraforming_node/scripts/node_blackbox/blackbox_session.sh`(bin `blackbox_session`)다. 서브는 `render_sub_env`가 배달한 `.claude/runtime/node_blackbox/`다.
  - 옛 실행 상태가 옛 이름을 적었으면 `session_launcher()`(같은 파일 :1847)가 형제 `.sh`로 바꾼다.
  - 추적 `.rs`·`.sh`에서 세션 도구가 Python을 띄우는 자리는 없다(c10 `.rs` 모드 허용목록에 세션 도구 행이 없다). 이식·컨테이너 폴백 판단이 필요 없다.
- **결정됨(h16, venv 경계 원칙 — h12의 pip freeze 판정을 바꾼다):** native 모드에서 upstream이 지휘하는 venv 구축과 그 venv의 실행은 런타임 환경이다(범위 밖, 계수에서 뺀다).
  - venv 구축은 생성 `python3.12 -m venv`와 그 venv 인터프리터·실행체로 도는 pip install·list·check·freeze, purelib 질의, import 검사다.
  - 그 venv의 실행은 ray·`ray status`, vllm 서빙·벤치 클라이언트다. 컨테이너 모드의 `docker build`·컨테이너 안과 같은 자리다.
  - 그 venv 인터프리터로 **저장소의 `.py`**를 돌리면 하네스로 센다. 하네스 로직이 venv로 숨는 것을 막는다.
  - 하네스는 이 과정을 지휘·판정·기록하는 upstream의 Rust 로직이다.
  - 그래서 서빙 venv의 `pip freeze --all`(rs 허용목록 12행, `native_multinode/up.rs:720`, `native_multinode.rs:1116` `pip_freeze_argv`)은 런타임 환경이다.
    c43c가 dist-info 읽기로 바꿀 의무는 없어졌다. 바꿀지는 c43c가 정한다(바꾸면 `pip-freeze-*.txt`의 파일 이름·proof 키는 L1이다, 「영속 데이터」).
- native 모드의 구조(그대로):
  - native multinode는 vLLM을 호스트 venv에서 띄운다(`native_multinode.rs:1234` `serve_argv` = 트리플렛 `.sh` 러너, `:1200` `venv/bin/ray start`).
  - 그 venv는 `up --apply`가 노드마다 부르는 `native_wheelhouse verify --venv <root>/venv`(`up.rs:712`, `native_multinode.rs:1105`)가 만들고 채운다.
  - verify가 import 탐침으로 고른 `env`(LD_LIBRARY_PATH 등)가 곧 서빙 환경이다(`up.rs:715` `parse_verify_verdict` → `:735` `runtime_env_recipe` → `:764` `runtime_env`).
- 런타임 환경 모드(container|native)를 닫힌 값으로 두는 일과 그 판정·린트, upstream SKILL.md native 절은 `c35-runtime-mode`가 맡는다.
- **parity `--self-check` 'strace-sees-python'(rs 허용목록 13행, c60):** strace 계수기가 실제 python 기동을 세는지 보려고 일부러 `python3 -c pass`를 띄운다.
  `pyexec0`는 이름 규칙으로 세므로 같은 이름의 대역(`python3`라는 이름의 다른 프로그램)도 센다. c60은 이 자기검사를 pyexec 범위 밖으로 빼거나
  계수기 규칙을 바꾸는 쪽을 정해야 한다(`entrypoints.tsv` `parity_self_check` 행).

### wheelhouse 기동 4–12행의 분류 (c43c 입력)

행 번호는 `rs_python_allowlist.tsv`의 데이터 행 순번이다(`#` 줄 제외). 분류는 둘이다.
- (가) native 서빙 venv(h16: 런타임 환경). 영구 목록 후보다.
- (나) 컨테이너로 옮길 검사(D16).

허용목록의 키는 파일과 호출 텍스트다. 그래서 같은 원문 줄이 어느 호출 경로에서 도는지는 가르지 못한다. 행마다 호출 경로를 함께 적는다. 경로는 모두 `upstream-version-watch/rust/runtime/src/` 기준이다.

- 경로 N(native 서빙): `native_multinode_serve up --apply` → `native_multinode/up.rs:712` → `wheelhouse_verify_argv`(`native_multinode.rs:1105`, `--venv` = `venv_dir(root)` `:1097`)
  → `wheelhouse_cli.rs:213-214` → `wheelhouse_verify.rs:513` `run_verify` → `:222` `verify`. 메인은 런처(`native_multinode.rs:1029`)로 돌고, 서브는 메인이 run root에 둔 ELF
  (`:1042`, `up.rs:695`)를 고정 러너로 돈다.
- 경로 S(자기시험): `native_wheelhouse --self-test` → `wheelhouse_cli.rs:198-199` → `wheelhouse_selftest.rs:382` `self_test` → `body`. 임시 폴더(`:383`)의 가짜 이미지 루트로
  wheelhouse를 만들어 쓰고, 그 안에서 `verify`를 parity 모드로 두 번 부른다(`:299` `venv_parity`, `:307` `venv_parity_neg`).

| 행 | 파일:줄 | 기동 | 경로 N | 경로 S | 근거 |
|---|---|---|---|---|---|
| 4 | `wheelhouse_selftest.rs:357` | `python3.12 -m venv <tmp>/venv` | — | (나) | 자기시험 (3)의 임시 venv. 서빙과 무관하다 |
| 5 | `wheelhouse_selftest.rs:360`·`:368` | 그 venv의 `pip install --no-index`, 가짜 배포 import | — | (나) | 같은 (3). 대상은 가짜 배포(gamma·alpha·ns·zeta)다 |
| 6 | `wheelhouse_verify.rs:274` | `python3.12 -m venv <venv>` | (가) 서빙 venv 생성 | (나) | 경로 N에서 `<venv>` = `<root>/venv` |
| 7 | `wheelhouse_verify.rs:280`·`:327` | `<venv>/bin/python -m pip list --format json` ×2 | (가) 설치 전 기준과 설치 집합 게이트 | (나) | `installed_set` 게이트(`:490` `base_gates`) |
| 8 | `wheelhouse_verify.rs:290` | `<venv>/bin/python -m pip install --no-index [--no-deps] …` | (가) 서빙 venv 설치 | (나) | 경로 N은 승인 파일을 주므로 parity(`--no-deps`)다 |
| 9 | `wheelhouse_verify.rs:325` | `<venv>/bin/python -m pip check` | (가) 설치 게이트 | (나) | `pip_check` 게이트 |
| 10 | `wheelhouse_verify.rs:385` | `<venv>/bin/python -c "import sysconfig;…purelib"` | (가) 서빙 env 계산 | (나) | purelib로 `ld_library_path_template`의 `{VENV_SITE}`를 채운다(`:388-401`) |
| 11 | `wheelhouse_verify.rs:435` | `<venv>/bin/python -c IMPORT_PROBE`(`:16`) | (가) import 검사와 서빙 env 선택 | (나) | 후보 env마다 torch·vllm import와 CUDA를 확인한다. 통과한 후보의 env가 서빙 env다(`:488-489`) |
| 12 | `native_multinode.rs:1116` | `<root>/venv/bin/python -m pip freeze --all` | (가) 설치 집합 기록(h16) | — | `up.rs:720`. 메인은 직접, 서브는 고정 러너 |

- 요약:
  - (가)는 경로 N의 6–12행(7행, 8기동)이다.
  - (나)는 4–5행(3기동)과 경로 S의 6–11행이다.
  - 4–5행은 (나)에만 쓰이고, 6–11행은 같은 원문 줄이 (가)와 (나)에 함께 쓰인다.
- c43c에 주는 함의: (나)를 vLLM 이미지 `docker run`으로 옮기려면 `verify`의 실행 자리를 둘로 갈라야 한다.
  - 예를 들면 기동을 실행자(호스트 | 컨테이너) 뒤로 모으고, 호스트 실행자의 python 기동은 경계 모듈 한 파일(예: `native_venv_host.rs`)에만 둔다. 자기시험은 컨테이너 실행자만 쓴다.
  - 그러면 4–5행은 0이 되고, 6–12행은 그 경계 모듈의 행으로 옮겨 아래 영구 목록 후보가 된다.
  - 컨테이너 실행자의 argv(`docker run … <image> python3.12 …`)를 `inline_python_scan`이 호스트 기동으로 세지 않게 하는 규칙은 c43c가 정한다. c23-rs-guard 결과를 보고 정한다(c10 검토 r1 비차단 D3·D4·I-2·K3과 같은 부류).
  - D17 ③에 따라 컨테이너 클래스 검사는 메인노드에서 실제 PASS가 1회 이상 있어야 한다.
- 참고(검토 code-review B): rs 허용목록 파일 14–19줄(데이터 행 6–11)의 사유 열은 "container 클래스로 옮긴다"라고 적는다. 이 말은 경로 N 몫(h16: 런타임 환경)과 어긋난다.
  그 열은 이 회차에 고치지 않는다(지시). 분류는 c43c와 그때의 사용자 HITL이 확정한다.

### 런타임 환경 기동과 "임시 허용 0" (제안 — 구현과 확정은 c43c, 그때 사용자 HITL)

1. 목록을 둘로 나눈다.
   - `rs_python_allowlist.tsv`(임시, 대체 cell 열)는 캠페인 끝에 데이터 행이 0이 된다.
   - 런타임 환경으로 남는 기동은 새 파일 `.claude/rust/py-boundary-guard/rs_runtime_env_allowlist.tsv`(영구)에 둔다.
     - 열은 `<path>\t<call text>\t<count>\t<boundary>\t<reason>`이다. `boundary`는 경계 이름이다(지금은 `native-venv` 하나, h16).
     - 키·개수·`RS-STALE` 규칙은 임시 목록과 같다. 승인은 이 파일이 아니라 캠페인 `approvals.tsv`(kind `allowlist`)에 둔다.
2. h16에 따른 후보:
   - rs 허용목록 6–12행(native venv의 생성·pip install·list·check·freeze·purelib·import 검사)은 경로 N 몫이 영구 "런타임 환경" 목록 후보다.
   - 4–5행(자기시험 임시 venv)은 D16대로 container로 옮긴다.
   - 6–11행의 경로 S 몫도 container로 옮긴다. 그러려면 c43c가 실행 자리를 위처럼 갈라야 하고, 같은 원문 줄이 두 목록에 걸리지 않게 한다.
   - 이 분류는 c43c와 그때의 사용자 HITL이 확정한다. 이 회차에는 rs 허용목록 파일을 고치지 않는다.
3. 검사(`inline_python_scan check`):
   - 두 목록에 같은 키가 있으면 실패한다.
   - 영구 목록의 `path`는 검사기에 상수로 박힌 경계 모듈 집합 안이어야 한다(예: c43c가 만드는 경계 모듈 한 파일). 파일을 늘리려면 검사기 코드를 바꾸고 검토를 받는다.
   - `boundary`는 검사기 상수 집합 안의 이름이어야 한다.
   - 출력에 `RS-TEMP-ALLOW <n>`과 `RS-RUNTIME-ENV <n>`을 따로 낸다. "임시 허용 0"은 `RS-TEMP-ALLOW 0`과 위반 0으로 판정한다.
   - 행 변경이 사람 결정으로 올라오는 길은 git 이력과 approvals 검사다. 파일 digest를 시험 상수로 다시 적지 않는다(policy:GIT_SINGLE_AUTHORITY).
4. 동적 계수와 이름을 맞춘다. 아래 「라이브 E2E 계수 경계」가 빼는 기동은 모두 `native-venv` 경계에 속한다. 정적 목록에 없는 경계를 동적 규칙이 빼지 않는다.
5. 대안: 한 파일에 `class`(`temp`|`runtime-env`) 열을 더할 수도 있다. 그러면 열 수가 바뀌어 기존 행과 시험을 모두 고쳐야 하고, "임시 0"이 걸러서 센 값이 되어 덜 분명하다. 그래서 두 파일을 권한다.

## 소스 본문·경로 결합 (4회차, 검토 2회차 R2-6)

다른 코드가 하네스 `.rs` 파일의 **본문**(토큰·문구·줄 모양)이나 **경로**(존재·이름)에 묶인 자리다. 이전 cell이 그 파일을 옮기거나 이름·문구를 바꾸면 읽는 쪽이 깨진다. 토큰과 문구는 L1이다(`types.md` 1절).
- 훑은 명령과 결과 수(M `orch/g-c0` `18f849be` 위, 4회차):
  1. `git grep -n '\.claude/[^ ]*\.rs' -- ':!**/checks/**' ':!**/tests/**' ':!*.md' ':!**/contract/**'` → 1016줄. 원장·데이터 파일 4개가 930줄(`branch_layer_ledger.json` 468, `base_skill_edges.json` 178, `claim_bindings.json` 149, `registry.yaml` 135), `rs_python_allowlist.tsv` 13, 나머지가 src·스크립트 약 35파일이다.
  2. `git grep -nE 'include_str!|read_to_string\([^)]*\.rs|"[a-z_]+\.rs"' -- '*.rs' ':!**/checks/**'` → 109줄. 약 80줄은 도움말 `.txt`·`.usage`의 `include_str!`이고, `#[path]`·`include!` 모듈 배선은 이름을 바꾸면 컴파일 오류가 날 뿐이다. 조각으로 경로를 만드는 독자(`claims_recipe.rs` `rt_src`, `conformance_checks.rs` 등)는 손으로 더했다.
  3. `git grep -n 'current_exe' -- '*.rs' ':!**/checks/**'` → 31줄. 형제 실행체를 찾는 9줄은 아래 「형제 실행체」 표다(자기 재실행 4, 저장소 루트 찾기 18).
- 열 "M gate": 그 읽기가 M의 크레이트 gate(`cargo test`)에서 실제로 도는지다.
  - policy-predicates는 `checks/predicate_checks.rs :: goldens`가 술어 75개를 실제 트리에서 돌린다(아래 "PP").
  - 단 **`claims_text` 29건·`claims_owner` 6건의 술어 골든은 M 기준선 실패 집합이다**(c19 기준선, shim 때문에 막힘 — 검토 2회차 검증자 Y). 그래서 그 두 파일이 읽는 결합이 깨져도 M gate는 잡지 못하고 W(`harness_verify`)에서만 드러난다. 표에는 "PP(기준선 실패)"로 적는다.
  - "W만"은 M gate에서 돌지 않고 W의 훅·`verify_distribution`·`harness_verify`·실제 동기화에서만 도는 것이다.
- **이전 cell 의무:** 아래 표의 대상 파일을 옮기거나 그 토큰·문구·줄 모양을 바꾸는 cell은 같은 cell에서 읽는 쪽을 고치고, 읽는 쪽 크레이트의 gate를 돈다. "W만"·"PP(기준선 실패)" 행은 M gate가 대신하지 못하므로 그 결합을 밀폐 시험으로 따로 확인하거나 사람에게 넘긴다.
  이름 개선(rust-repack)도 같다. 함수를 `impl`·`mod` 안으로 옮기는 것도 본문 변경이다(`support.rs:493-501` `rust_top_fn`은 0열 `fn` 머리와 0열 `}`를 찾는다).
- 경로 약자: `tf/` = `.claude/skills/terraforming_node/rust/src/`, `hp/` = `.claude/skills/hint-publisher/rust/src/`, `ab/` = `.claude/skills/adversarial-benchmark/rust/src/`, `up/` = `.claude/skills/upstream-version-watch/rust/runtime/src/`, `rx/` = `.claude/skills/vllm-recipe-explorer/rust/runtime/src/`, `rc/` = `.claude/rust/role-contract/src/`, `pp/` = `.claude/rust/policy-predicates/src/`.

| 대상 파일 | 읽는 자리 | 토큰 또는 존재만 | 깨질 때 동작 | M gate |
|---|---|---|---|---|
| `tf/agent_control.rs:273`(오류 문구 `"$.target: role/transport mismatch"`) | `pp/claims_text.rs:770-774`(A2A_IDENTITY_PROOF_FAIL_CLOSED.C3) | 문구 `role/transport mismatch`(L1 — 오류 문구지만 대조된다) | 술어 FAIL | PP(기준선 실패) |
| `tf/render_sub_env.rs` | `pp/claims_text.rs:648-651`(SUB_GIT_LOCAL_ONLY.C2), `:749`·`:777-781`(A2A.C4) | 0열 `pub fn render_tree(`가 `render_file("CLAUDE.template.md", "CLAUDE.md", "md", &mut produced)`를 품는다. `Agent_Card.template.json`을 품고 `signing_key`·`a2a_signing`·`trusted_keys.json`·`runtime/a2a`가 없다 | 술어 FAIL | PP(기준선 실패) |
| `tf/relay.rs`·`tf/relay/*.rs`(selftest 빼고) | `pp/claims_text.rs:794-796`(`support.rs:504-520` 경유, LIBRARY_GROUNDING_FAIL_CLOSED.C5) | 금지 토큰 `serve_library_requests`·`library_request`가 없다 | 술어 FAIL. `relay.rs`가 없으면 파일 없음 FAIL, `relay/`가 옮겨지면 **조용히 덜 훑는다** | PP(기준선 실패) |
| `rc/scan_node_probe.rs` | `pp/claims_text.rs:750`·`:755-758`(A2A.C1) | `BatchMode=yes`, `host-key 미등록/불일치`, `키 인증 거부` | 술어 FAIL | PP(기준선 실패) |
| `.claude/rust/policy-gate/src/completion_gate.rs` | `pp/claims_text.rs:762-775`(A2A.C2·C3) | `approval_atoms`·`approved_by:`·`plan_sha256`·`allowed_actions`·`EXECUTION_APPROVAL_ACTION_NOT_ALLOWED` | 술어 FAIL | PP(기준선 실패) |
| `rc/scan_node_manifest.rs`, `rc/manifest_gate.rs`, `up/smoke_model.rs`, `rx/model_cli.rs` | `pp/claims_owner.rs:14-18`·`:121-149`·`:166-172`(MODEL_ACQUISITION_TERNARY_GATE.C1·C2, HOST_SAFETY_LAYERED_DEFENSE.C3) | `pub const VALID_MODES: [&str; N] = [managed, ephemeral, custom]`, `smoke_model.rs`의 `pub const PROGRAM`·`Command::new(` 개수 2·**들여쓰기까지 정확한** `Command::new("bash")\n` + 20칸 + `.arg(&spec_script)`, `crosscheck_spec`의 `--hf-repo-id` opt 모양 등 | 술어 FAIL(함수 이름이 바뀌면 AttributeError) | PP(기준선 실패) |
| 같은 `rc/scan_node_manifest.rs`·`rc/manifest_gate.rs`·`up/smoke_model.rs` | `pp/companion.rs:20-24`·`:80-105` | 위와 같은 토큰 | 단위 시험 FAIL | PP |
| `hp/tag.rs`·`hp/core.rs`(push 경로) | `pp/companion.rs:28-55`·`:71`, `pp/claims_hint.rs:22`·`:681-701`·`:751-758`(HINT_TAG_ACTIVATION_GATE.C1)·`:1278-1287`(C5)·`:1549-1553`(LAST_GOOD_ROLLBACK_ANCHOR.C3) | 0열 함수 `push_tag`·`_tag_ref`·`check_ref_format`·`git_push_authenticated`·`tref`와 그 본문 토큰(`let refspec = format!("{r}:{r}");`, `format!("refs/tags/{tag}")`, `pub const HINT_TAG_PREFIX: &str = "hint/";` 등), `--cleanup=verbatim`, 금지 토큰(`--tags`·`--all`·`--mirror`·`--force`·`refs/tags/*`), 인자 `remote: &str` | 술어·단위 FAIL | PP |
| `hp/cli/mod.rs`·`hp/cli/*.rs`, `hp/**/*.rs` | `pp/claims_hint.rs:24`·`:558-569`·`:1398-1404`(C6), `:1554-1555`, `:20-21`·`:1121-1169`(C4) | `.default("origin")`(`build_parser`), `HINT_APPROVAL_ABSENT`와 호출 순서, 금지 토큰(ssh·relay·`render_sub_env`·`relay.sh` 등) | FAIL. C4는 금지 토큰이 생길 때만 FAIL이고, 파일이 범위 밖으로 옮겨지면 **조용히 빠진다**(비어 있지 않음만 본다) | PP |
| `ab/scan_node_cli.rs`, `rc/scan_node*.rs`, `rx/recipe_cmd.rs`, `up/render_template.rs`, `ab/campaign_write.rs`, `policy-publisher/src/publisher.rs` | `pp/claims_tf.rs:222-296`·`:349-357`·`:407-416`·`:577-584`·`:741-822` | 정확한 줄(`if a.emit_manifest && gate_ok && (…) {` 등), 함수 본문 토큰(`require_terraform_flag(&root)` 1회, `doc.set("grounding"` 등), 금지 토큰(`std::fs`·`read_to_string` 등) | 술어 FAIL | PP |
| `rc/topology.rs`, `up/render_template.rs`, `up/slave_forward.rs` | `pp/claims_lib.rs:65-68`, `pp/claims_upstream.rs:31`·`:87-89`·`:139-140`·`:173-189` | `pub fn layer_files(`~`pub fn read_layer_header(` 사이 토큰, `pub fn build_context(` 인자, `pub const VALIDATED_SOURCE_BUILD_KEYS: [(&str, &str); N] = [`, slave_forward 존재 | 술어 FAIL | PP |
| `rx/run_trial_session.rs`, `rx/cleanup_*.rs`, `rx/model_card.rs`, `rx/recipe_cmd.rs` | `pp/claims_recipe.rs:24-28`·`:207-216`·`:339-353`·`:400-406`·`:469-535` | 함수 본문 토큰(`start_memwatch`, `if !apply {` 순서, `load_config` 다음 줄 등) | 술어 FAIL | PP |
| `pp/claims_*.rs`(자신) | `pp/claims.rs:124-145`(`include_str!`) | 각 `pub fn <술어>(` 본문에 `req(`가 하나 이상 | 컴파일 오류·FAIL | PP |
| `up/bin/sub_runner.rs:146`·`:173`·`:177` | `.claude/rust/policy-distribution/src/checks.rs:698-699`·`:735-739` | `let owner = format!(`·`while lo > 0 && is_chars(cs[lo - 1])`·`if token != owner`(파일이 없으면 빈 문자열로 읽는다) | 검사 `sub_retirement_consumer_audit_before_tombstone` 실패 → verify 종료 1 | 예(`policy-distribution contract_checks :: sub_runner_source_tokens_are_audited`) |
| `tf/provider_claude_code.rs` | `.claude/rust/policy-distribution/src/checks.rs:39-57`·`:856-865`, `sync_branches.sh:166-184`·`:828-846` | 경로가 `ROOT_RELOCATION_REPLACEMENTS`와 같다, 원본 가지 트리에 있다 | 검사 실패 / `sync_branches` 종료 4 | W만 |
| `up/layer_ledger.rs` | `sync_branches.sh:494-510`·`:545-553`, `closing_sequence.sh:166-173`, `up/branch_sync_selftest.rs:48`·`:553-898` | 원본 가지에 있음 + 바이트 동일(`hash-object` 대 `rev-parse`) | 종료 3 `LEDGER_JUDGE_STALE` / 종료 10 `COMMON_LAYER_UNJUDGED` / 닫기 순서 중단 | W만 |
| `tf/render_sub_env.rs`, `hp/catalog.rs` | `up/branch_sync_selftest.rs:23-41`·`:490-493`·`:761-775`·`:1152-1158` | 존재, 정확한 줄 `    let mut mismatch = topology_rules_mismatch(ctx, &topology, None);`, `pub const EXIT_LOCAL_TAG_OBJECT_MISSING … = <n>;` | 자기시험 FAIL | W만(`selftest_branch_sync`는 M에서 돌지 않는다) |
| `hp/tag.rs` | `up/push_branches_selftest.rs:79-112` | `pub const PUSH_SCOPE_ARGS: [&str; N]` 줄 하나, N = 항목 수, 항목 = `push_branches::SCOPE_ARGS`. 파일은 `current_exe`·cwd의 조상에서 찾는다 | `--self-test` 비0 | 예(upstream `contract_checks :: consumer_self_tests_pass`) |
| `tf/render_sub_env.rs`·`tf/render_sub_env/selftest.rs` | `up/bin/sub_runner.rs:113-125` `RETIREMENT_DECLARING_OWNERS` | 정확한 경로(면제) | 이름이 바뀌면 면제를 잃어 retirement-scan이 걸리고 `sync_to_sub` 실패 | W만 |
| `hp/core.rs`·`hp/evidence/*.rs`, `completion_gate.rs`·`policy-publisher/src/gate.rs`, `ab/resolve_topology_selftest.rs`·`ab/sweep_repeats_fixture.rs`, `ab/*.rs` 사본 | `tf/render_sub_env/selftest.rs:470-484`·`:505-514`·`:600-620` | `pub const REL_X: &str = "…";` 줄(배달·분류 대상 파생), `load_schema` 호출, 런처 경로 문자열, 스킬 소스 참조 | 자기시험 FAIL. `evidence/`가 옮겨지면 **조용히** 상수가 줄어든다 | 예(terraforming `consumer_self_tests_pass`) |
| `hp/tag.rs:51`·`:52`·`:1315` | `tf/check_sub_hint_equip.rs:563-571` | `pub const TOKEN_KEY: &str = "GITHUB_TOKEN";`, `pub const TOKEN_ENV_FILE_REL: &str = "envs/.env";`, `pub fn read_push_token(` | 검사 FAIL | 예(같음) |
| `tf/agent_control.rs`·`tf/provider_claude_code.rs`, `tf/relay*.rs` | `tf/agent_control/selftest.rs:14-16`·`:120-125`, `tf/relay/selftest.rs:645-657`(`include_str!`) | 표지 `// --self-test (plan_26093022`, `claude -p` 등 토큰의 위치, 시간 상한 리터럴의 부재 | 컴파일 오류·자기시험 FAIL | 예(같음) |
| `up/render_dockerfile.rs` | `hp/core.rs:39` → `hp/artifacts/a1.rs:1834-1866`·`:2069-2076`, `hp/artifacts/selftest.rs:488-490`·`:1095-1103` | git 이력의 측정 시점 판과 작업 트리의 **파일 전체 바이트 동일**, 존재, `pub const RUNNER_SCRIPTS` 정규식 | 본문을 조금만 고쳐도 이전 측정의 재렌더가 거절된다(fail closed). 파일이 없으면 renderer_revision 행이 **조용히 빠진다** | 측정 재렌더는 W만, 자기시험은 예(hint `artifacts_cases :: artifacts_selftest_is_clean`) |
| `ab/campaign_validator.rs:22-23` | `rx/recipe_cmd/simulate_body.rs:22`·`:119-140`, `rx/gmu_roles_selftest.rs:36-39`, `hp/artifacts/selftest.rs:1758-1795` | `pub const LOCKSET_PROVENANCE: [&str; 2] = [`·`pub const KNOB_SOURCES: [(&str, &[&str]); 3] = [` — **배열 길이까지 리터럴**, 행 모양 `("knob", &["a", …]),` | `recipe simulate` 종료 5("소유자가 없다"·"… 가 없다"), 자기시험 FAIL. 노브를 하나 더하면 recipe가 종료 5다 | 예(recipe `gmu_roles_checks`, hint `artifacts_selftest_is_clean`) |
| `ab/bench_naming.rs` | `hp/core.rs:49` → `hp/lineage.rs:437-470`·`:557-610` | `pub const KIND_DEFAULT_EXTENSIONS` 블록, 그리고 `fn parse_conventional(` 바로 위 **문서 주석 줄**(`:344`)을 문법으로 읽는다 | `HINT_OWNER_MODULE_MISSING` / `HINT_LINEAGE_GRAMMAR_DRIFT` | 예(hint `lineage_cases :: lineage_selftest_passes`) |
| `completion_gate.rs`, `tf/blackbox_session.rs:1747`, `ab/repeat_owner.rs:459-460`, `ab/assemble_sweep_meta.rs:1045-1056` | `hp/evidence/e1.rs:411-419`·`:653-683`(빌드 체크아웃의 `CARGO_MANIFEST_DIR`에서 읽는다), `hp/evidence/selftest.rs:1298-1304`·`:2275-2279`, `hp/naming.rs:238-243`·`:2484-2490` | 이름 10개(`parse_flat_certificate` … `CERTIFICATE_NON_ISSUING_AUTHORITIES`)의 정의 줄, `store(&["--mem-total-mib"]).int().required()`, 문구 두 줄, 여러 줄 리터럴 셋 | `HINT_GATE_API_MISSING` / 자기시험 FAIL | 예(hint evidence·naming cases) |
| `rx/recipe_cmd/simulate_body.rs:797`·`:805`, `ab/{classify_cell,…,assemble_sweep_meta}.rs` | `ab/conformance_checks.rs:163-275` | `reconciliation`, `cap = cap_declared - lite_spent`, `reconciliation_cap 소진`, `read_lite_verdict` 등(없는 파일은 빈 문자열) | 보고 FAIL → 종료 1 | 예(adversarial `contract_checks :: self_test_conformance_full_superset_lite`) |
| `rx/recipe_cmd.rs` | `ab/campaign_write.rs:5002-5007` `device_total_gib`(`--cell-materialize`) | 존재만 | **조용히** `미상(판정 소유자 부재: <경로>)` | W만(M 시험은 그 파일이 없는 임시 루트라 늘 미상 갈래) |
| `rx/run_trial.rs`·`rx/run_trial_session.rs` | `rx/anchor.rs:49-58` → `rx/generation_cli.rs:172-196` | 후보 필드 이름 토큰 | 종료 2 "parity: run_trial 소스를 읽지 못했다" | 예(recipe `consumer_self_tests_pass`) |
| `up/slave_forward.rs`, `.claude/rust/runtime-support/src/schema.rs` | `.claude/rust/policy-audit/src/arch.rs:14`·`:28-33`, `policy-audit/src/selftest.rs:15-50`·`:82-112` | 존재, 0열 `pub fn validate_against_schema` | `ARCH_VARIANT_PORT_SELECTOR_WIRING_UNREADABLE`, 자기시험 종료 1 | 자기시험은 예(`registry_self_test_passes`). 실제 트리의 verify는 M 시험이 종료 1도 받으므로 W만 |
| `registry.yaml` 증거 경로·`claim_bindings.json`·`pp/claims*.rs` 이름 | `policy-audit/src/registry.rs:26-33`·`:865-965`·`:1060-1215` | 존재(추적·정규 파일) + 0열 `[pub] const`·`static`·`fn`… `<assertion_id>` 정의 줄, 바인딩 튜플 일치, 술어는 `claims*.rs` 안 | `EVIDENCE_MISSING_FILE`·`EVIDENCE_ASSERTION_ID_NOT_FOUND`·`CLAIM_BINDING_MISMATCH`(verify 종료 1) | W만(M `registry_verify_envelope_and_exit_code`는 종료 0과 1을 모두 받는다) |
| `base_skill_edges.json`의 간선 | `.claude/rust/policy-selftest/src/tripwires.rs:1074-1206`(tripwire ⑨) | 정책 소스에서 훑은 `skills/<경로>` 문자열 집합과 같아야 한다(존재 확인 없음) | "미등재 간선"·"사라진 간선" → pre-commit 차단 | W만(훅) |
| `branch_layer_ledger.json`의 `.rs` 행 | `up/layer_ledger.rs:16`·`:988-1010` | 경로 기록(추가만, 존재 확인 없음) | 없음(다음 닫기 순서에서 새 미분류 차이가 된다) | W만 |
| `rs_python_allowlist.tsv` 13행의 `.rs` | `.claude/rust/py-boundary-guard/src/rs_scan.rs:26`·`:90-127` | 경로 + 정규화한 호출 텍스트 + 개수 | `RS-STALE`·`RS-VIOLATION` → 종료 1(본문을 고쳐 호출 텍스트가 바뀌어도) | 예(`inline_scan_checks :: this_repository_passes`) |
| `tf/render_sub_env.rs`의 줄 `    let card_path = render_file("Agent_Card.template.json"`, `hp/tag.rs`의 줄 `    args.push(refspec);` | `.claude/rust/policy-predicates/checks/data/mutations.json`(M05·M11, 위 훑기 밖) | 줄 전체(변이 시험의 닻) | "mutation anchor missing" | PP |
| `ab/` 등 경로 정규식 범위(`rc/turn_budget.rs`, `ab/turn_budget*.rs`, `ab/(resolve_)?bench_tool*.rs`) | `.claude/rust/policy-selftest/src/antipattern.rs:33-47` | 경로 정규식(금지 모양 훑기의 범위) | 없음 — 파일이 옮겨지면 **조용히** 범위 밖 | W만 |

**한 번의 이름 바꾸기에 함께 고칠 곳(lockstep):**
- `tf/provider_claude_code.rs`: `sync_branches.sh:179`, `policy-distribution checks.rs:52`, 실제 트리.
- 정책 소스·`registry.yaml`·`claim_bindings.json`이 이름을 적은 스킬 `.rs`: 읽는 쪽 상수, `registry.yaml`, `claim_bindings.json`, `base_skill_edges.json`(tripwire ⑨의 "미등재"와 "사라진"이 함께 걸린다). 원장은 손대지 않는 것이므로(요청서 §1) 이 경우는 사람에게 넘긴다.
- `tf/render_sub_env.rs`: `sub_runner.rs:119-120` 면제, PP 술어, `branch_sync_selftest`, parity TB 골든.

### 형제 실행체 (`current_exe` 옆의 다른 bin, L1)

| 부르는 자리 | 형제 bin | argv와 읽는 것 | 없을 때 |
|---|---|---|---|
| `ab/classify_cell_owner.rs:319-336`(`:221-243`이 쓴다) | `node_identity_resolve`(같은 폴더, `deps/`면 한 단계 위) | `--repo <repo> --source-owner <node_identity.sh 경로>`, 종료 0이어야 하고 stdout = node id, stderr 앞 두 줄을 사유로 | `CONFIGURATION_ERROR` "…`node_identity_resolve` is not installed next to this program…". 비0이면 `node_identity.sh --resolve rc=…` |
| `ab/resolve_topology.rs:150-163`(`:132-137`, `:557-582`) | `topology_parity` | `evaluate --repo <repo> --format json`, 종료 0·5일 때 stdout JSON `verdict`·`topology`·`reasons`·`legs` | `CONFIGURATION_ERROR` "…`topology_parity` is not installed next to this program…" |
| `ab/resolve_topology_selftest.rs:139-172` | `bench_shell`·`repeat_axis(_api)`·`bench_preset(_api)`·`topology_parity`·`resolve_topology(_api)` | 직접 부르지 않고 픽스처의 native 폴더로 복사한다 | 복사 실패 → 자기시험 실패 |
| `ab/bin/sweep_stop_bench.rs:166-173` | `sweep_stop` | `--state - --now-utc <NOW>`, stdin 상태, 출력은 버린다 | panic(벤치 도구, 게이트 아님) |
| `hp/cli/selftest_e2e.rs:24-36`·`:1870-1874` | `render_bench_section`(환경 변수로 내보냄), `hint` | `hint match --vllm … --model … --json`(env_clear), 종료 0과 JSON 목록 | 내보내지 않음(조용히, 픽스처 런처가 따로 찾는다) / e2e 자기시험 오류 |
| `up/native_multinode/self_test.rs:334-337` | `native_remote` | `sh -c` 통로로 `<native_remote> -`, stdin JSON 요청(`op`·`root`·`run_id`·`marker`·`created_by`) | 검사 FAIL, 뒤 원격 검사들이 "transport unavailable" |
| `rx/bin/selftest_gmu_roles_batch.rs:19-22` | `recipe` | `recipe simulate …`(arg0 = 사본의 `target/release/recipe`), 종료 코드·stdout·lockset | 종료 2 "the program under test is missing" |
| `up/branch_sync_selftest.rs:656-673`·`:1127-1137` | `layer_ledger`와 upstream `target/release`의 ELF 전부(저장소 루트로 찾는다) | 진짜 ELF를 가짜 루트로 링크하고 자기를 `layer_ledger`로 복사, 고장 모드에서 진짜 `layer_ledger`를 같은 argv로 exec | `read_dir` 실패 / rc 1(W만) |

- 이전 cell 의무: 이 bin들의 이름·argv·출력을 바꾸면 같은 cell에서 부르는 쪽을 고친다. bin을 다른 크레이트로 옮기면 "같은 폴더" 전제가 깨진다(`target/<profile>/`가 달라진다).

## 라이브 E2E 계수 경계 (초안 3판, h16 · D17 ②)

T `strace_scan.awk`·`w_verify.sh`(e-* ③)와 e-* 업무지시가 쓸 규칙이다. 오케스트레이터가 이 초안으로 T와 업무지시를 고친다.
- 지금의 이름 규칙(FORMAT.md 「safe_run」 `python_exec`: path나 argv[0]의 basename이 `^python[0-9.]*$`인 성공 execve)은 그대로 둔다.
- 센 기동을 "하네스" 몫과 "런타임 환경" 몫으로 가른다. 판정은 하네스 몫만 0을 요구한다.
- 범위는 h16 venv 경계 원칙이다(「세션 도구와 남은 Python 기동 판정」). `<W>`는 하네스 저장소 루트, `V`는 서빙 venv, `S`는 서빙 venv 집합이다.
- 3판(g-c0 검토 2회차)에서 고친 것: venv 생성의 ensurepip 자손(E1), CPython argv 해석(E2), superseded와 줄 모양(E3), F 좁히기(E4), 런타임 zone 안 시스템 python의 계수(E5), run root 위치(E6).

### 입력

- strace 명령: `strace -f -q -s 4096 -e trace=execve,execveat,clone,clone3,fork,vfork -e signal=none -o <log>`. 한 파일에 pid 머리를 붙여 쓴다(`-ff`가 아니다).
  - 지금 형식(`-e trace=execve,execveat`, FORMAT.md 「w_verify 입력」)에는 부모 사슬이 없다.
  - `-s` 기본값 32는 argv 문자열을 줄인다(`trunc=1`).
  - `-e signal=none`은 `--- SIG… ---` 줄을 끈다. `-q`는 붙이기·떼기 알림을 끈다. `+++ exited`·`+++ killed`·`+++ superseded` 줄은 `-e trace`·`-e signal`과 상관없이 찍힌다.
- 받아들이는 줄 모양(닫힌 목록). 머리는 모두 `<pid> `다. 이 밖의 줄은 읽지 못한 줄이며 판정을 내지 않고 실패한다(fail-closed).
  1. 완결 syscall: `<syscall>(<args>) = <ret>`. `<ret>`는 수, `-1 E<NAME> (<설명>)`, `? <unavailable>`(프로세스가 syscall 도중 사라짐), `? ERESTART<…> (<설명>)`(신호로 끊겨 다시 시작) 중 하나다.
  2. 끊긴 진입: `<syscall>(<args> <unfinished ...>`.
  3. 이어진 결과: `<... <syscall> resumed><나머지 args>) = <ret>`(`<ret>`는 1과 같다).
  4. 신호: `--- SIG<NAME> {…} ---`. `-e signal=none`이면 나오지 않지만, 나오면 무시한다.
  5. 끝: `+++ exited with <N> +++`, `+++ killed by SIG<NAME>[ (core dumped)] +++`, `+++ superseded by execve in pid <T> +++`.
  - 결과가 `?`이거나 `ERESTART…`인 execve는 성공이 아니다. 그런 clone 계열은 `fork fail`이다(다시 시작한 호출은 새 줄로 나온다).
- `strace_scan.awk` 행(제안). 모든 행 끝에 로그 줄 번호 `seq`를 더한다. 끝에 덧붙이므로 지금 열 위치를 읽는 쪽은 그대로 돈다.
  - `exec <ok|fail> <path> <argv0> <pid> <argv> <trunc> <seq>`: 지금 행에 `seq`만 붙인다. `<unfinished ...>`로 갈린 execve는 진입 줄의 argv와 결과 줄을 짝지어 한 행으로 낸다. `seq`는 진입 줄이다.
  - `fork <ok|fail> <parent pid> <child pid|-> <entry seq>`: `clone`·`clone3`·`fork`·`vfork`. 진입 줄(완결이거나 `<unfinished ...>`)과 결과(같은 줄의 `= N` 또는 그 pid의 다음 `<... <syscall> resumed> … = N`)를 짝짓는다. 스레드 생성도 든다.
    - 한 pid에 끝나지 않은 syscall은 한 번에 하나뿐이다. 그래서 짝은 "같은 pid의 같은 syscall 이름의 다음 resumed 줄"이다. 예외는 아래 superseded다.
  - `exit <pid> <seq>`: `+++ exited with N +++`·`+++ killed by SIG… +++`.
  - `supersede <leader pid> <tid> <seq>`: `+++ superseded by execve in pid <tid> +++`(리더 pid의 줄).

**superseded(E3):** 리더가 아닌 스레드 T가 execve하면, 커널이 T를 리더 pid L로 바꾼다. strace는 L의 줄로 `+++ superseded by execve in pid T +++`를 찍고, 그 execve의 결과를 L의 `<... execve resumed>` 줄로 찍는다.
- 규칙: **T의 진입 상태를 리더 pid L로 넘긴다.**
  - L의 지금 수명은 끝나지 않는다. L의 상태를 T가 execve에 들어갈 때의 상태로 바꾼다.
  - T의 끊긴 execve 진입은 L로 옮긴다. 그래서 L의 다음 `<... execve resumed>` 줄이 그 진입과 짝을 이룬다.
  - T의 수명은 여기서 끝난다(상태를 지운다).
- 진입을 찾지 못하면(T에 끊긴 execve가 없음) 읽지 못한 줄과 같이 실패한다.

### 알고리즘

1. **1패스(지도):** 로그 전체에서 child → parent 지도를 만든다.
   - `fork ok` 행마다 (자식 pid, 부모 pid, 진입 seq)를 담는다.
   - 자식 pid의 **수명**은 그 pid가 진입 seq 뒤에 처음 나타난 행부터 그 pid의 `exit` 행(또는 그 pid가 T인 `supersede` 행)까지다.
   - 한 pid가 끝난 뒤 다시 쓰이면(pid 재사용) 새 수명이다.
   - `fork` 행은 진입 seq 뒤의 가장 이른, 아직 다른 `fork`가 차지하지 않은 그 pid의 수명에 붙인다.
   - vfork·`posix_spawn`(clone3 + `CLONE_VFORK`)은 자식의 execve 줄이 부모의 `resumed` 줄보다 먼저 찍힌다. 그래도 진입 줄은 늘 자식보다 먼저다. 그래서 이 지도가 순서 뒤집힘을 푼다.
2. **2패스(판정):** 행을 `seq` 순으로 읽는다. `fork` 행은 진입 seq 자리에서 읽는다.
   - `fork` 행에서 부모의 **진입 시점 상태**를 복사해 그 자식 수명에 담는다(상태는 아래). 자식 행이 진입 뒤에만 오므로 복사는 늘 자식보다 먼저다.
   - 어떤 pid의 수명에서 첫 행을 볼 때, 지도에 부모가 있으면 담아 둔 상태로 시작한다.
     없으면 뿌리 상태(`zone = harness`, `nm` 꺼짐, `prog` 없음)로 시작하고 `orphan_pids`를 1 늘린다. 첫 pid(strace가 띄운 명령)도 뿌리 상태다.
   - `exec` 행(진입 seq)에서 그 pid의 **진입 상태**를 적어 둔다(superseded가 쓴다). `exec ok`이면 아래 「분류」를 한 뒤 그 pid의 상태를 바꾼다.
   - `supersede` 행은 위 규칙대로 한다. `exit` 행에서 그 pid의 상태를 지운다. 같은 pid의 다음 행은 새 수명이다.

pid 상태:
- `prog`: 그 pid의 마지막 성공 exec(path·argv). 자식은 부모의 것을 물려받는다(exec 전 fork 자식은 부모 프로그램이다).
- `nm`: 조상에 `native_multinode_serve up --apply`가 있었다. 성공 exec의 path basename이 `native_multinode_serve`이고 argv[1] = `up`이며 argv에 `--apply`가 있으면 켠다.
- `zone`: `harness` 또는 런타임 뿌리 id(`RE-VENV`·`RE-TRIPLET`).

서빙 venv 집합 S(2패스가 늘린다): 성공 exec가 다음을 모두 만족하면 그 `V`를 S에 더한다(`native_multinode.rs:1105` argv 꼴).
- path basename이 `native_wheelhouse`이고 argv[1] = `verify`이며 그 pid의 `nm`이 켜져 있다.
- argv에 `--venv V`와 `--wheelhouse-dir H`가 있다. `dirname(V) = dirname(H)`(= run root), `basename(V) = venv`, `basename(H) = wh`다. 곧 경로 접두는 `<run root>/venv/`다.
- V와 H는 절대 경로이고 `.`·`..` 성분이 없다. 아니면 S에 더하지 않는다(빼지 않는 쪽).
- venv는 그 verify 안에서 처음 만들어진다. 그래서 V를 쓰는 exec는 늘 이 행 뒤에 온다.
- **전제(E6):** run root(`campaign.topology_sections.<t>.native_run_root_base` 아래)는 `<W>` 밖이다. V가 `<W>` 아래이면 w_verify는 `run_root_in_repo=1`을 적고 아무것도 빼지 않는다(모두 `python_exec`). 아래 재진입 규칙도 V 아래 path를 하네스 프로그램으로 보지 않는다.

### CPython argv 해석 (E2 — 닫힌 규칙)

이름 규칙에 걸린 성공 exec의 argv[1:]을 CPython 3.12 명령행 문법(`python [option…] (-c cmd | -m mod | file | -) [arg…]`)으로 읽어 `mode`와 `script`를 정한다. 이 해석에서 벗어나면 fail-closed로 `mode = unknown`이다.
1. 원소를 앞에서부터 본다.
   - `--`: 다음 원소가 `script`다(`mode = file`). 다음 원소가 없으면 `mode = stdin`이다.
   - `-`: `mode = stdin`. 끝.
   - `--check-hash-based-pycs`: 다음 원소를 값으로 먹는다. `--check-hash-based-pycs=<값>`은 붙은 형이다.
   - `--version`·`--help`·`--help-env`·`--help-xoptions`·`--help-all`: 값 없는 플래그다. 그 밖의 `--…`는 `mode = unknown`이다.
   - `-<글자들>`(짧은 옵션 묶음): 글자를 왼쪽부터 본다.
     - 값 없는 플래그 `b`·`B`·`d`·`E`·`h`·`i`·`I`·`O`·`P`·`q`·`R`·`s`·`S`·`u`·`v`·`V`·`x`·`?`는 다음 글자로 간다.
     - `W`·`X`: 남은 글자가 있으면 그것이 값이고(붙은 형 `-Wignore`·`-Xdev`), 없으면 다음 원소가 값이다. 그 원소에서 끝난다.
     - `c`·`m`: 남은 글자가 있으면 그것이, 없으면 다음 원소가 명령·모듈이다(`mode = c`·`m`). 해석은 끝나고 나머지는 프로그램 인자다.
     - 그 밖의 글자는 `mode = unknown`이다.
   - `-`로 시작하지 않는 원소: `script`다(`mode = file`). 끝.
   - 값을 먹어야 하는데 원소가 없으면 `mode = unknown`이다.
2. 원소가 다 떨어졌는데 `mode`가 없으면 `mode = stdin`(대화 모드)이다.
3. `trunc=1`인 exec는 `mode = unknown`이다.

### 분류

이름 규칙에 걸린 성공 exec E(pid p, 그 직전 `prog` = P)를 다음 순서로 가른다. 런타임 환경으로 빼려면 **E의 execve path**를 본다. argv[0]은 보지 않는다.

1. **소스를 알 수 없거나 V 밖 스크립트면 하네스다.** 어느 zone이든 같다.
   - `mode = unknown`·`stdin`이면 `python_exec`다.
   - `mode = file`이면 `script`가 다음을 모두 만족할 때만 2로 간다. 하나라도 어기면 `python_exec`다.
     - 절대 경로다(`/`로 시작). 상대 경로는 cwd를 모르므로 하네스로 센다.
     - `.`·`..` 성분이 없다.
     - 어휘 정규화(겹친 `/`와 끝 `/`를 하나로) 뒤 어느 V(V∈S)의 `V/` 아래다.
   - `mode = c`·`m`이면 2로 간다.
2. **RE-VENV-CREATE:** `mode = m`, 모듈 `venv`, argv의 마지막 원소가 V(V∈S)이고, P가 `native_wheelhouse verify --venv V`이면 런타임 환경이다(`wheelhouse_verify.rs:274`).
   이 exec 뒤 p의 `zone`을 `RE-VENV`로 둔다(E1). 그래서 venv가 띄우는 ensurepip 자손이 3의 런타임 뿌리 자손이 된다.
3. **RE-VENV:** E.path가 `V/` 아래(V∈S)인 경우다.
   - p의 `zone`이 런타임 뿌리이면(그 자손) 런타임 환경이다. vllm·ray가 `sys.executable`로 띄우는 `-c`(multiprocessing spawn)·`V/` 아래 워커 스크립트, venv 생성의 ensurepip가 여기 든다.
   - p의 `zone`이 `harness`이면(하네스가 직접 띄움) argv[1:]이 닫힌 꼴 F일 때만 런타임 환경이다. F 밖이면 `python_exec`다. F는 다음 다섯이다.
     - `-m pip list --format json`, `-m pip check`, `-m pip freeze --all`(바이트 그대로).
     - `-m pip install --no-index [--no-deps] --find-links H/wheelhouse -r H/requirements-closure.txt`(바이트 그대로, `H = dirname(V)/wh`). 다른 인자·다른 경로가 하나라도 있으면 F가 아니다(E4). 그래서 `pip install -e <W>`·`<W>` 아래 요구 파일은 F 밖이다.
     - `-c` + purelib 문자열(`wheelhouse_verify.rs:385`)과 바이트가 같은 것, `-c` + `IMPORT_PROBE`(`:16`)와 strace 이스케이프를 푼 바이트가 같은 것.
     - 근거 줄은 `wheelhouse_verify.rs:280`·`:290-294`·`:325`·`:327`·`:385`·`:435`, `native_multinode.rs:1116`이다. V 아래 스크립트는 F에 넣지 않는다(E4).
4. 그 밖은 `python_exec`(하네스)다. 런타임 zone 안이라도 path가 어느 V 아래도 아닌 이름 규칙 기동(예: 시스템 `python3`)은 여기 든다.
   - **E5 판단:** 이 기동은 하네스 몫으로 센다(fail-closed). h16의 경계는 "venv 인터프리터·실행체"이고, 런타임이 띄운 시스템 python은 그 밖이다.
     라이브에서 나오면 e-* ③이 실패하고, 오케스트레이터가 그 기동을 런타임 경계에 넣을지 사람에게 묻는다(경계를 넓히는 결정이라 HITL).
     검토가 싸게 되도록 `python_exec_in_runtime_zone`(그중 런타임 zone 안에서 나온 수)과 `zone=<뿌리>`를 함께 낸다.

zone 바뀜(분류 뒤, 성공 exec마다):
- `RE-VENV`: E.path가 `V/` 아래(V∈S)이면 p의 `zone`을 `RE-VENV`로 둔다. python이든 `V/bin/ray`·`V/bin/vllm`·`V/bin/pip` 같은 진입점 스크립트든 같다.
  분류 2(RE-VENV-CREATE)인 exec도 p의 `zone`을 `RE-VENV`로 둔다(E1).
  셔뱅 스크립트는 strace에 스크립트 path로만 나타나므로 이름 규칙에는 걸리지 않는다. 그 자손은 이 zone을 물려받는다.
- `RE-TRIPLET`: E.path basename이 `bash`이고 argv[1]이 `^<W>/output/[^/]+/configs/[^/]+\.sh$`이며 p의 `nm`이 켜져 있으면 `RE-TRIPLET`으로 둔다(`native_multinode.rs:1234` `serve_argv`, `native_triplet.rs:396`).
  트리플렛 러너가 직접 띄우는 `V/bin/python -m vllm…`이 3의 "런타임 뿌리 자손"으로 든다.
- 재진입: `zone`이 뿌리인 pid가 하네스 프로그램을 exec하면 `zone`을 `harness`로 되돌린다. 하네스 프로그램은 다음 셋 중 하나다. 단 path가 어느 `V/` 아래이면 하네스 프로그램이 아니다(E6).
  - path가 `<W>/.claude/` 아래다.
  - path basename이 `consumers.tsv` `bin` 열의 이름이다.
  - path basename이 `bash`·`sh`이고 argv[1]이 `<W>/.claude/` 아래다.
- 이 밖의 exec는 `zone`을 바꾸지 않는다.

fail-closed:
- 로그에 `fork` 행이 하나도 없는데 pid가 둘 이상이면 부모 사슬을 모른다. 이때는 아무것도 빼지 않고(모두 `python_exec`) `boundary_mode=no-process-trace`를 적는다.
- 부모를 모르는 수명(지도에 없음)은 뿌리 상태 곧 `harness`로 시작한다(`orphan_pids`). 결과를 못 찾은 `<unfinished ...>` clone의 자식이 여기 든다.
- S·뿌리·F 판정에 쓰는 exec가 `trunc=1`이면 그 조건은 거짓이다(빼지 않는다).
- 받아들이는 줄 모양 밖의 줄, 짝을 못 찾은 superseded는 판정을 내지 않고 실패한다.
- S가 비면 venv 쪽 규칙(2·3)이 아무것도 빼지 않는다. `run_root_in_repo=1`이어도 같다.

### 빼지 않는 것

- venv 인터프리터로 돈 저장소 `.py`와, V 밖 스크립트·상대 경로·`.`/`..` 경로·stdin 스크립트(분류 1). 하네스 로직이 venv로 숨는 길을 막는다.
- 하네스가 V 인터프리터로 직접 띄운 F 밖의 `-c`·`-m`·스크립트(분류 3). V 아래 스크립트도 하네스가 직접 띄우면 센다.
- 자기시험과 단독 verify(경로 S, `nm` 없는 `native_wheelhouse verify`)의 venv·pip·import. V∉S이므로 `python_exec`다. (나)로 컨테이너에 옮기면 0이 된다.
- 이 로그에 verify가 없는 옛 run root의 venv(V∉S).
- 하네스 bin·런처·훅이 띄우는 시스템 python 기동, 런타임 zone 안의 시스템 python(E5).

### 사례

빼는 사례(양성, h16 런타임 환경):

| 사례 | 판정 길 |
|---|---|
| `python3.12 -m venv V`(P = `native_wheelhouse verify --venv V`, `nm` 켜짐) | 분류 2. p의 `zone` = `RE-VENV` |
| 그 자식 `V/bin/python3.12 -m ensurepip --upgrade --default-pip`(CPython 3.12 `venv/__init__.py` `_setup_pip` → `_call_new_python`, `--without-pip` 없음 `wheelhouse_verify.rs:274`) | `mode = m`. E.path가 V 아래이고 p의 `zone`이 `RE-VENV`(물려받음) → 분류 3 런타임 |
| 그 손자 `V/bin/python3.12 -W ignore::DeprecationWarning -c <pip 부트스트랩>`(`ensurepip/__init__.py` `_run_pip`) | `-W`가 값을 먹고 `mode = c` → 분류 3 런타임 |
| 하네스 zone의 `V/bin/python -m pip install --no-index --no-deps --find-links H/wheelhouse -r H/requirements-closure.txt` | F → 런타임. 그 뒤 p의 `zone` = `RE-VENV` |
| ray 워커 `V/bin/python -u V/lib/python3.12/site-packages/ray/_private/workers/default_worker.py …` | `-u` 플래그, 절대 V 아래 스크립트 → 분류 3 런타임 |

반대 사례(빼면 안 되는데 단순한 규칙에 걸리는 경우와 그 막음):
1. "path가 venv 아래면 뺀다"는 접두 규칙만 쓰면 `V/bin/python <W>/.claude/…/x.py`(venv로 숨은 하네스)를 뺀다. 막음: 분류 1(V 밖 스크립트는 늘 하네스).
2. 하네스가 `V/bin/python -c '<하네스 로직>'`을 직접 띄우면 접두 규칙에 걸린다. 막음: 분류 3의 하네스 zone은 닫힌 꼴 F만 뺀다.
3. 사람이나 에이전트가 `native_wheelhouse.sh verify --venv /tmp/x/venv --wheelhouse-dir /tmp/x/wh …`를 따로 돌리면 verify 꼴과 같다. 막음: S는 `nm`이 켜진 verify에서만 모은다.
4. vfork·`posix_spawn` 자식의 execve가 부모의 clone 결과 줄보다 먼저 찍힌다(검토 실험 300회 중 3회).
   로그 순서대로 상속하면 자식이 부모 상태를 못 받아 흔들리고(거짓 FAIL), 앞 수명의 상태를 덮어 빼면 fail-open이 된다. 막음: 1패스 지도와 진입 시점 복사.
5. pid 재사용으로 새 프로세스가 죽은 런타임 프로세스의 zone을 물려받는다. 막음: `exit`에서 상태를 지우고 수명마다 지도를 붙인다.
6. `exec -a <V>/bin/python /usr/bin/python3 …`처럼 argv[0]만 꾸민 기동. 막음: 빼는 조건은 execve path로 본다.
7. 트리플렛이나 vllm이 하네스 런처를 부르고 그 런처가 python을 띄우면 뿌리의 자손이 된다. 막음: 재진입.
8. `V/bin/python -X dev <W>/x.py`: "옵션이 아닌 첫 argv"만 보면 `dev`를 스크립트로 읽어 `<W>/x.py`를 놓친다. 막음: CPython argv 해석(`-X`가 값을 먹는다) → `script = <W>/x.py` → 분류 1 하네스.
9. 런타임 zone 안의 `V/bin/python .claude/x.py`(상대 경로, cwd = `<W>`): 글자 접두로는 `<W>/` 아래로 보이지 않아 런타임 자손으로 빠진다. 막음: 분류 1(상대 경로는 하네스).
10. `V/bin/python V/../../<W>/x.py`·`V/bin/python -m pip install -e <W>`: 앞은 `..` 성분, 뒤는 F 밖 인자다. 막음: 분류 1과 F의 바이트 일치.

### 사각(계수 밖이고, 이 규칙으로 메우지 않는다)

- **systemd 데몬:** 블랙박스 데몬(collect·events·lifecycle·regen-envelope, 「런처와 환경 변수」)은 PID 1의 자손이다. 그래서 E2E strace 트리에 나타나지 않는다.
  그 bin의 Python 0은 정적 검사(`inline_python_scan`)와 설치 검증(`verify_node_blackbox.sh`, 설치본 ELF 매직)으로만 확인된다.
- **서브 노드:**
  - 서브의 verify·pip freeze·ray worker·서브 런처는 메인이 ssh로 서브에서 띄운다(`native_multinode.rs:784` `run`, `:791` `start`, 「A2A 와이어 형식」). 메인 로그에는 `ssh` exec만 남는다.
  - w_verify는 메인 strace만으로 "서브 하네스 python 0"을 말하지 않는다.
  - 서브 계수 방법(제안만, 구현은 e-* 업무지시와 c43 몫): 서브 고정 러너의 `run`·`start` op를 서브에서 같은 strace 명령으로 감싸는 계수 모드를 둔다(환경 변수 하나로 켠다).
    서브 로그는 문서 기반 회수(`fetch_sub_docs.sh`)로 메인에 가져와 같은 규칙으로 판정한다. 그 전까지 서브 몫은 정적 검사와 서브 워커의 자기 보고로 확인한다.
- **셔뱅 Python:** 셔뱅 스크립트는 strace에 스크립트 path로만 나타나서 이름 규칙이 세지 않는다. venv 밖의 셔뱅 Python을 하네스가 띄우는 자리는 정적 검사(`inline_python_scan`, c23-rs-guard)가 맡는다.
  런타임 zone 자손이 V 아래에 하네스가 써 넣은 스크립트를 띄우는 것도 정적 검사 몫이다.
- **심볼릭 링크:** 어휘 정규화는 링크를 풀지 않는다. V 안에 `<W>`를 가리키는 링크를 두면 `V/link/x.py`가 V 아래로 보인다. 하네스가 V 안에 링크를 만들지 않는지는 정적 검사가 맡는다.
- **컨테이너:** docker 컨테이너 안 프로세스는 dockerd·containerd-shim의 자손이라 추적 트리에 없다(h16: 런타임 환경). `docker run|exec … python…`은 argv[0]이 docker라 이름 규칙 기동이 아니다.
  추적 트리에 `runc`·`crun`·`conmon`(rootless podman 등)의 exec가 보이면 이 초안은 그 자손을 빼지 않는다. 센 뒤 사람에게 넘긴다.
- **모듈·인라인 코드의 출처:** 런타임 zone의 `-m <module>`·`-c`가 cwd·`PYTHONPATH`를 통해 저장소 코드를 읽는지는 strace(execve만)로 알 수 없다. 하네스가 그런 argv를 만들지 않는지는 정적 검사가 맡는다.

### 출력(제안)

- `python_exec`: 하네스 몫이다. e-* ③은 이것만 0을 요구한다. 그중 런타임 zone 안에서 나온 수 `python_exec_in_runtime_zone`을 따로 낸다(E5).
- `python_exec_runtime_env`와 분류(`RE-VENV-CREATE`·`RE-VENV`)·zone별 수.
- `serving_venvs`(S), `boundary_mode`(`process-trace`|`no-process-trace`), `orphan_pids`, `run_root_in_repo`, `unknown_argv`(분류 1의 `mode = unknown` 수).
- 뺀 기동은 숨기지 않는다. 수와 처음 몇 줄(pid·path·argv 앞부분·zone)을 보인다.

## 계약 시험에 반영할 것 (c21 a·b·c가 다시 열릴 때)

3회차·4회차에 늘어난 계약이다. 크레이트별로 적는다. 각 줄의 근거는 위 절과 `consumers.tsv` `r3:`·`r4:` 행이다. 4회차에 더한 줄은 `(4)`로 시작한다.

- **py-boundary-guard (c21 a):**
  - (4) `crate_allow`의 하네스 면제는 닫힌 목록이다(`src/crate_allow/harness.rs` `HARNESS_CRATES`, 16 크레이트의 manifest 경로·이름·판). 목록 밖의 source 없는 패키지는 `path-unknown`이다.
    - 그래서 `contract_checks.rs` `crate_repo`(`:166`)의 경로 패키지 `a 0.1.0`은 무엇을 추적하든 `path-unknown`이다. 이 픽스처를 쓰는 시험 셋이 지금 실패한다:
      `crate_allow_check_marker_and_findings`(`:202`), `crate_allow_path_package_needs_a_tracked_manifest_beside_its_lock`(`:224`, 끝의 "자기 lock을 갖추면 통과"를 고정), `crate_allow_no_tracked_lock_and_non_utf8_argument_are_exit_2`(`:265`, UTF-8 이름 대조가 통과를 기대).
    - 고칠 방향: 픽스처의 하네스 패키지를 목록 안의 크레이트로 바꾼다. 예: `.claude/rust/py-boundary-guard/Cargo.toml`(`py-boundary-guard 0.1.0`)과 그 옆 `Cargo.lock`을 같은 경로로 두고 추적한다(`crate_allow_checks.rs` `clean_repo`가 그렇게 한다).
      "자기 lock을 갖춘 17번째 크레이트 → `path-unknown`"으로 단언을 뒤집는다. 시험을 빼지 않는다.
  - (4) lock 머리 변형(`[[ package ]]`, 머리 뒤 주석, 들여쓴 머리, 알려진 머리 밖 `[` 줄)은 종료 2다(`lockfile.rs` `KNOWN_HEADERS`). 계약 표면에 더할지 정한다. 셈은 `crate_allow_checks.rs :: lock_table_headers_cargo_does_not_write_exit_2`가 한다.
- **upstream-runtime (c21b):**
  - `sub_runner` 세 서브커맨드의 인자·stdin·stdout 줄·종료 코드(「고정 표면」). 밀폐 실행 가능하다(임시 폴더). c21b 대응표의 "실행·출력 표면 없음" skip을 고친다.
  - `native_remote -`: base64 JSON 요청의 키와 `op` 11종, 결과 키, 허용 밖 op 거절. c21b 대응표의 "전제 틀림" skip을 고친다. `exists`·`text`·`tree`처럼 호스트에 닿지 않는 op로 밀폐할 수 있다.
  - `resolve_torch_pin`: `"torch_prefix": "<v>"`가 한 줄에 있는 pretty 출력(L1). c21b 대응표의 L2·skip을 L1로 고친다.
  - `layer_ledger append`·`merge`: 원장 항목과 최상위의 모르는 키(`_reach`·`_note` 등)가 그대로 남는다(키 보존). (4) `"k": null`과 키 없음을 둘 다 넣은 항목이 그대로 남는다(`types.md` 4절 3 (b)).
  - (4) `layer_ledger divergence`의 diverged 사례(CLI): 대상 가지에만 공통층 커밋을 둔 원장 저장소에서 종료 코드·JSON `status`(diverged)·`inline_json divergence-summary` 첫 칸을 고정한다.
    `sync_branches.sh:526-541`은 diverged면 종료 9로 멈춘다(유일한 차단). 지금 c21b 시험에 diverged 사례가 없다(검토 P2-B1).
  - (4) `judge_version_delta --write-resolved`: `resolved.json`의 모르는 키가 그대로 남고, 손저작 `upstream_delta_*`가 `_superseded_manual_*`로 옮겨진다(`judge_delta.rs:1205-1229`). `upstream_delta` 블록의 키는 L2다(hint `e1.rs`·`e3.rs`, recipe `cleanup_docker.rs`가 읽는다).
  - `regen_build_patches_src unbundle|verify --root`: ssh로 서브에서 부르는 형식.
- **terraforming (c21b):**
  - 블랙박스 데몬 4종(`blackbox_collect`·`blackbox_events`·`logs_lifecycle`·`regen_envelope`)을 런처 없이 띄운다.
    - ELF를 임시 폴더에 복사해 cwd `/`, argv[0] = 그 사본 경로, `TERRAFORMING_OWNER_PATH` 없이 `--self-test`가 종료 0이다.
    - 유닛 argv(`--node-dir <d> --interval 1` 등)가 받아들여진다.
  - (4) 블랙박스 표본 CSV(「영속 데이터」): `blackbox_collect`를 임시 node-dir로 몇 표본만 돌려(`max_samples`가 있다, `blackbox_collect.rs:538`) `samples/<UTC일자>.csv`의 머리 줄 전체, 열 순서, 1열 정수, 줄 단위로 자라는 것을 고정한다.
    지금 c21b 시험은 `ts,` 접두만 본다(검토 I2-N4). `thermal_watchdog.sh`의 awk(1열 정수·`NF>=5`·5열)가 그 파일을 읽을 수 있는지도 같은 시험에서 본다.
  - `library_access`: manifest `library` 블록의 줄 모양(키·JSON 문자열 인용)과 authorized_keys 줄 모양·표지 `wiki-desk-gateway:<id>`.
    wiki-desk `accessor.ts` 쪽 대조는 wiki-desk 시험 몫이다(TS, c45와 함께).
  - (4) manifest 줄 모양 writer: `render_sub_env`가 쓰는 서브 manifest에 최상위 `self_role: <slug>` 줄이 있고, 그 파일을 `node_identity.sh`의 grep 꼴(`^self_role:`)로 읽으면 같은 slug가 나온다.
    메인 manifest writer(scan)는 `- role: main` 항목이 정확히 하나다. 밀폐로 돌 수 있는 쪽(`render_sub_env` 자기시험 픽스처)부터 고정한다.
  - `relay`: 릴레이 원장의 모르는 키 보존. `campaign_init --sub-echo` 호출 형식. (4) reslice의 `--rev`는 base 결함(모호한 옵션)이므로 지금 동작(거부)을 고정하지 않는다. c40·c44의 처분을 기다린다.
  - (4) `relay`의 리포트 검사: `task-report.schema.json` 밖 키가 든 리포트는 거부된다(거부 보존).
- **adversarial-benchmark (c21c):**
  - `campaign_init`: 캠페인 상태의 열린 자리(`_*` 최상위 안내 키, `budgets`·`topology_sections`, 단계 상태 파일 최상위)의 모르는 키 보존. `--sub-echo`·`--evidence-relocate`(`consumers.tsv` `r3:` 행).
    (4) `--assign-set`·`--phase-set`의 통째 교체 객체(`assignments`, `proof`)는 키 보존 시험에서 뺀다(`types.md` 4절 3 (a)).
  - (4) 캠페인 검증기 거부 보존: 닫힌 자리(최상위·`node`·`hint_target` 등)에 모르는 키를 넣은 `campaign.yaml`은 검증기(`campaign_template_validator.sh`)가 거부하고, 그 종료 코드를 고정한다.
  - manifest_contract 픽스처 처분(`types.md` 5절)은 c31이 로더를 바꿀 때 바뀐다. c21c는 지금 기대값을 그대로 둔다. (4) c31은 골든 행을 빼지 않고 기대값만 바꾼다.
- **policy-publisher (c21c):**
  - `evidence_publisher`: hint-publisher `e4.rs`가 쓰는 서브커맨드 7개와 플래그 24개(「고정 표면」), stdout JSON `reason_codes`·`messages`, 빈 stdout.
  - (4) `append-raw`는 **거부 보존** 시험으로 바꾼다(3회차의 "모르는 키 보존"은 틀렸다 — 발행 기록은 닫힌 스키마다).
    - 최상위 모르는 키가 든 기록 → 종료 2, `reason_codes`에 `PUBLICATION_RECORD_UNKNOWN_FIELDS`, 기록 파일 바이트 불변.
    - 항목 안(`identity` 등) 모르는 키 → 종료 2, `PUBLICATION_RECORD_SCHEMA_INVALID`, 바이트 불변.
    - 대조: 모르는 키가 없는 같은 기록은 받아들여진다.
    - 지금 `checks/contract_checks.rs:354-377` `append_raw_never_drops_unknown_record_keys`는 base에서 늘 참이다(거부라 다시 쓰지 않는다). 이름은 두고 단언을 위처럼 강화한다(강화만이므로 `test_delta.tsv` 없음).
- **policy-distribution (c21c):** `checks.rs:735-739`의 `sub_runner.rs` 소스 토큰 대조는 계약이다(「고정 표면」). 이 검사를 약하게 하지 않는다.
- **hint-publisher (c21 a):**
  - `evidence_publisher`를 `.py` 상수 → `.sh` 런처로 부르는 길(`core.rs:32`·`:667-669`). 런처가 없으면 fail-closed다.
  - `pip-freeze-{main,sub}.txt` proof 키 검사(`e1.rs:2226-2244`)와 블랙박스 세션 이벤트 키(`e2.rs`).
  - (4) `resolved.json` 독자: `_bind_resolve`(`e1.rs:1980-2027`)가 읽는 키, 사전이 아닐 때 낮은 출처로 가는 길(`:1996`), `HINT_MISSING_RESOLVE`(`e3.rs:1657`).
- **소스 결합 중 M에서 돌 수 있는 것(4, 각 크레이트):** 「소스 본문·경로 결합」의 "M gate" 열이 "W만"·"PP(기준선 실패)"인 행 가운데 밀폐로 돌 수 있는 것을 그 독자 크레이트의 계약 시험으로 옮길지 정한다.
  - policy-predicates: `claims_text`·`claims_owner` 술어가 읽는 토큰(위 표 앞 여섯 행)을 술어 골든과 따로, 실제 트리 파일을 읽어 대조하는 시험. 기준선 실패 집합의 골든과 별개로 돌아야 한다.
  - adversarial-benchmark: `campaign_write.rs:5002-5007`의 `recipe_cmd.rs` 존재 갈래(지금 M 시험은 늘 `미상` 갈래만 본다).
  - upstream-runtime: `branch_sync_selftest`가 대조하는 `render_sub_env.rs`·`catalog.rs`의 줄(W만).
  - policy-audit: `registry verify`가 실제 트리에서 종료 0인지(지금 M 시험은 0과 1을 모두 받는다).
- **공통(이전 cell):** `types.md` 2절 비UTF-8 사례 의무, 4절 키 보존·거부 보존 시험 의무, 이 문서 머리의 재조사 의무(h18).
