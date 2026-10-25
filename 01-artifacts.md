# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_patch_pre` | 안 실림 | — | 1-signal | build-ledger:docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json | 적용 판정 없음 — 원장 관측(기계 파생) |
| `build_recipe` | 실림 | `artifacts/build_recipe/Dockerfile.source-build` · `artifacts/build_recipe/requirements.txt` | 3-signal | build-ledger:build-ledger.dockerfile | 이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(S1 슬롯 정정 · `plan_26102420_…` 정정 기록 "이미지 재빌드가 필요 없어"). 측정 이미지의 Created 는 2026-10-14T01:45:19Z 다(1.1 경고). 이 이미지를 지은 정확한 렌더 개정은 이 계보 안 문서에 적혀 있지 않다(미확인). - Dockerfile(`generated-unverified`): 측정 이미지 docker history RUN · COPY 와 16/17 이 일치한다. 불일치 한 단계는 실린 파일의 가드 echo(`RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0) …"` · L40)… |
| `compose` | 실림 | `artifacts/compose/.env.template` · `artifacts/compose/arm_patch.sh` · `artifacts/compose/docker-compose.yaml` | 2-signal | file:output/single/docker-compose.yaml | single 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 둘이다. - ① `${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro` 볼륨(L24). 측정 직전 커밋 20c7690 이 넣은 줄이다(W6 · 1.1 실린 리비전). 프로젝트 env 형상 `.env.template` 의 `PLE_MMAP_HOST_PATH=<manifest.ple_mmap_host_path>` 가 그 값을 준다. 셀 env 는 compose `env_file`(`envs/.env.${CONFIG_FILE}`)로 컨테이너에 도달한다. - ② 러너 경로 `command: ["bash", "/app/configs/${CONFIG_FILE}.sh"]` 와 `./configs:/app/configs:ro`… |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 셀 env 에 VARIANT 줄 없음 = stock(workflow.md §변종 좌표의 거처)(기계 파생) |
| `runtime_patch` | 실림 | `artifacts/runtime_patch/q38fn31-nv4-mmap-1m-tp1_patch.py` | 3-signal | none:output/single/configs/q38fn31-nv4-mmap-1m-tp1_patch.py | (a) **무엇을 고치나** — Qwen4Exp PLE n-gram 테이블을 디스크에서 mmap 으로 읽는 out-of-tree vLLM 플러그인 `qwen_ple_mmap_plugin` 0.1.0 의 번들이다. 파일은 두 부분이다. stdlib 만 쓰는 짧은 로더(머리 주석 5줄 · `_arm()`)와, zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/qwen_ple_mmap_plugin_dist_957f66252c` 에 풀어 `sys.path` 에 얹는다. 그러면 vLLM 이 `load_general_plugins()` 에서 entry point `qwen_ple_mmap = qwen_ple_mmap_plugin:register` 를 부른다. `register(… |
| `triplet` | 실림 | `artifacts/triplet/.env.q38fn31-nv4-mmap-1m-tp1` · `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.sh` · `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.yaml` | 3-signal | file:output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml | 서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 줄은 둘로 나뉜다. - 셀 env 의 플러그인 활성 줄: `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4` · `VLLM_PLE_MMAP_VERIFY=16`(+ `RUNTIME_PATCH_STEM` — 위 arm_patch 참고). VERIFY 는 디버그 대조 노브이고 처음 16회 조회에서만 추가 수집을 한다. 엔진은 이 셋을 "Unknown vLLM environment variable" 로 경고한다(lite_engine L44~L47) — 플러그인이 읽는 env 라 정상이다. - yaml 의 `hf-overrides`: `max_position_embeddings` 를 함께… |

**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)

