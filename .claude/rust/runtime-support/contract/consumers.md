# 소비자 인벤토리 (고정 표면 · 영속 데이터 · A2A)

- 정본: plan 26111805 「보존 계약」·「cell 세부」 c20, interview D8. 쓴 cell: c20-contract (base `ead7087d`). 읽는 cell: c21(bin별 계약 시험), c30~c60.
- 기계 표: `consumers.tsv`(bin × 소비자 파일 한 행). 고정 표면의 수준(L1 바이트·L2 값·L3 자유)은 `types.md` 1절이다.
- 빠진 것을 찾으면 행을 더하고 이 문서의 해당 절을 고친다. c20을 다시 열면 c21도 다시 연다(plan 「진행 규칙」).

## consumers.tsv 열

| 열 | 뜻 |
|---|---|
| `bin` | Cargo bin 이름(16 크레이트, `[[bin]]`과 `src/bin/*.rs`, 201개) |
| `crate` | T `crates.tsv`의 키 |
| `launcher` | 그 bin을 `exec`하는 추적 `.sh`(`;`로 이음, 없으면 `-`) |
| `launcher_env` | 런처의 실행체 덮어쓰기 변수(`<NAME>_BIN`, `TERRAFORMING_BIN_DIR` 등)와 런처가 `export`하는 변수(`(export)`) |
| `consumer` | bin·런처를 부르거나 이름을 적은 추적 파일 |
| `class` | `script`(셸), `hook`(pre-commit), `rust`(다른 크레이트의 src), `other-test`(다른 크레이트의 checks), `ts`(wiki-desk), `same-crate`·`same-crate-other`(같은 크레이트의 다른 bin·lib·스크립트), `own-test`·`test-data`(자기 시험·데이터), `parity`(parity cases), `skill-doc`(SKILL.md·references·규칙 문서), `ledger`(원장 JSON), `other`(템플릿·도움말 텍스트 등) |
| `lines` | 그 파일 안에서 이름이 나온 줄 |
| `invocation` | 소비자가 넘기는 서브커맨드·플래그(`env:NAME` = 그 호출에 주는 환경 변수) |
| `reads` | 소비자가 읽는 것: `exit:<구분하는 코드>`, `stdout-json:<키>`, `stdout-text:<무엇>`, `stderr:<무엇>`, `merged-output`, `file:<bin이 쓴 경로>`, `exec`, `path-only`(경로만 적음), `test`, `comment`, `agent-instruction`(에이전트가 읽는 명령), `golden(…)`, `path-record` |
| `fixed` | `yes`(기계 소비 — 고정 표면), `argv`(문서가 적은 명령행 — 서브커맨드·플래그가 고정), `path`(경로 이름만 고정), `golden`(재기준선 가능), `?` |
| `basis` | `checked` = 사람(검토 에이전트)이 소비자 코드를 읽고 채운 행, `auto` = 아래 규칙으로 뽑은 행(`invocation`·`reads`는 어림) |
| `note` | 기계로 파싱되는 문자열(reason code, 표 머리, 표식 줄) 등 |

뽑은 규칙(재현용): bin 목록은 각 `Cargo.toml`, 런처는 `exec`하는 변수에 `/<bin>` 경로를 대입하는 추적 `.sh`, 소비자는 추적 텍스트 파일에서 런처 파일 이름·`target/release/<bin>`·`CARGO_BIN_EXE_<bin>`이 나오는 줄이다.
`docs/report/*`, `PORTING.md`, `notices/`, parity `data/`, 빌드 스크립트(`build_rust*.sh`, `rust_bootstrap.sh`)는 소비자에서 뺐다(빌드 스크립트는 bin 목록을 나열할 뿐이다).
다른 크레이트·셸·훅·TS 소비자 348행은 `checked`다. 나머지(같은 크레이트, 문서, 시험, parity, 원장)는 `auto`다.

## 고정 표면 — 주요 기계 계약

다른 bin·셸·훅이 기계로 읽는 자리다(`consumers.tsv`의 `checked` 행 중 `reads`가 `stdout-json`·`stdout-text`·`stderr`·`file:`·`merged-output`인 것, 45개 bin).
c21은 아래 계약을 bin별 계약 시험에 우선 고정한다. 근거 줄은 `consumers.tsv`에 있다.