- ⚠ `build_recipe` `output/single/Dockerfile.source-build` — 실린 바이트(worktree)가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다(16/17) · 이미지 빌드 뒤 바뀐 레시피일 수 있다 — 마지막 커밋이 이미지 Created 뒤(이미지 Created 2026-10-14T01:45:19Z · 마지막 커밋 2026-10-14T03:07:26Z)
- `build_recipe` `output/single/Dockerfile.source-build` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)` · docker history 대조 16/17 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 아니오
- `build_recipe` `output/single/requirements.txt` 실린 리비전: 리비전 미특정 · 선택 방법 `worktree(빌드 원장 sha256 일치 — 쓰인 바이트 관측 확인 = 예)` · docker history 대조 없음 · 실린 바이트 `worktree` · 쓰인 바이트임을 관측으로 확인 예
- `compose` `.claude/skills/upstream-version-watch/assets/configs/arm_patch.sh` 실린 리비전: 커밋 `44d1f5b05e16` · 선택 방법 `worktree(git log -1 --before=2026-10-25T01:22:24Z -- .claude/skills/upstream-version-watch/assets/configs/arm_patch.sh → 44d1f5b05e16 · 작업트리 mtime 2026-10-25T14:07:01Z > 측정(sweep_index.generated_utc)이지만 바이트가 그 개정과 같다 · 측정 뒤 커밋 0 · 작업트리 = HEAD)` · docker history 대조 없음 · 실린 바이트 `worktree` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오
- `compose` `output/single/docker-compose.yaml` 실린 리비전: 커밋 `20c7690b560e` · 선택 방법 `git log -1 --before=2026-10-25T01:22:24Z -- output/single/docker-compose.yaml → 20c7690b560e · 작업트리 mtime 2026-10-25T14:10:42Z > 측정 2026-10-25T01:22:24Z(sweep_index.generated_utc) · 측정 뒤 커밋 1(2199257f7f71) · 작업트리 = HEAD — 측정 전 마지막 커밋의 바이트를 싣는다(git show · 측정 당시 미커밋 수정이 있었는지는 관측 불가)` · docker history 대조 없음 · 실린 바이트 `git:20c7690b560e38d326fb4b7bf853af990945dda0` · 근거 `git-rev-before-measurement` · 쓰인 바이트임을 관측으로 확인 아니오 · 측정 뒤 커밋 1건

**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)

- `build_patch_post` known-non-patch 1건: `.gitkeep`
- `build_patch_pre` known-non-patch 1건: `.gitkeep`

**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · 관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)

| 슬롯 | 파일 | 검증 | 검증 근거 | 관련성 | 관련성 근거 |
|---|---|---|---|---|---|
| `build_recipe` | `artifacts/build_recipe/Dockerfile.source-build` | `generated-unverified` · renderer | 실린 Dockerfile.source-build(worktree)이 측정 이미지 docker history RUN·COPY 와 16/17 만 일치 — 이 이미지를 지은 바이트임을 관측으로 확인하지 못했다(_select_dockerfile · worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 16/17 · 전 이력(output/*/Dockerfile.source-build · 모든 ref)에도 전수 일치 개정 없음 — recipe_vs_image 경고 유지)) | `unknown` | 이 셀은 이 경로로 실행되지 않았다(generated-unverified · 생성 renderer) — 실행 기록이 없어 필요 여부를 판정하지 않는다(참고 형상) |
| `build_recipe` | `artifacts/build_recipe/requirements.txt` | `verified` · cell-run | 쓰인 레시피(build-ledger.dockerfile) — 개정 선택 slots.build_recipe.evidence.selected_revisions | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/.env.template` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/arm_patch.sh` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `compose` | `artifacts/compose/docker-compose.yaml` | `verified` · cell-run | 셀 서빙 경로 입력(측정 당시 개정 · slots.compose.evidence.selected_revisions · env 형상은 값 치환 사본) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `runtime_patch` | `artifacts/runtime_patch/q38fn31-nv4-mmap-1m-tp1_patch.py` | `verified` · cell-run | 셀 configs 의 런타임 패치(arming 관측 — 측정 기동 로그 output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:34 · output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:34 (`[arm_patch] runtime patch armed → /app/configs/q38fn31-nv4-mmap-1m-tp1_patch.py` = .pth 무장 · 패치 효과 자체는 별도 관측)) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/.env.q38fn31-nv4-mmap-1m-tp1` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.sh` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
| `triplet` | `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.yaml` | `verified` · cell-run | 셀 트리플렛 — 측정이 이 셀 config 로 돌았다(스윕 색인) | `required` | 재현 경로 입력(패치 아님 — 관련성 판정은 빌드 패치만 · 이 셀의 빌드·기동 경로가 읽는 파일) |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
**적용 집합** — 상태 `observed` · 출처 `docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json`

| 단계 | 파일 | 결과 | 결과 출처 | 사유 | 실림 | model-trigger |
|---|---|---|---|---|---|---|
| inline | `strip-hoist` | skipped | build-ledger | register_opaque_type accepts hoist / opaque_object 부재 | 안 실림 | — |

**결과 출처 범례**

- `build-ledger` — 이미지 안 빌드 원장이 적용 결과를 기록했다(관측)

**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)

| 파일 | 스크립트 바이트 정체 | 이미지 사본 sha256 | 적용 표지(상태 찾음/선언) | 정적 규칙 | 원장 분류 |
|---|---|---|---|---|---|
| `strip-hoist` | — | — | — | — | status-file |

**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)

| 순위 | 결과 | 대상 |
|---|---|---|
| attestation | absent | output/single/benchlog/attestation_q38fn31-nv4-mmap-1m-tp1.json(부재) |
| local-image | observed | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 |
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

### runtime_patch — `q38fn31-nv4-mmap-1m-tp1_patch.py`(이 셀의 핵심 · +1 런타임 패치 슬롯 · owner `vllm-recipe-explorer`)

(a) **무엇을 고치나** — Qwen4Exp PLE n-gram 테이블을 디스크에서 mmap 으로 읽는 out-of-tree vLLM 플러그인 `qwen_ple_mmap_plugin` 0.1.0 의 번들이다. 파일은 두 부분이다. stdlib 만 쓰는 짧은 로더(머리 주석 5줄 · `_arm()`)와, zlib+base64 로 압축한 tar 한 덩어리(`_B`)다. 로더는 번들을 `/tmp/qwen_ple_mmap_plugin_dist_957f66252c` 에 풀어 `sys.path` 에 얹는다. 그러면 vLLM 이 `load_general_plugins()` 에서 entry point `qwen_ple_mmap = qwen_ple_mmap_plugin:register` 를 부른다. `register()` 는 `VLLM_PLE_MMAP` 이 1 · true · yes 일 때만 `backend.install()` 을 부르고, `install()` 은 `vllm.models.qwen4_exp.nvidia.ngram_embedding` 모듈의 전역 `Qwen4ExpPLEDeviceEmbedding` · `Qwen4ExpPLEPinnedHostEmbedding` 을 둘 다 `Qwen4ExpPLEMmapEmbedding` 으로 바꾼다(그래서 `engram-config.cpu_offload` 와 무관해진다 · 번들 주석). 새 클래스가 하는 일:
- 0행 placeholder 가중치를 둔다(`allocate_embedding_weight`).
- 체크포인트 샤드 적재(`checkpoint_start` 가 있는 `weight_loader` 호출)를 no-op 로 버린다.
- forward 에서 이 rank 의 ETP 어휘 구간 행만 `MmapRowStore`(`table.py` · 스테이징 디렉터리의 `*.safetensors` 헤더에서 샤드별 파일 · 오프셋을 찾는다)로 모아 장치로 올린다.
- TP>1 이면 FP8 행을 int8 뷰로 all-reduce 한다. **이 셀(TP=1)에서는** `_reduce` 가 `tp_size == 1` 이면 그대로 돌려주므로 all-reduce 를 타지 않고, ETP 구간도 어휘 전체 `[0,320001536)` 이다(lite_engine L93).

이식 출처는 업스트림 PR 이 아니다. 머리 주석은 provenance 를 `hand-written(자체 저작 · 참조 설계: blazux … 코드 비복사)` 라 적는다. 설계는 S1 판정 문서 §4 이고, 이전 vLLM 빌드 패치 62 의 `v0.31.0` 재저작이다(W5). 저작 때 이 파일의 6번째 줄 이하(로더 · 번들)를 멀티 FP8 셀 `q38fn31-fp8-1m-tp2_patch.py` 와 `diff` 하니 같았다 — 머리 주석(셀 이름)만 다르고 플러그인은 한 판이다.

> [원문] q38fn31-nv4-mmap-1m-tp1_patch.py §L1-5
> # q38fn31-nv4-mmap-1m-tp1 런타임 패치 — Qwen4Exp PLE n-gram 테이블 mmap 백엔드(vLLM 0.31.x out-of-tree 플러그인 번들)
> # 목적: GB10 은 호스트 RAM = GPU 메모리라 stock Device/PinnedHost 임베딩이 47.7GiB 테이블을 전부 재료화한다 → 디스크(mmap)에서 필요한 행만 읽는 백엔드로 교체. 62-qwen4exp-ple-mmap(0.29) 의 0.31 재저작.
> # 활성: 셀 env VLLM_PLE_MMAP=1 일 때만(미설정 = stock 동일). 스테이징 경로 VLLM_PLE_MMAP_DIR. provenance=hand-written(자체 저작 · 참조 설계: blazux qwen3.8-Flash-DGX ple mmap, 코드 비복사) · 미검증 → 서빙 스모크가 유일한 중재자.
> # site-init safe: stdlib 만 사용, vllm/torch import ✗.
> # 동작: 번들을 /tmp 에 풀고 sys.path 에 얹는다 → vLLM 이 load_general_plugins() 로 entry point(qwen_ple_mmap)를 발견·register().

**번들에서 읽을 수 있는 소스 트리 꺼내기.** 패치 파일을 **실행하지 않고** `_B` 문자열만 AST 로 읽어 푼다. 저작 때 Python 3.12.3 으로, 빈 임시 디렉터리에 패치 사본을 `artifacts/runtime_patch/` 자리에 두고 아래 명령을 그대로 실행해 파일 목록을 확인했다(rc 0). `filter="data"` 는 Python 3.12 또는 3.11.4 이상에서 쓸 수 있다.

```sh
mkdir -p ./ple_mmap_plugin_src && python3 -I - artifacts/runtime_patch/q38fn31-nv4-mmap-1m-tp1_patch.py ./ple_mmap_plugin_src <<'EOF_PY'
import ast, base64, io, sys, tarfile, zlib
src, dst = sys.argv[1], sys.argv[2]
tree = ast.parse(open(src, encoding="utf-8").read())
blob = next(n.value.value for n in tree.body
            if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == "_B")