| 생산 bin(런처) | 소비자 | 고정할 것 |
|---|---|---|
| `completion_gate`(`.claude/policies/runtime/completion_gate.sh`) | `sync_branches.sh`·`sync_to_sub.sh`(`authorize --action …`), hint-publisher `src/evidence/e1.rs`(`authorize --mode promotion`), policy-selftest `src/suite.rs:468`(`verify`), policy-publisher `src/publisher.rs`(종료 코드·stdout 그대로 전달), `inline_json`(같은 모양의 게이트 JSON) | 서브커맨드 `authorize`·`verify`와 플래그, stdout JSON `allowed`, `propagation_scope.{status,topology,node_id,ssh_host,work_dir,planes}`, `state`, `eligible_for_promotion`, `reason_codes`(예: `HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE`, `*_GATE_OUTPUT_UNREADABLE`), 종료 코드 0/비0 |
| `runtime_selftest`(`runtime_selftest.sh --tripwires-only`) | pre-commit 훅, policy-gate `src/completion_gate.rs`(stderr 마지막 줄) | 종료 코드, stderr 마지막 줄(사유), `[tripwire] FAIL …` 줄 |
| `antipattern_scan`(tripwire ⑥) | policy-selftest `src/tripwires.rs:853`(`tripwire_cli` → `(rc, stderr)`) | `--root --tripwire`, rc, stderr 꼬리. 입력은 terraforming SKILL.md `## 4. 공개 표면` 표(`policy-selftest/src/antipattern.rs:528` `SURFACE_HEADING`) — 표 머리와 열은 L1 |
| `hint`(`hint.sh`) | policy-distribution `src/checks.rs`(`match … --json`), policy-selftest `src/hint_suite.rs`(publish·lint·continue·verify·push), policy-predicates `claims_hint.rs`, upstream `branch_sync_selftest.rs`(`catalog derive`) | 서브커맨드·플래그, JSON `[].tag`·`[].arch_match`·`allowed`·`action`·`exit_code`·`reason_codes`, stderr `[hint] FAIL <CODE>:` 접두, 종료 코드 이름(`EXIT_LOCAL_TAG_OBJECT_MISSING` 등) |
| `topology_parity`(`topology_parity.sh`) | `sync_to_sub.sh`·`closing_sequence.sh`(런처를 `bash` 없이 직접 실행 — 실행 비트 L1), `render_sub_env` | `evaluate --repo --expect --format json`, JSON `verdict`·`topology`·`reason_codes`(`EXPECT_MISMATCH`)·`legs_judged`·`legs_checked`·`reasons` |
| `node_role_contract` | terraforming `src/bootstrap_canary.rs`, `sync_to_sub.sh` | `evaluate --manifest --topology --field <delivery_plane|tool_plane|sub_mode> --format value|json`, 값 `active`, JSON `<field>.value`·`<field>.source` |
| `manifest_contract`(Flag 게이트) | `run_bench.sh`·`lite_bench.sh`, policy-distribution `checks.rs`, policy-predicates `claims_tf.rs`·`claims_owner.rs`, `native_health_probes.sh`(사설 브리지) | `--topology --repo --require-flag`(`--json`), 종료 0 = Flag 발행·3 = manifest 없음, `MANIFEST_CONTRACT_VALIDATE_ONLY=1`(인자 없음, 종료 코드 그대로), 브리지 `"protocol": "manifest-contract-bridge/1"`와 typed wire `{"t": "int", "sign": "+", "hex": "1"}`(공백까지) |
| `resolve_topology` | `run_bench.sh`·`lite_bench.sh` 등 벤치 스크립트 6개 | `--repo`, stdout `single`|`multi`, 종료 3 = manifest 없음(다른 비0과 구분) |
| `resolve_bench_tool` | policy-predicates `claims_text.rs` | `--json`, 정확히 10키 사전, `status` resolved(0)/absent(3), stderr `ABSENT`·`NOTE` |
| `classify_cell_api`·`parse_guidellm`·`bench_reporting_api` | recipe `classify.rs`·`escalation_bridge.rs`, hint `e4.rs`, policy-gate `doc_naming.rs`, policy-selftest `suite.rs` | `evaluate-jsonl` stdin/stdout 규약: 요청 한 줄 `id`·`operation`·`arguments_json`·`result_json`, 응답 한 줄 `id`·`encoding`=`json-values-v1`·`kind`(value|exception)·`value_json`·`type`·`message`, 종료 비0 또는 stderr 출력 = 실패, `constants` 호출의 `DOWNGRADE_REASONS`·`BENCH_MODE_LITE`·`KILL_EVENT_KINDS`·`EVENTS_RECOVERED_PARTS` |
| `bench_preset`·`repeat_axis`·`verdict_rule`·`sweep_stop`·`bench_shell` | `run_bench.sh`·`sweep_bench.sh`·`lite_bench.sh`·`judge_bench.sh` | `\x1f`로 나눈 stdout 줄(`bench_preset resolve` = `KEY\x1fVALUE`, `default`·`repeat_axis resolve` = `N\x1fSRC\x1fKIND`, `verdict_rule spec-row` 4칸, `sweep_stop report declared-repeats` 2칸), `bench_shell`의 위치 인자 서브커맨드 약 30개와 입력 환경 변수, 판정 낱말 `pass`·`server_failed`·`passed`·`refused`·`not_applicable`·`fresh`·`PASS`·`explicit`, `sweep_stop` 종료 0/3 |
| `render_report`(산출 Markdown) | hint-publisher `render_bench_section.rs`, policy-gate completion_gate 판정 표, `evidence_publisher publish-benchmark` | 스윕 표 머리 `| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |`, `★판정점`, `## 측정 구성 — bench_mode · 도구 · 반복 …`(허용 키 21개), `mode: lite`, `## lite 지표`, `## 판정 (표시만`의 `verdict`·`루브릭 권한` 행 |
| `blackbox_session`(세션 도구) | `single_serve_up.sh`·`single_serve_down.sh`·`multinode_serve_smoke.sh`·`budget_renew_loop.sh`·`run_bench.sh`, recipe `run_trial_session.rs` | 서브커맨드 `declare-budget`·`renew-budget`·`clear-budget`·`budget-status`·`budget-defaults --field ttl_s`·`budget-block`·`budget-skip`·`start`·`stop`, `ttl_s` 숫자만, `budget-status` JSON `label`·`floor_mib`, renew 종료 3 = 선언 교체, `serve_budget.env`의 `floor_mib`·`expires_epoch`·`label` |
| `budget_preflight` | `multinode_serve_smoke.sh`·`single_serve_up.sh` | `--json` 출력의 `"floor_mib"`·`"arm_ceiling_mib"`·`"overhead_max_mib"`·`"decl_min_ceiling_mib"`, 종료 4 = 감시기가 거절할 선언, `--render gmu|reasons|summary`(stdin JSON) |
| `check_smoke_model` | 같은 두 스크립트 | `<CONFIG> --emit-gate-params`, 합친 출력의 `required_mib=<N>`·`BUDGET_PARAMS … ckpt_mib= tp= kv_mib=` 줄, 종료 0/2(모델 없음)/7(RAM 게이트 거절) |
| `layer_ledger`·`slave_forward`·`blackbox_eta`·`blackbox_thermal` | `sync_branches.sh`(`inline_json divergence-summary` 경유), policy-predicates `claims_owner.rs`, hint `a2.rs`·`a3/collect.rs`, 감시기 스크립트 | `divergence` JSON `status`·`basis`·`base`·`sync_point.status`·`absorbed`(stdout/stderr 분리), `slave_forward prefix|json`의 `forwards[].key/group/value`·`sub_recipe`·stderr `[slave-forward] FAIL(<code>): <msg>`, `--emit-params`가 쓰는 `BB_*`·`BB_TP_*` 파일(감시기가 `source`) |
| `render_dockerfile`·`rope_scaling_translate`·`resolve_torch_pin`·`escalation_predicate`·`render_sub_env` | hint `a1.rs`·`a2.rs`, upstream `smoke_model.rs`, `multinode_serve_smoke.sh`, adversarial-benchmark `campaign_write.rs`, upstream `branch_sync_selftest.rs` | `--nccl-envfile`·`--cluster-envfile`·`--env-tier`(JSON `tier`·`detail`), `--json`(`needed`·`hf_overrides`·`native_max_position_embeddings`·…), `torch_prefix`, `predicate_1`·`model_plane_*`, 종료 5/3과 stderr `TOPOLOGY_RUL…` |
| `yaml11_oracle` | wiki-desk `checks/frontmatter.check.ts` | stdin YAML → stdout JSON `[[key, value], …]` (c45가 바꾼다) |
| `crate_allow`(`crate_allow.sh`, c20) | 캠페인 검사 `deps-allow` | `check`, 마지막 줄 `DEPS-ALLOW-PASS`, `DEPS-VIOLATION <kind> <crate>@<version> …` 줄, 종료 0/1/2 |

**텍스트로 읽는 JSON(L1 주의):** 셸 소비자 일부가 JSON을 `sed`·`grep`으로 읽는다. `budget_renew_loop.sh:145`는 `"label": *"…"`를 한 줄 정규식으로 읽고,
`multinode_serve_smoke.sh:883-885`는 `"floor_mib"` 등의 키를 줄 단위로 찾는다. 이전한 bin이 이 출력을 `serde_json` pretty로 쓰면 지금처럼 키와 값이 한 줄에 남는다.
압축(한 줄 전체) 출력이나 키 뒤 줄바꿈으로 바꾸지 않는다. c21은 이 줄들을 L1로 고정한다.

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

## 런처와 환경 변수

- 런처 127개(bin 127개, 런처 하나에 bin 하나): "ELF가 없으면 종료 2 + 안내, 있으면 `exec`" 형식이다. 실행체 덮어쓰기 변수는 88개 런처에 있고(`<NAME>_BIN` 62개, `TERRAFORMING_BIN_DIR` 26개),
  terraforming 런처 26개는 `TERRAFORMING_OWNER_PATH`를 export한다. 그 밖의 export(`BENCH_VERIFY_REPO_ROOT`, `*_REPO_ROOT`, `*_SCRIPTS_DIR`, `HINT_SKILL_DIR` 등)는 `consumers.tsv` `launcher_env`에 있다.