tf = tarfile.open(fileobj=io.BytesIO(zlib.decompress(base64.b64decode(blob))))
tf.extractall(dst, filter="data")
print("\n".join(m.name for m in tf.getmembers()))
EOF_PY
```

출력(6항목 · 디렉터리 1 · 메타 2 · Python 3): `qwen_ple_mmap_plugin-0.1.0.dist-info/{METADATA, entry_points.txt}` · `qwen_ple_mmap_plugin/{__init__.py, backend.py, table.py}`(줄 수 20 · 196 · 132). 이 zip 의 패치 사본과 원래 거처 `output/single/configs/q38fn31-nv4-mmap-1m-tp1_patch.py` 는 저작 때 `cmp` 로 바이트가 같았다. 원래 거처는 비추적이라 휘발한다.

(b) **왜 필요했나** — 빼면 W3 이 그대로 다시 나온다. stock 임베딩이 PLE 를 재료화해 단일 노드 상주가 체크포인트 전량이 되고, 예산 산술이 불가가 된다. 패치 파일만 두고 `VLLM_PLE_MMAP` 을 켜지 않아도 같다(`register()` 가 아무것도 하지 않는다).

(c) **이 셀에서 발화했나** — 예. 무장과 활성을 측정 기동 엔진 로그가 따로 보여 준다.
- 무장: `[arm_patch] runtime patch armed → /app/configs/q38fn31-nv4-mmap-1m-tp1_patch.py` 한 줄(lite_engine L34 · 01 §1.1 판정 근거와 같은 줄).
- 활성: EngineCore 의 `PLE mmap backend installed (Qwen4ExpPLEMmapEmbedding replaces device/pinned embeddings)`(L75).
- 테이블: `128 shards in 1 files, 320001536 rows x 160 B (47.7 GiB on disk, dir=/app/ple_mmap/q38fn-nvfp4)`(L93).
- 정확도 대조: 셀 env `VLLM_PLE_MMAP_VERIFY=16` 이 처음 16회 조회마다 마스크 없는 독립 수집과 결과를 바이트로 대조한다(번들 `_verify`). 측정 기동 로그에 `verify OK` 가 L128 · L141(32768행) · L198(9056행) · L199~L211(16행 13줄)로 16줄 있고 FAILED 는 0줄이다. TP=1 에서는 마스크가 어휘 전체이고 reduce 가 항등이라, 이 대조는 행 저장소가 같은 행을 두 번 같게 읽는지를 보는 자기 일관성 검사에 가깝다 — 상주 경로와의 비트 동일 확증이 아니다.

(d) **언제 불필요해지나** — 대상 vLLM 이 디스크 기반(재료화하지 않는) PLE 백엔드를 정식 제공하거나, 노드 메모리가 체크포인트 전량 상주를 덮을 때다(02 Q6 · R1). 종료 경로는 모두 **fail-loud** 다(번들 `backend.py` · `table.py`).
- eager 가 아니면 생성자에서 `RuntimeError`.
- `VLLM_PLE_MMAP_DIR` 이 디렉터리가 아니거나, 샤드 텐서가 없거나, 샤드가 128개에 못 미치거나, 샤드 폭 · dtype 이 임베딩과 다르면 기동이 멈춘다.
- 조용히 stock 으로 떨어지는 경로는 없다. `VLLM_PLE_MMAP` 이 없으면 아무것도 하지 않는 것이 설계다.

무장 자체의 경로는 `arm_patch.sh` 다. 패치 파일이 있을 때만 `.pth` 를 쓰고, 없으면 아무것도 하지 않는다(기동을 멈추지 않는다). single 판은 키를 `CONFIG_FILE` 하나로 찾는다(멀티 판의 `RUNTIME_PATCH_STEM` 우선 규칙은 이 개정에 없다 — 셀 env 의 `RUNTIME_PATCH_STEM` 줄은 이 통로에서 읽히지 않으며, 값이 `CONFIG_FILE` 과 같아 결과는 같다).

> [원문] arm_patch.sh §L15-22
> _cfg="${CONFIG_FILE:-default}"
> _patch="/app/configs/${_cfg}_patch.py"
> if [ -f "$_patch" ]; then
>     _sp="$(python3 -c 'import sysconfig;print(sysconfig.get_paths()["purelib"])' 2>/dev/null || true)"
>     if [ -n "$_sp" ] && [ -w "$_sp" ]; then
>         printf "import importlib.util as u;s=u.spec_from_file_location('__cfgpatch','%s');m=u.module_from_spec(s);s.loader.exec_module(m)\n" \
>             "$_patch" > "${_sp}/zzz_${_cfg}_patch.pth"
>         echo "[arm_patch] runtime patch armed → ${_patch} (purelib ${_sp})"

### compose — `docker-compose.yaml` · `arm_patch.sh` · `.env.template`

single 공통 서빙 경로이며 이 모델 전용이 아니다. 이 셀에 걸린 것은 둘이다.
- ① `${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro` 볼륨(L24). 측정 직전 커밋 20c7690 이 넣은 줄이다(W6 · 1.1 실린 리비전). 프로젝트 env 형상 `.env.template` 의 `PLE_MMAP_HOST_PATH=<manifest.ple_mmap_host_path>` 가 그 값을 준다. 셀 env 는 compose `env_file`(`envs/.env.${CONFIG_FILE}`)로 컨테이너에 도달한다.
- ② 러너 경로 `command: ["bash", "/app/configs/${CONFIG_FILE}.sh"]` 와 `./configs:/app/configs:ro` 볼륨 — 트리플렛 러너가 `arm_patch.sh` 를 source 한다.

측정 뒤 커밋 1건(2199257 — 렌더 compose 의 PLE mmap 주석에서 운영자 절대경로 리터럴 제거 · 커밋 메시지)이 있으나 실린 바이트는 측정 전 개정 20c7690 이다(1.1). compose 에는 `oom_score_adj: 800` 이 있다 — 통합메모리 압박 때 커널이 vLLM 컨테이너를 먼저 잡게 하는 층이다(compose 주석). 폐기 조건: PLE mmap 셀이 아니면 볼륨은 기본값으로 무해하다(compose 주석).

### triplet — `q38fn31-nv4-mmap-1m-tp1.yaml` · `.sh` · `.env.q38fn31-nv4-mmap-1m-tp1`

서빙 노브(1.3) · 러너 · 셀 env 다. 이 셀 고유의 줄은 둘로 나뉜다.
- 셀 env 의 플러그인 활성 줄: `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4` · `VLLM_PLE_MMAP_VERIFY=16`(+ `RUNTIME_PATCH_STEM` — 위 arm_patch 참고). VERIFY 는 디버그 대조 노브이고 처음 16회 조회에서만 추가 수집을 한다. 엔진은 이 셋을 "Unknown vLLM environment variable" 로 경고한다(lite_engine L44~L47) — 플러그인이 읽는 env 라 정상이다.
- yaml 의 `hf-overrides`: `max_position_embeddings` 를 함께 실은 YaRN factor 4 다(W2 · W4).

yaml 머리 주석 중 둘은 이 셀과 맞지 않는다. 2번째 줄의 "동치 시험 셀(상주 PLE 셀 q38fn31-nv4-1m-tp2 와 대조)" 은 이 TP=1 셀이 아니라 멀티 쪽 대조 셀을 묘사하는 문구로 보인다(이 셀에서 상주 셀과의 동치 대조를 한 기록은 계보 안에 없다 · 추론). 4번째 줄 "참조(승계 아님): camp-26090918 nv4-bf-1m-res … memwatch TRIP" 은 캠페인 id 가 계보 판정 문서와 다르다 — 그 TRIP 은 22셀 재수행 캠페인(`camp-26091110-qwen38fn-multi-r2`)의 셀 #16 으로 기록돼 있다(`testlog_26091121_…` 비-measured 표 · 멀티 TP=2 · 이전 vLLM). yaml 의 `async-scheduling: false` 는 엔진에 닿지 않았고 측정은 async on 상태였다(M4 · 소스 판독 + 로그 부재). 러너 `.sh` 는 `arm_patch.sh` 를 한 번 더 source 한다(멱등 · arm_patch 머리 주석).

> [원문] q38fn31-nv4-mmap-1m-tp1.yaml §L1-4
> # 셀 q38fn31-nv4-mmap-1m-tp1 — plan_26102420 S2 · 싱글 TP=1 · Qwen3.8-Flash-Next NVFP4 · 1M YaRN · PLE=mmap(런타임 플러그인 qwen_ple_mmap)
> # 선언 축: vLLM 0.31.0 stock · NVFP4 · kv=fp8_e4m3 · ctx=1048576(YaRN f4 · rope_scaling_translate 병합 산출) · PLE=mmap(런타임 플러그인 qwen_ple_mmap · 디스크 스테이징) · 동치 시험 셀(상주 PLE 셀 q38fn31-nv4-1m-tp2 와 대조)
> # ⚠ 0.31.0 의미 변경: YaRN 은 max_position_embeddings 가 이미 확장값이라고 가정(factor 재적용 ✗, config/model.py _get_and_verify_max_len) → 오버라이드에 max_position_embeddings=1048576 을 함께 싣는다(0.29 번역기 산출은 이 키가 없다)
> # 참조(승계 아님): camp-26090918 nv4-bf-1m-res 는 0.29.0rc6 에서 memwatch TRIP(MemAvailable<10GiB · KV 20GiB) — 이 셀은 KV 를 줄여 재확정한다

### build_recipe — `Dockerfile.source-build` · `requirements.txt`(공유 이미지 · 이 모델의 요구 아님)

이미지 `easy-vllm:0.31.0-cu133-aarch64-source` 는 이 셀을 위해 다시 짓지 않았다(S1 슬롯 정정 · `plan_26102420_…` 정정 기록 "이미지 재빌드가 필요 없어"). 측정 이미지의 Created 는 2026-10-14T01:45:19Z 다(1.1 경고). 이 이미지를 지은 정확한 렌더 개정은 이 계보 안 문서에 적혀 있지 않다(미확인).
- Dockerfile(`generated-unverified`): 측정 이미지 docker history RUN · COPY 와 16/17 이 일치한다. 불일치 한 단계는 실린 파일의 가드 echo(`RUN echo "[guard] source-build key (26.07-py3 x vLLM 0.31.0) …"` · L40)다 — 저작 때 `docker history --no-trunc` 에 `guard` 문자열이 0건이었다. 실린 바이트는 작업트리 판이고 마지막 커밋이 이미지 Created 뒤라(1.1), 빌드에 쓰인 바이트 그대로라고 확인되지 않는다. `ARG VLLM_REF` 기본값은 v0.31.0 이다(L46). `use_existing_torch.py` 는 `tools/` 먼저 · 루트 다음으로 찾고 둘 다 없으면 fail-loud 로 멈춘다(L82-84) — 이미지 history 에도 이 분기 단계가 있다.
- `requirements.txt`(`verified`): 빌드 원장 sha256 과 바이트가 일치한다 — 이 이미지 빌드가 COPY 한 의존 목록과 같다(머리 주석은 v0.31.0 wheel Requires-Dist 기준이라 적는다 · L1-3).

PLE mmap 은 이미지가 아니라 런타임 패치에 있다. 이 모델의 요구는 이미지 핀이 아니다.

### 싣지 않은 슬롯
- `build_patch_pre` · `build_patch_post` — 측정 이미지의 빌드 원장에 적용된 빌드 패치 파일이 없다. 원장이 기록한 것은 inline `strip-hoist` 하나이고 결과는 **skipped** 다(`register_opaque_type accepts hoist`). 원장 `context_inventory` 는 두 슬롯 모두 `.gitkeep` 만 기록한다 — 이미지를 지은 빌드 컨텍스트에 패치 파일이 없었다. 이전 vLLM 의 빌드 패치 60 · 62 · 64 는 이 이미지에 들어가지 않았다(W1 · W5). 싣지 않은 것은 이 셀에 무엇이 필요했는지에 대한 판정이 아니다.
- `fork_pin` — 셀 env 에 `VARIANT` 줄이 없다. stock 빌드다.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 10개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `gpu-memory-utilization` | 0.80 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-model-len` | 1048576 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | fp8_e4m3 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-memory-bytes` | 16642998272 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | 15.5GiB: TP1 은 KV 헤드가 샤딩되지 않는다 — 멀티 FP8 실측 7,625 B/token/GPU → TP1 15,250 B/token × 1,048,576 = 14.9GiB + 여유 | serving-yaml |
| `enforce-eager` | true | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `async-scheduling` | false | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-seqs` | 1 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `max-num-batched-tokens` | 2048 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `engram-config` | {"cpu_offload": false} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: gpu-memory-utilization
값: 0.80
지위: inherited
근거: 멀티 셀들과 같은 값 · 이 셀에서 바꿔 잰 기록 없음 · KV 절대 클램프가 있어 엔진이 프로파일링을 건너뛰고 gmu 를 KV 산정에 쓰지 않는다고 기록(lite_engine L129)
출처: [q38fn31-nv4-mmap-1m-tp1.yaml, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V2
kind: value-status
노브: max-model-len
값: 1048576
지위: inherited
근거: 셀의 목표 길이(사람이 정한 셀 축 · YaRN f4 로 연다) · 길이를 바꿔 잰 스윕 없음 · 엔진 Using max model len 1048576(lite_engine L54)
출처: [plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화 §단계, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V3
kind: value-status
노브: kv-cache-dtype
값: fp8_e4m3
지위: inherited
근거: 멀티 1M 셀들에서 승계(1M 은 fp8 KV 전제로 계획됨) · 이 셀에서 bf16 과 대조한 적 없음 · v0.31.0 stock 이 그대로 받음(lite_engine L61)
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스 (기본 24셀 + 레버 서브스윕), output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V4
kind: value-status
노브: kv-cache-memory-bytes
값: 16642998272
지위: inherited
근거: 손레버 — yaml 주석이 다른 셀(멀티 FP8) 실측 7,625 B/token/GPU 를 TP1 로 옮겨 계산 · 줄이거나 키워 본 시도 없음 · 엔진은 1,237,563 토큰(1M 요청 하나 기준 1.18x)을 잡음(lite_engine L130) · 음성대조 아님
출처: [q38fn31-nv4-mmap-1m-tp1.yaml, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V5
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: 플러그인 v1 제약 — 번들 backend.py 가 enforce_eager 아니면 생성자에서 RuntimeError(소스 · 이 셀에서 graph 기동 실패를 관측한 기록 없음 · 그래서 declared-requirement 로 올리지 않음)
출처: [q38fn31-nv4-mmap-1m-tp1_patch.py, testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §4. 재저작 설계 (S2 에서 구현 · 이 문서는 판정만)]
```

```hint-event
id: V6
kind: value-status
노브: async-scheduling
값: false
지위: inherited
근거: 이전 vLLM 의 MTP+async 회피 조치에서 승계 · 엔진 non-default args 에 실리지 않음 — 값은 기본값 None 이고 엔진 규칙상 async on 으로 돌았다(무효 줄 · M4 · 소스 판독 + 로그 부재)
출처: [testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §1, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V7
kind: value-status
노브: max-num-seqs
값: 1
지위: inherited
근거: 1M 요청 하나를 목표로 한 셀 선언 · 바꿔 잰 기록 없음 · 이 값 때문에 full 벤치 동시성 2·4·8·16 이 걸러져 레벨 1개(sweep_index bench_knobs.levels.filtered_out)
출처: [output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json, q38fn31-nv4-mmap-1m-tp1.yaml]
```

```hint-event
id: V8
kind: value-status
노브: max-num-batched-tokens
값: 2048
지위: inherited
근거: 손레버 — 근거 문서 없이 둔 값 · 스윕 없음 · 엔진 FlashInfer autotune 이 2048 토큰 버킷까지 돎(lite_engine L138)
출처: [q38fn31-nv4-mmap-1m-tp1.yaml, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V9
kind: value-status
노브: engram-config
값: '{"cpu_offload": false}'
지위: inherited
근거: PLE 상주 셀(Device 백엔드 선택)에서 승계 · VLLM_PLE_MMAP=1 이면 플러그인이 Device · PinnedHost 전역을 모두 교체해 이 값과 무관(번들 install 주석) · 엔진 Resolved Engram configuration cpu_offload=False(lite_engine L63)
출처: [testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정 §3. stock 0.31.0 의 두 백엔드가 GB10 에서 왜 상주를 못 줄이나, q38fn31-nv4-mmap-1m-tp1_patch.py, output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log]
```

```hint-event
id: V10
kind: value-status
노브: hf-overrides
값: '{"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}}'
지위: declared-requirement
근거: W4 — max_position_embeddings 없이 rope_parameters 만 두면 v0.31.0 이 1M 을 ValidationError 로 거부(같은 모델 · 같은 이미지의 멀티 NVFP4 셀에서 관측 · 이 셀에서는 관측 안 함) · W2 — rope 오버라이드 자체가 없으면 262144 초과 길이가 거부(이전 vLLM 관측)
출처: [testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정 §1. 기동과 교정, testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §R8 실서빙 재검증 — factor=4 완결]
```

복사하면 위험한 값은 셋이다. `kv-cache-memory-bytes`(V4)는 다른 셀의 토큰당 바이트에서 옮긴 손계산이라 네 엔진의 `GPU KV cache size` 로 다시 재야 한다. `max-num-seqs: 1`(V7)은 성능 곡선을 한 점으로 만든 원인이다 — 최적값이 아니라 1M 단일 요청 셀의 선언이다. `async-scheduling: false`(V6)는 엔진에 닿지 않은 무효 줄이다 — 측정은 async on 상태의 값이고, 끄려면 `--no-async-scheduling` 이 non-default args 에 실리는지 확인해야 한다(M4). yaml 노브 가운데 빼면 깨진다고 관측된 것은 `hf-overrides`(V10) 하나이고, 그것도 이 셀이 아니라 자매 셀의 관측이다. yaml 밖에서 빼면 형상이 무너지는 것은 셀 env 의 플러그인 활성 줄(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR`)이다 — 빼면 W3 의 상주 형상으로 돌아간다(1.2 runtime_patch (b)).

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | build | 빌드 OK · 이미지 실재 | 층 창 38분 19초(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님) | 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z (layer-window) | docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 · 우리 Dockerfile 층 24개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 시각 — 빌드 소요 아님) | reconstructed(build-ledger build_args + compose build) |
| 2 | serve | health 200 + 추론 1회(스모크 판정) | ≤ 10분 27초(상한) | 2026-10-25T00:56:37Z → 2026-10-25T01:07:04Z (upper) | docs/logs/main/events/2026-10.jsonl budget_declare(label serve-q38fn31-nv4-mmap-1m-tp1) → 첫 측정 시각(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_raw_q38fn31-nv4-mmap-1m-tp1.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작)) | derived(스모크 사용법) |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 15분 20초 | 2026-10-25T01:07:04Z → 2026-10-25T01:22:24Z (exact) | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_raw_q38fn31-nv4-mmap-1m-tp1.json measured_utc(호스트 date -u · lite_bench.sh 가 cold 레그 직전에 찍는 측정 시작) → sweep_index.generated_utc(호스트 UTC · 두 끝 같은 시계) | reconstructed(bench json) |

**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)

- `build` 이미지 층 CreatedAt(docker history): 2026-10-14T01:07:00Z → 2026-10-14T01:45:19Z · 층 24개 · 출처 `docker history --no-trunc --format '{{json .}}' sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034` — 우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. build**

```sh
# 실린 artifacts/build_recipe/Dockerfile.source-build(실린 개정: worktree — 이미지를 지은 바이트임을 관측으로 확인 못함(docker history RUN·COPY 16/17))를 output/single/Dockerfile.source-build 자리에 두고 컨텍스트에서 짓는다.
mkdir -p output/single/build_patches_src output/single/build_patches   # 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep']
docker build -f output/single/Dockerfile.source-build \
  --build-arg BUILD_JOBS=8 \
  --build-arg RAY_VERSION=2.48.0 \
  --build-arg SM12X_PORT=0 \
  --build-arg SRC_DEPS_AUTHORITY=0 \
  --build-arg TORCH_CUDA_ARCH=12.1a \
  --build-arg VLLM_PRETEND_VERSION= \
  --build-arg VLLM_REF=v0.31.0 \
  --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git \
  -t easy-vllm:0.31.0-cu133-aarch64-source \
  output/single
# 동등 경로(compose): docker compose -f output/single/docker-compose.yaml --env-file output/single/envs/.env.q38fn31-nv4-mmap-1m-tp1 build
```

**2. serve**

```sh
bash .claude/skills/upstream-version-watch/scripts/single_serve_up.sh q38fn31-nv4-mmap-1m-tp1
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_cold_q38fn31-nv4-mmap-1m-tp1.json (date 20261025-010731) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 566 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_warm_q38fn31-nv4-mmap-1m-tp1.json (date 20261025-010813) · ⚠ --random-input-len/--random-output-len = total ÷ completed 평균(chat 템플릿 포함 실측 토큰 · 측정 도구 원문 인자 미관측 — lite_bench.sh 원문 인자와 다를 수 있다)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 565 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)

| zip 경로 | 빌드 컨텍스트 자리 | 근거 |
|---|---|---|
| `artifacts/triplet/.env.q38fn31-nv4-mmap-1m-tp1` | `envs/.env.q38fn31-nv4-mmap-1m-tp1` | compose env_file envs/.env.${CONFIG_FILE}(셀 env) |
| `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.sh` | `configs/q38fn31-nv4-mmap-1m-tp1.sh` | compose volumes ./configs:/app/configs |
| `artifacts/triplet/q38fn31-nv4-mmap-1m-tp1.yaml` | `configs/q38fn31-nv4-mmap-1m-tp1.yaml` | compose volumes ./configs:/app/configs |
| `artifacts/runtime_patch/q38fn31-nv4-mmap-1m-tp1_patch.py` | `configs/q38fn31-nv4-mmap-1m-tp1_patch.py` | compose volumes ./configs:/app/configs |
| `artifacts/build_recipe/Dockerfile.source-build` | `Dockerfile.source-build` | compose build.dockerfile Dockerfile.source-build(리터럴) · build.context . |
| `artifacts/build_recipe/requirements.txt` | `requirements.txt` | Dockerfile.source-build COPY requirements.txt |
| `artifacts/compose/.env.template` | `.env` | compose 변수치환 프로젝트 .env(${…} 참조) |
| `artifacts/compose/arm_patch.sh` | `configs/arm_patch.sh` | compose volumes ./configs:/app/configs |
| `artifacts/compose/docker-compose.yaml` | `docker-compose.yaml` | compose 파일(빌드 컨텍스트 루트) |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches_src/` | Dockerfile.source-build COPY build_patches_src/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_pre = ['.gitkeep'] |
| (실린 파일 없음 — 빈 디렉터리 생성) | `build_patches/` | Dockerfile.source-build COPY build_patches/ · 실린 패치 없음 — 빈 디렉터리를 만든다(없으면 COPY 단계에서 빌드가 멈춘다) · 빌드 원장 context_inventory.build_patch_post = ['.gitkeep'] |

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/level_01/post_health_q38fn31-nv4-mmap-1m-tp1.json` · `output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/level_01/guidellm_q38fn31-nv4-mmap-1m-tp1.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 34행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-10-24T14:07:31Z | main | `budget_declare` | `serve-q38fn31-nv4-mmap-1m-tp1` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=12599 · weights_mib=77705 · kv_mib=15872 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 1/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-24T14:33:19Z(선언 후 25m48s) · measured_utc 2026-10-25T01:22:24Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:565 |
| 2 | 2026-10-24T14:07:31Z | main | `serve_start` | — | serve_start · model=qwen3.8-flash-next · tensor_parallel_size=1 · topology=single · session=serve-q38fn31-nv4-mmap-1m-tp1-20261024T140731Z | docs/logs/main/events/2026-10.jsonl:566 |
| 3 | 2026-10-24T14:07:32Z | main | `budget_honored` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_honored · floor_mib=12599 · arm_ceiling_mib=9527 · remaining_s=7199 · 창 시작 2026-10-24T14:07:32Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:787 |
| 4 | 2026-10-24T14:07:33Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=7198 · 창 시작 2026-10-24T14:07:31Z 후 0m02s | docs/logs/main/events/2026-10.jsonl:567 |
| 5 | 2026-10-24T14:08:14Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=88855 · rate_mib_s=24279 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:07:32Z 후 0m42s) | docs/logs/main/events/watchdog.jsonl:788 |
| 6 | 2026-10-24T14:33:19Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:07:31Z 후 25m48s) | docs/logs/main/events/2026-10.jsonl:568 |
| 7 | 2026-10-24T14:33:19Z | main | `serve_stop` | — | serve_stop · model=qwen3.8-flash-next · duration_s=1548 · session=serve-q38fn31-nv4-mmap-1m-tp1-20261024T140731Z | docs/logs/main/events/2026-10.jsonl:569 |
| 8 | 2026-10-24T14:33:20Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-24T14:07:32Z 후 25m48s) | docs/logs/main/events/watchdog.jsonl:789 |
| 9 | 2026-10-25T00:44:22Z | main | `budget_declare` | `serve-q38fn31-nv4-mmap-1m-tp1` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=12599 · weights_mib=77705 · kv_mib=15872 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 2/3: budget_renew 1회 · 닫힘 budget_clear 2026-10-25T00:56:05Z(선언 후 11m43s) · measured_utc 2026-10-25T01:22:24Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-10.jsonl:622 |
| 10 | 2026-10-25T00:44:22Z | main | `serve_start` | — | serve_start · model=qwen3.8-flash-next · tensor_parallel_size=1 · topology=single · session=serve-q38fn31-nv4-mmap-1m-tp1-20261025T004422Z | docs/logs/main/events/2026-10.jsonl:623 |
| 11 | 2026-10-25T00:44:23Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-25T00:44:22Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:624 |
| 12 | 2026-10-25T00:44:23Z | main | `budget_honored` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_honored · floor_mib=12599 · arm_ceiling_mib=9527 · remaining_s=7199 · 창 시작 2026-10-25T00:44:23Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:811 |
| 13 | 2026-10-25T00:44:58Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=71844 · rate_mib_s=27971 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:44:23Z 후 0m35s) | docs/logs/main/events/watchdog.jsonl:812 |
| 14 | 2026-10-25T00:56:05Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:44:22Z 후 11m43s) | docs/logs/main/events/2026-10.jsonl:625 |
| 15 | 2026-10-25T00:56:05Z | main | `serve_stop` | — | serve_stop · model=qwen3.8-flash-next · duration_s=703 · session=serve-q38fn31-nv4-mmap-1m-tp1-20261025T004422Z | docs/logs/main/events/2026-10.jsonl:626 |
| 16 | 2026-10-25T00:56:05Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:44:23Z 후 11m42s) | docs/logs/main/events/watchdog.jsonl:813 |
| 17 | 2026-10-25T00:56:37Z | main | `budget_declare` | `serve-q38fn31-nv4-mmap-1m-tp1` | 예산 선언(선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정) · floor_mib=12599 · weights_mib=77705 · kv_mib=15872 · overhead_mib=18432 · mem_total_mib=124608(러너가 기동 시 잰 노드 MemTotal — 선언값 아님(blackbox_session declare-budget --mem-total-mib = /proc/meminfo MemTotal)) · 기동 창 3/3: budget_renew 4회 · 닫힘 budget_clear 2026-10-25T01:22:38Z(선언 후 26m01s) · measured_utc 2026-10-25T01:22:24Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-10.jsonl:627 |
| 18 | 2026-10-25T00:56:37Z | main | `serve_start` | — | serve_start · model=qwen3.8-flash-next · tensor_parallel_size=1 · topology=single · session=serve-q38fn31-nv4-mmap-1m-tp1-20261025T005637Z | docs/logs/main/events/2026-10.jsonl:628 |
| 19 | 2026-10-25T00:56:37Z | main | `budget_honored` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_honored · floor_mib=12599 · arm_ceiling_mib=9527 · remaining_s=7200 · 창 시작 2026-10-25T00:56:37Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:814 |
| 20 | 2026-10-25T00:56:38Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=7199 · 창 시작 2026-10-25T00:56:37Z 후 0m01s | docs/logs/main/events/2026-10.jsonl:629 |
| 21 | 2026-10-25T00:56:53Z | main | `engine_log_window_start` | — | 캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 2026-10-25T00:56:37Z 후 0m16s · 시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [2026-10-25T00:56:37Z, 2026-10-25T01:22:38Z] 안에 드는 것을 확인 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:37 |
| 22 | 2026-10-25T00:57:12Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=92461 · rate_mib_s=20821 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:56:37Z 후 0m35s) | docs/logs/main/events/watchdog.jsonl:815 |
| 23 | 2026-10-25T00:57:14Z | main | `engine_prefetch_disabled` | — | 가중치 auto-prefetch 꺼짐 줄 ×1 — filesystem CIFS(vLLM 로그 문구) | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:99 |
| 24 | 2026-10-25T01:05:56Z | main | `engine_weights_loaded` | — | 가중치 적재 완료 줄 ×1(rank·단계별) · 최장 521.92s · 이 행 = 마지막 줄 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:114 |
| 25 | 2026-10-25T01:06:08Z | main | `engine_model_loaded` | — | 모델 적재 완료 줄 ×1(rank 별) · 마지막 줄 74.14 GiB · 539.6s | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:116 |
| 26 | 2026-10-25T01:06:37Z | main | `engine_kv_cache_sized` | — | GPU KV cache 1,237,563 tokens · 요청당 1,048,576 tokens 기준 최대 동시성 1.18x | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:130 |
| 27 | 2026-10-25T01:06:43Z | main | `engine_init_done` | — | 엔진 초기화(profile·KV 할당·warmup) 34.35s | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:144 |
| 28 | 2026-10-25T01:06:52Z | main | `server_starting` | — | API 서버 기동 — 이 측정 창 선언 2026-10-25T00:56:37Z 후 10m15s · 캡처 로그 첫 시각 2026-10-25T00:56:53Z 후 9m59s | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log:160 |
| 29 | 2026-10-25T01:08:14Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=6504 · 창 시작 2026-10-25T00:56:37Z 후 11m37s | docs/logs/main/events/2026-10.jsonl:630 |
| 30 | 2026-10-25T01:13:02Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=6912 · 창 시작 2026-10-25T00:56:37Z 후 16m25s | docs/logs/main/events/2026-10.jsonl:631 |
| 31 | 2026-10-25T01:17:44Z | main | `budget_renew` | `serve-q38fn31-nv4-mmap-1m-tp1` | budget_renew · floor_mib=12599 · ttl_s=7200 · remaining_before_s=6918 · 창 시작 2026-10-25T00:56:37Z 후 21m07s | docs/logs/main/events/2026-10.jsonl:632 |
| 32 | 2026-10-25T01:22:38Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:56:37Z 후 26m01s) | docs/logs/main/events/2026-10.jsonl:633 |
| 33 | 2026-10-25T01:22:38Z | main | `serve_stop` | — | serve_stop · model=qwen3.8-flash-next · duration_s=1561 · session=serve-q38fn31-nv4-mmap-1m-tp1-20261025T005637Z | docs/logs/main/events/2026-10.jsonl:634 |
| 34 | 2026-10-25T01:22:38Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → serve-q38fn31-nv4-mmap-1m-tp1 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-10-25T00:56:37Z 후 26m01s) | docs/logs/main/events/watchdog.jsonl:816 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-q38fn31-nv4-mmap-1m-tp1` | 2026-10-24T14:07:31Z | 1회 · 첫 2026-10-24T14:07:33Z | 없음 | `budget_clear` 2026-10-24T14:33:19Z | 5 |
| 2 | main | `serve-q38fn31-nv4-mmap-1m-tp1` | 2026-10-25T00:44:22Z | 1회 · 첫 2026-10-25T00:44:23Z | 없음 | `budget_clear` 2026-10-25T00:56:05Z | 5 |
| 3 | main | `serve-q38fn31-nv4-mmap-1m-tp1` | 2026-10-25T00:56:37Z | 4회 · 첫 2026-10-25T00:56:38Z | 없음 | `budget_clear` 2026-10-25T01:22:38Z | 12 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
**측정 실행의 env 관측 4행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다) — 그중 4행은 종류 `key-present`: 키가 프로세스 env 에 **있었다**는 관측뿐이고 값은 로그에 없다(값 미관측)

| # | 키 | 값 | 노드 | 종류 | 출처(첫 줄) | 관측 |
|---|---|---|---|---|---|---|
| 1 | `VLLM_PLE_MMAP` | (키 존재 · 값 미관측) | 미특정 | key-present | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:L44 | 로그 2개 |
| 2 | `VLLM_PLE_MMAP_DIR` | (키 존재 · 값 미관측) | 미특정 | key-present | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:L45 | 로그 2개 |
| 3 | `VLLM_REF` | (키 존재 · 값 미관측) | 미특정 | key-present | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:L46 | 로그 2개 |
| 4 | `VLLM_PLE_MMAP_VERIFY` | (키 존재 · 값 미관측) | 미특정 | key-present | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:L47 | 로그 2개 |

- 값 주: 키 존재만 관측 · 값 미관측(vLLM 'Unknown vLLM environment variable detected' 경고 — 값은 로그에 없다)
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**(a) 사전 준비.** 노드는 하나다(메인 · 싱글 TP=1).
- 가중치: 체크포인트는 관리 NAS(CIFS)에 있고 compose 가 `${QUANT_MODEL_PATH}:/app/quant_models:ro` 로 넣는다(`.env.template` 의 `QUANT_MODEL_PATH=<manifest.quant_model_path>`). yaml `model: /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4` 가 그 안을 가리킨다.
- PLE 스테이징: **노드 로컬 디스크**의 `<manifest.ple_mmap_host_path>/q38fn-nvfp4/` 에 PLE 샤드가 든 safetensors 파일이 있어야 한다. compose 가 `${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro` 로 넣고, 셀 env `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4` 가 가리킨다. 측정 기동은 그 디렉터리에서 1파일 128샤드 47.7 GiB 를 찾았다(lite_engine L93). 만드는 법은 1.6 (b).
- 필수 env 의 자리: 셀 env(`artifacts/triplet/.env.q38fn31-nv4-mmap-1m-tp1`)의 `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR` · `VLLM_PLE_MMAP_VERIFY` · `CONFIG_FILE` · `IMAGE_TAG` 와 프로젝트 env 형상(`artifacts/compose/.env.template`)의 마운트 4변수. 위 '측정 실행의 env 관측' 표는 비어 있다(엔진 로그에 env echo 줄 없음). 대신 측정 엔진 로그는 `VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR` · `VLLM_PLE_MMAP_VERIFY` 가 프로세스 env 에 있었음을 "Unknown vLLM environment variable detected" 경고로 보여 준다(L44~L47 · 값은 찍지 않는다).

**(b) 빌드.** 이 셀은 이미지를 새로 짓지 않았다 — 기존 공유 이미지 `easy-vllm:0.31.0-cu133-aarch64-source`(digest `sha256:339737df…`)를 썼다. 다시 지을 때는 위 1. build 명령(빌드 원장 build-arg 에서 재구성 — 실행 원문 아님)으로 메인 노드에서 짓는다. 위 표의 '층 창 38분 19초' 는 캐시 재사용 층을 포함한 층 시각 범위이고 빌드 소요가 아니다.

**(c) 기동 순서와 성공 판정.** 러너 `single_serve_up.sh q38fn31-nv4-mmap-1m-tp1` 이 모델 · RAM 게이트 → 예산 선언 → compose 기동 → health 를 묶는다(위 2. serve). 측정 기동의 관측 시각(선언 00:56:37Z 기준 · 엔진 로그는 컨테이너 시각을 UTC 로 읽은 값):
- 플러그인 설치 00:57:08(L75) → 가중치 적재 521.92 s(L114) → 모델 적재 74.14 GiB · 539.58 s(L116) → KV 1,237,563 토큰(L130) → 엔진 초기화 34.35 s(L144) → 서버 시작 01:06:52(L160 · 선언 후 10분 15초).
- 성공 판정: health 200 · 추론 1회(위 FACT 근거 파일) + 플러그인 발화 서명 셋 — `PLE mmap backend installed` · `128 shards in 1 files … dir=/app/ple_mmap/q38fn-nvfp4` · `verify OK`(VERIFY>0 일 때) — 그리고 `verify FAILED` 0줄.
- 기동 총 소요: 이 측정 기동은 선언 → 서버 시작 10분 15초(관측). 계보 경유 문서 `testlog_26102500_…`(읽기 목록 밖)는 이 셀의 READY 를 645 s 로 적는데, 그것은 시도 1(S3 · 2026-10-24T14:07:31Z 선언 · 그 시도의 엔진 로그 서버 시작 14:18:10)의 값으로 보인다(시각 정합 · 추론).
- 무서워 보이지만 벽이 아닌 것: 적재 초반 원장의 `watchdog_highrate_hold` 1회(시도마다 · 사살 없음), `[transformers] Unrecognized keys in rope_parameters for 'rope_type'='default'` 반복 경고(L48~L59 등 — 체크포인트 원래 rope 를 읽는 단계의 경고이고 측정 기동은 1048576 으로 떴다), `Failed to import the DeepSelect extension`(L60), auto-prefetch 끔(L99 — CIFS 가 인식된 네트워크 FS 가 아니고 체크포인트가 가용 RAM 90% 초과라는 두 조건).

**(d) 호스트 메모리 안전 예산.** 원장 선언(mem_total 은 러너가 기동 때 `/proc/meminfo` 에서 잰 값 · 나머지는 선언값): `mem_total 124608 · weights 77705 · kv 15872 · overhead 18432 → floor 12599 MiB` · `arm_ceiling 9527`(행 17 · 19). 엔진 실측과 나란히: 모델 적재 74.14 GiB · KV 15.5 GiB. 기동 시도는 표와 같이 **3회**이고 셋 다 선언 수리 · 사살 0 · `budget_clear` 로 닫혔다. 시도 1 은 S3(needle · lite)로 보이고(02 §2.2 · 추론), 시도 2 의 내용은 미확인, 시도 3 이 이 측정이다. 벤치는 이 바닥에서 bench 예산 2048 MiB 로 들어가야 한다(4096 BLOCK · 02 W7).

**(e) 측정 명령.** 위 3. bench 의 두 줄은 lite 레그(cold · warm)를 bench JSON 에서 재구성한 것이고 실행 원문이 아니다(입출력 길이는 chat 템플릿 포함 평균 — 도구 원문은 512/128 · 03 §3.4). full 판정점(GuideLLM × 3)은 이 표에 명령이 없다 — 03 §3.4 의 `run_bench.sh@3fa6eac00a18` 원문 L439-453 이 그 명령이다.

**(f) 엔진이 스스로 고른 경로.** world_size 1 · NCCL backend 표기(L77 · 단일 rank 라 통신 없음) · V2 Model Runner(L79) · NVFP4 MoE `FLASHINFER_CUTLASS`(L92) · FlashAttention 2(L95) · GDN prefill FlashInfer · decode cuda(L90-91) · eager 로 torch.compile · CUDA graph 끔(L64 · L67) · 비전 경로 warmup(L96). KV 는 `kv_cache_memory_bytes` 로 프로파일링 없이 15.5 GiB 를 잡았고 gmu 는 따르지 않는다고 적었다(L129). 생성 기본값은 모델 `generation_config.json` 의 `temperature 1.0 · top_k 20 · top_p 0.95` 로 덮였다(L157). 가중치는 auto-prefetch 를 끄고 읽었고(조건 둘 — CIFS 가 NFS/Lustre 가 아님 · 체크포인트가 가용 RAM 90% 초과), 그때 엔진이 본 가용 RAM 은 34.44 GiB 였다(L98-99).

> [원문] output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_q38fn31-nv4-mmap-1m-tp1.log §L114-116
> (EngineCore pid=316) INFO 10-25 01:05:56 [default_loader.py:484] Loading weights took 521.92 seconds
> (EngineCore pid=316) INFO 10-25 01:05:57 [nvfp4.py:612] Using MoEPrepareAndFinalizeNoDPEPModular
> (EngineCore pid=316) INFO 10-25 01:06:08 [model_runner.py:407] Model loading took 74.14 GiB memory and 539.581230 seconds

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음).
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
해당 없음 — single 토폴로지(서브 노드 없음).
<!-- /FACT:measurement_env_nodes -->

> 해당 없음 — 이 절의 조건 `topology=multi` 이 이 셀에서 성립하지 않는다(`topology=single`).

## 1.6 재현 절차 — 기동 전 준비(사람이 할 일)
> 1.4 의 명령을 치기 **전에** 사람이 손으로 해야 하는 것이다 — 가중치 획득 · 스테이징 생성 · 네트워크가 필요한 단계. 에이전트가 대신 할 수 없는 자리(다운로드 승인 · 물리 경로 · 외부 계정)가 여기 모인다.

> 이 절이 답하는 질문: 이 셀을 기동하기 전에 사람이 준비해야 하는 것은 무엇이며, 각각을 어떤 명령으로 만들고, 그 절차의 출처는 어디인가?

### (a) 가중치 획득 — `nvidia/Qwen3.8-Flash-Next-NVFP4`

- 체크포인트: HF `nvidia/Qwen3.8-Flash-Next-NVFP4` · revision `fc694b54fb0174e0913e6adf86691ef85a4ead47`(00 §0.2 — 체크포인트 git HEAD). 크기 123.53 GiB(`model.safetensors.index.json` `metadata.total_size` · `testlog_26102406_…` §근거 1) · shard 11개(엔진 적재 진행 `11/11` · lite_engine L111).
- 획득 모드: 관리 NAS 상재(`model_source: managed` · 다운로드 불요 — `plan_26090918_qwen38_flashnext_multi_딥캠페인` §2.1). 이 셀은 노드 1개라 한 곳이면 된다.
- 받는 명령(이 계보는 다운로드를 하지 않았다 — **추론 · 실행 검증 ✗** · 모델 획득은 사람 승인 게이트다):
  `huggingface-cli download nvidia/Qwen3.8-Flash-Next-NVFP4 --revision fc694b54fb0174e0913e6adf86691ef85a4ead47 --local-dir <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`
- 크기 확인은 `du` 가 아니라 index 의 `total_size` 로 한다(NAS `du` 248G · 02 §2.6).

### (b) PLE 스테이징 — `<manifest.ple_mmap_host_path>/q38fn-nvfp4/`

- 무엇을: 체크포인트의 PLE 샤드 파일 `model-fp8-mtp-ple.safetensors`(50.03 GiB · PLE 샤드 128개 47.68 GiB 포함 — `testlog_26102420_…` §2). 플러그인은 스테이징 디렉터리의 `*.safetensors` 헤더만 훑어 `…ngram_embedding.shard_<i>.weight` 를 찾는다(번들 `table.py` `find_shards`) — index.json 은 플러그인이 읽지 않는다.
- 이력: 이 스테이징은 이번에 만든 것이 아니다. 이전 vLLM 멀티 캠페인 때 양 노드 로컬 NVMe 에 `model-fp8-mtp-ple.safetensors + index.json` 53.7GB 를 두었고(`testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정` §4), 22셀 재수행 plan 이 "mmap 스테이징 양 변종 준비됨(`ple_staging/q38fn-{fp8,nvfp4}`)" 이라 적는다(`plan_26091108_qwen38fn_22셀_재수행_캠페인` §2). 그것을 만든 명령은 계보에 **미기록**이다.
- 위치 조건: 노드 로컬 디스크(compose 주석 "로컬 NVMe mmap 스테이징"). NAS(CIFS) 위 mmap 은 S1 판정이 느릴 수 있다고 적은 불확실성이다(`testlog_26102420_…` §4 ②).
- 만드는 명령(**추론 · 실행 검증 ✗**): `mkdir -p <manifest.ple_mmap_host_path>/q38fn-nvfp4 && cp <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/model-fp8-mtp-ple.safetensors <manifest.ple_mmap_host_path>/q38fn-nvfp4/`
- 생성 확인: 원본과 `cmp`(또는 `sha256sum` 대조) · 기동 뒤 엔진 로그에 `128 shards in 1 files, 320001536 rows x 160 B (47.7 GiB on disk, dir=/app/ple_mmap/q38fn-nvfp4)`(lite_engine L93) · `VLLM_PLE_MMAP_VERIFY>0` 이면 `verify OK`.
- manifest 에 `ple_mmap_host_path` 를 기입하고 프로젝트 `.env` 로 materialize 해야 compose 가 마운트한다(W6 · `.env.template` 의 `PLE_MMAP_HOST_PATH`).

### (c) 네트워크가 필요한 단계

- 가중치 다운로드(위 (a)) — 오프라인 대체: NAS 에 미리 받아 둔다(이 셀이 그렇게 했다).
- 이미지 빌드 — 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` pull · vLLM 소스 `https://github.com/vllm-project/vllm.git` `v0.31.0` clone · `requirements.txt` pip 설치 · Ray 2.48.0(build-arg) — 오프라인 대체: 이미 지은 이미지를 노드에서 재사용한다(이 셀은 재빌드 없음 · 이미지 save/load 전송은 이 저장소 정책상 쓰지 않는다).
- 측정 클라이언트 GuideLLM 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` pull(스윕 meta `bench_tool_image_ref` · digest `sha256:e3ad2371…`) — 오프라인 대체: 미리 pull 해 둔다. GuideLLM 컨테이너는 `HF_HUB_OFFLINE=1` 로 돌고 토크나이저는 호스트에서 스테이징해 넣는다(`run_bench.sh@3fa6eac00a18` L444 · L449).
- 플러그인 번들 자체는 패치 파일 안에 들어 있어 네트워크가 필요 없다(1.2 runtime_patch).