- 소비자가 직접 설정하는 덮어쓰기 변수: `COMPLETION_GATE_BIN`, `RUNTIME_SELFTEST_BIN`, `PY_BOUNDARY_GUARD_BIN`, `CAMPAIGN_READ_BIN`, `CAMPAIGN_VALIDATOR_BIN`, `MANIFEST_CONTRACT_BIN`,
  `TOPOLOGY_PARITY_BIN`, `BENCH_SHELL_BIN`, `TURN_BUDGET_BIN`, `HINT_BIN`, `CAMPAIGN_READ_COMPAT_OWNER`, `CAMPAIGN_READ_VALIDATE_ONLY`, `MANIFEST_CONTRACT_VALIDATE_ONLY`. 이름은 L1이다.
- 런처 51개는 `exec -a "<런처 경로>"`로 argv[0]을 런처 경로로 둔다. 일부 bin은 그 경로에서 저장소 루트를 찾는다(예: `verify_distribution.sh` 머리 주석). argv[0] 해석은 런처↔bin 내부 계약(L1)이다.
  clap으로 바꿀 때 `prog` 이름(도움말 L3)과 루트 탐색(L1)을 섞지 않는다.
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

| 데이터 | 경로 | 쓰는 쪽(근거) | 읽는 쪽(근거) | 형식·수준 |
|---|---|---|---|---|
| 브랜치 레이어 원장 | `.claude/policies/branch_layer_ledger.json` | upstream `layer_ledger`(`src/layer_ledger.rs`, 정식 writer, plan U4) | `sync_branches.sh`·`closing_sequence.sh`, tripwire(`policy-selftest/src/tripwires.rs`·`antipattern.rs`) | JSON, L2. 손편집 금지(요청서 §1) |
| 아치 변종 원장 | `.claude/policies/arch_variant_ledger.json` | 사람 + upstream escalation | policy-audit `src/arch.rs:61`(JSON 로더), policy-predicates `src/claims_upstream.rs`, hint-publisher `src/artifacts/a1.rs`, recipe `src/recipe_set.rs` | JSON, L2 |
| 스킬 간선·루트 등록부·claim 바인딩·정책 등록부 | `.claude/policies/{base_skill_edges,root_registry,claim_bindings}.json`, `.claude/policies/registry.yaml` | 사람(개정 plan) | policy-audit `src/cli.rs:92-101`(registry.yaml 도 JSON 로더), policy-selftest `src/tripwires.rs`(⑦ root registry·간선)·`src/suite.rs` | JSON, L2. 손대지 않음(요청서 §1) |
| 노드 manifest | `output/<topology>/manifest.yaml`, `output/<topology>/sub_manifest.yaml` | terraforming(인터뷰·스캔), `render_sub_env`(서브용) | role-contract `src/load.rs:70`·`src/topology.rs:874`, terraforming `src/staleness_gate.rs:825`, hint-publisher `src/evidence/e1.rs:792`, upstream `src/manifest.rs:152` | YAML(1.1 → 1.2, `yaml_inputs.tsv`), L2 |
| 캠페인 상태 | `campaigns/<id>/campaign.yaml`, `cells/<cell>/{config.yaml,lockset.json,cell.status.json}`, `campaigns/<id>/relay/<ctx>.json`, `relay/pending_hitl.json` | `campaign_init.sh`(유일한 writer, workflow.md §캠페인), `relay.sh`(릴레이 원장) | adversarial-benchmark `src/campaign_write.rs:5019`·`src/campaign_validator.rs:650`, recipe `--config`, `relay.sh --continue` | YAML·JSON, L2. `campaigns/_template/**`는 추적 뼈대 |
| 벤치 인증서 | `docs/benchmark/benchmark_<…>.yaml` | `publish_benchmark_record.sh`·`lite_bench.sh --publish-report` | policy-gate `src/completion_gate.rs:499`(flat 파서), policy-publisher `src/gate.rs:102`, hint-publisher `src/evidence/e1.rs:811`, tripwire `policy-selftest/src/tripwires.rs:683` | **flat `key: value` 문자열**, L1(키·값 문자열). YAML 로더가 읽지 않는다 |
| 벤치 리포트·스윕 지도 | `docs/benchmark/bench_report_<…>.md`, `sweep_map_<…>.{md,json}` | adversarial-benchmark `render_report`·`render_sweep_map` | `evidence_publisher publish-benchmark`(측정 구성 표 파싱, docs.md §publisher 계약), hint-publisher lite 자격 | Markdown 표(측정 구성 표 행 L1), JSON L2 |
| 발행 기록·work manifest | evidence_publisher 의 publication record·work manifest | `evidence_publisher.sh init|append-raw|…|finalize` | `completion_gate.sh verify`·`authorize` | JSON, L2(키·reason_codes L1) |
| hint 태그·카탈로그 | `hint/*` 태그 페이로드, `hints/` 카탈로그·`hints/.drafts/` | `hint.sh publish|continue` | `hint.sh match|verify`(hint-publisher) | 페이로드 바이트(봉인)·태그 이름 L1 |
| 블랙박스 데이터 | `docs/logs/<node_id>/{envelope.json,campaign_brief.json,…}` | terraforming `regen_envelope`·`logs_lifecycle`·`blackbox_*` | 에이전트 폴링(envelope), 메인의 서브 관측(campaign_brief 회수 미러) | JSON, L2 |
| simlog·testlog | `docs/simlog/<run>/…`, `docs/testlog/…` | recipe·upstream·벤치 | 사람·에이전트, `completion_gate`(필수 증거 경로) | 원시 vault(편집 금지), 경로·이름은 L1 |
| 서브 회수 미러 | `sync_staging/sub_docs/…` | `fetch_sub_docs.sh` | 사람·에이전트, campaign_brief 관측 | 서브 문서의 사본 |

## A2A 와이어 형식 (메인 ↔ 서브)

정본 절차는 `.claude/skills/terraforming_node/SKILL.md` §2.7과 `references/orchestration/a2a_relay.md`다. 이 표는 이전 때 바이트·값이 지켜져야 하는 자리만 적는다.

| 통로 | 실행자 → 상대 | 실려 가는 것 | 고정 수준 |
|---|---|---|---|
| 턴제 릴레이 `relay.sh` | 메인 → 서브 하네스 CLI(ssh) | 과업 텍스트·예산 인자, 응답 스트림 안의 리포트 JSON(`parse_report`, a2a_relay.md:127), 원장 `campaigns/<id>/relay/<ctx>.json` | 리포트 JSON 키·`reason_codes` L1·L2 |
| 서브 진행 요약 | 서브 → 메인(문서 회수) | `docs/logs/<node_id>/campaign_brief.json`(a2a_relay.md:33) | 키 L2 |
| 오버레이·빌드킷 배달 | `sync_to_sub.sh`, `render_sub_env` | 헌법·스킬·블랙박스 오버레이, 서브 manifest, `[sync]` 커밋, `Agent_Card.json`(A2A 1.0.1, unsigned) | 파일 바이트(rsync), 서브 manifest YAML L2, 카드 JSON L2 |
| 문서 회수 | `fetch_sub_docs.sh` | 서브 `docs/` → `sync_staging/sub_docs/` | 경로 L1 |
| native multinode 고정 러너 | 메인 `native_multinode_serve` → 서브 `native_remote`(ssh) | argv 프로토콜, 메인이 서브 실행 루트에 둔 ELF(`native_wheelhouse`) | argv·종료 코드 L1, 출력 L2 |
| 서고 접근자 | 서브 wiki-desk → 메인 서고(ssh 강제명령 게이트웨이, `library_access.sh`) | 질의·반출 결과 | 요청·응답 형식 L1 |
| 파생 선언 | `campaign_init --emit-slice <node>` → 릴레이 stdin | 서브용 캠페인 조각 | 키 L2 |

## 메인/서브 버전 혼재 정책

단계적 배달(w-* → n-*) 동안 메인과 서브의 하네스 판이 다를 수 있다. 서브는 자기 최초 온라인 빌드로 ELF를 만들고(D2·D9), 메인은 서브 ELF를 밀어 넣지 않는다
(native_remote·native_wheelhouse처럼 메인이 서브 실행 루트에 ELF를 직접 두는 통로는 예외이며, 그 경우 판이 같다).

1. 위 A2A 표의 형식은 캠페인 동안 **추가만** 한다. 키·플래그·종료 코드의 삭제·개명·의미 변경은 `fixed-surface` divergence이고, 메인·서브를 같은 배달과 그 뒤 노드 정비(n-*)에서
   함께 바꾼 다음에만 쓴다.
2. 읽는 쪽은 모르는 키를 무시하고, 새 키가 없으면 옛 의미로 읽는다. 이전한 크레이트의 serde 구조체는 `deny_unknown_fields`를 A2A·영속 데이터 읽기에 쓰지 않는다.
3. 메인은 n-* 회수로 서브의 `git log -1`과 빌드 로그(`exit=0`)를 확인하기 전에는 서브가 새 형식을 말한다고 가정하지 않는다.
4. YAML 1.1 → 1.2 전환(c30·c22) 뒤에도 서브 데이터 이전(wd-yaml) 전까지 서브 쪽 로더는 옛 판일 수 있다. 같은 파일을 두 판이 읽어도 같은 값이 되도록 이전 모드는
   의미가 같은 표기(`true`/`false`)로만 바꾼다(`yaml_compat compare3`의 구 파서 = 새 파서 조건).
5. 버전 혼재 중 실패는 노드 정비 정지선(plan 「정지선」 "서브 온라인 빌드를 워커가 회수하지 못한다")으로 사람에게 넘긴다.

## 세션 도구와 남은 Python 기동 판정 (D16·D17 ④)

- **블랙박스 세션 도구는 이미 Rust다.** `upstream-version-watch/rust/runtime/src/native_multinode.rs:1842` `session_interpreter()`는 늘 `bash`를 돌려주고,
  도구는 Rust 런처 `.claude/skills/terraforming_node/scripts/node_blackbox/blackbox_session.sh`(bin `blackbox_session`, 서브는 `render_sub_env`가 배달한
  `.claude/runtime/node_blackbox/`)다. 옛 실행 상태가 옛 이름을 적었으면 `session_launcher()`(같은 파일 :1847)가 형제 `.sh`로 바꾼다.
  추적 `.rs`·`.sh`에서 세션 도구가 Python을 띄우는 자리는 없다(c10 `.rs` 모드 허용목록에 세션 도구 행이 없다). 이식·컨테이너 폴백 판단이 필요 없다.
- **native multinode `pip freeze`(rs 허용목록 행, 대체 cell c43)는 하네스 기동이다(최종 0 대상).** `native_multinode/up.rs:720`이 `wheelhouse verify` 뒤
  서빙 venv의 `<root>/venv/bin/python -m pip freeze --all`(`native_multinode.rs:1116`)을 메인은 직접, 서브는 고정 러너로 돌려 설치 집합을 simlog에 남긴다.
  서빙 자체가 아니라 하네스의 기록 단계이고, 호스트 프로세스 트리의 python execve로 센다(D17 ②). 대체: c43이 venv의 `site-packages/*.dist-info`(METADATA의
  Name·Version, URL 설치는 `direct_url.json`)를 Rust로 읽어 같은 목록을 만든다. 서브 쪽은 `native_remote` 하위 명령으로 둔다. 허용목록 행의 대체 cell(c43)은 그대로다.
- **사람 결정이 필요한 충돌(c43 전에 정할 것):** native multinode는 vLLM을 호스트 venv 환경에서 띄운다(`native_multinode.rs:1234` `serve_argv` = 트리플렛 `.sh` 러너, `:1200` `venv/bin/ray start`).
  그 venv는 `wheelhouse verify --venv <root>/venv`(`native_multinode.rs:1105`, `up.rs:712`)가 만든다. D16은 wheelhouse selftest·verify를 vLLM 이미지 `docker run`(container 클래스)으로
  옮기라고 정했지만, native 모드에서는 verify가 만든 venv가 곧 호스트 서빙 venv라서 컨테이너로 옮기면 native 서빙이 설 자리가 없다. 선택지:
  (a) native 모드의 venv 생성·설치는 런타임 환경 준비(범위 밖)로 보고 container 이전 대상에서 뺀다, (b) native 모드를 퇴역한다, (c) verify(검사)만 컨테이너로 옮기고
  설치는 별도 런타임 환경 단계로 둔다.
- **E2E strace 계수 경계(D17 ②):** native 서빙의 vLLM·Ray 프로세스는 런타임 환경 Python이다. 트리플렛 러너·`venv/bin/ray`의 execve는 셸·셔뱅 스크립트라 이름 규칙
  (`^python[0-9.]*$`)에 걸리지 않지만, Ray 워커는 `sys.executable`(`venv/bin/python3`)로 뜨므로 `strace -f`가 하네스 트리 안에서 센다. 라이브 E2E(e-*)의 계수 범위에서
  "런타임 환경 경계(컨테이너 안, native 서빙 venv의 vllm·ray 자손)"를 빼는 규칙을 e-* 업무지시와 w_verify 판정에 적어야 한다(오케스트레이터 결정).
- **parity `--self-check` 'strace-sees-python'(rs 허용목록 c60 행):** strace 계수기가 실제 python 기동을 세는지 보려고 일부러 `python3 -c pass`를 띄운다.
  `pyexec0`는 이름 규칙으로 세므로 같은 이름의 대역(`python3`라는 이름의 다른 프로그램)도 센다. c60은 이 자기검사를 pyexec 범위 밖으로 빼거나
  계수기 규칙을 바꾸는 쪽을 정해야 한다(`entrypoints.tsv` `parity_self_check` 행).
