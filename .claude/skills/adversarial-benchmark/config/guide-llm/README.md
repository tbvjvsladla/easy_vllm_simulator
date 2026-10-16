# config/guide-llm — full-bench 프리셋

full-bench(현재 구현체 GuideLLM `v0.7.3`)의 측정 형상을 정하는 프리셋 폴더다. 바인딩은 `../bench_bindings.yaml` 의
`full-bench: guide-llm` 한 줄이다.

- `default_preset.yaml` — 유일한 기본값 층. git 이 추적하고 배포된다.
- `<이름>.yaml` — 사용자 프리셋. **git 이 추적하지 않는다**(`.gitignore`). 노드마다 자기 프리셋을 직접 만든다.

스크립트에는 측정 형상 기본값이 없다. `sweep_bench.sh --preset <이름>` 이 프리셋을 고르고(미지정 = `default_preset`),
`scripts/bench_preset.py` 가 검증·해소하고, 결과를 `sweep_index.json` 의 `bench_knobs` 에 기록한다.

## 1. 편집 규칙

- 값만 바꾼다. 키를 늘리거나 지우면 계약 변경이다. `selftest_bench_preset.py`(리뷰 시점)와 `bench_preset.py`(기동 시점)가
  둘 다 거부한다.
- 모든 프리셋이 모든 knob 을 선언한다. 사용자 프리셋은 `default_preset.yaml` 을 복사해서 값만 바꾸는 것이 출발점이다.
- 다른 도구의 블록이나 꺼 둔 블록을 실제 키로 남기면 미지 키로 기동이 거부된다.

## 2. knob 읽는 법

knob 마다 세 가지가 보인다.

| 필드 | 뜻 |
|---|---|
| `owner` | 누가 조정하나. `agent` = 서빙전략(explorer 산출)에서 Agent 가 고른다 · `user` = 사용자만 바꾼다 |
| `options` | 고를 수 있는 값의 닫힌 목록. 목록 밖 값은 누가 넣어도 기동 거부다. 선택지를 늘리려면 프리셋을 고친다 |
| `grade` | 그 값을 누가 열 수 있나. `auto` = Agent 가 고른다 · `confirm` = 셀별 사전 승인(`--approved-by`)이 있을 때만 · `escalate` = 사람 핀으로만(`campaign.yaml budgets.bench_pins` · CLI `--pin K=V --pinned-by`) |

- 등급은 정적으로 적는다. 자기신뢰도나 확률로 등급을 고르지 않는다.
- `owner: user` knob 의 비기본 값은 전부 `escalate` 다.
- `default` 는 `options` 안에 있어야 하고 `auto` 여야 한다.

**해소 순서**: 사람 핀(CLI `--pin --pinned-by`) > 사람 핀(`campaign.yaml`) > Agent 선택(`--knob-reason` 근거 코드 필수) > 프리셋 `default`.

**`serving_cap`**(explorer 서빙 yaml 의 한계로 선택지를 거르는 상한 필터 · 값을 만들지는 않는다):

| 값 | 뜻 |
|---|---|
| `le_max_num_seqs` | 레벨 원소 ≤ `max-num-seqs` 만 남긴다 |
| `input_plus_output_le_max_model_len` | `input_len + output_len ≤ max-model-len` 이어야 한다 |
| `none` | 거르지 않는다 |

## 3. 데이터셋 knob (2026-10-16 · plan_26101614)

입력 텍스트의 **출처**만 바꾼다. 길이와 판정은 그대로다.

- 입력은 전처리가 `input_len` 토큰에 정확히 맞춘다(모델 토크나이저 기준). 출력은 `ignore_eos` 로 `output_len` 에 고정한다.
- 판정(`verdict_rule.py` · roofline)은 데이터셋과 무관하게 결과 지표만 본다. 단, **CNN 셀은 CNN 셀끼리만 비교한다**.
- 전부 `owner: user` 다. 스윕 플래그가 없다 — 프리셋을 작성하거나 사람 핀(`--pin`)으로만 바뀐다.

| knob | 값 | 닫는 방식 |
|---|---|---|
| `dataset_kind` | `synthetic` \| `parquet_file` \| `json_file` \| `csv_file` | 닫힌 목록 |
| `dataset_source` | `managed`(공유 NAS · read-only) \| `custom`(노드 로컬 경로) \| `ephemeral` | 닫힌 목록. `ephemeral` 은 이번 개정에서 경로가 없어 기동 거부 |
| `dataset_id` | 사람이 읽는 식별자. 예: `abisee/cnn_dailymail 3.0.0 test` | 한 줄 문자열 |
| `dataset_path` | 원본 파일의 절대경로 | 절대경로 · `..` 금지. 측정 시점에 실재·읽기 가능 검사 |
| `dataset_revision` | HF 커밋 SHA | 40자 소문자 hex |
| `source_sha256` | 원본 파일 sha256 | 64자 소문자 hex. 측정 시점에 실측과 대조 — 다르면 거부 |
| `column_map` | `{text_column: <원본의 텍스트 열>}` | 키는 `text_column` 하나. 전처리 때 열이 없으면 실패 |
| `preprocess_strategy` | `concatenate` \| `ignore` \| `pad` \| `error` | 닫힌 목록 |
| `seed` | 정수 | 전처리·샘플링 고정 seed |

교차 규칙: `dataset_kind: synthetic` 이면 나머지 8개는 전부 `null`. 그 밖의 값이면 8개 전부 값이 있어야 한다.
`--tool vllm` 에 `synthetic` 아닌 데이터셋을 주면 거부한다(실제 데이터셋 입력은 full-bench 전용). lite 벤치는 데이터셋 knob 이 없다.

**어떻게 도나**: 스윕 시작 시 원본의 sha256 을 대조한다. 첫 런 직전에 같은 GuideLLM 이미지로 **네트워크를 끈 컨테이너**를
따로 띄워 `guidellm preprocess dataset` 을 그 모델의 토크나이저로 한 번 돌린다(스윕 폴더에 캐시). 레벨·반복마다 **겹치지 않는
행**을 잘라 쓴다. 행이 모자라면 멈추지 않고 seed 고정 랜덤 중복 샘플링을 하고, 그 사실을 기록한다.

**기록되는 것**(리포트 측정 구성 표 · 인증서 소프트 지문 · hint `03-benchmark.md`): `dataset_kind` · `dataset_source` ·
`dataset_id` · `dataset_revision` · `source_sha256` · `column_map` · `preprocess_strategy` · `seed` · `duplicate_sampling` ·
전처리 토크나이저. synthetic 도 `dataset_kind: synthetic` 으로 기록한다. 이 필드가 없는 과거 인증서는 synthetic 으로 읽는다(소급 ✗).

## 4. 사용자 프리셋 만들기 (예: CNN/DailyMail)

### 4.1 원본 확보 — 사람 승인이 필요하다

데이터셋 다운로드는 사람이 승인한 대상 1건만 한다. 벤치 컨테이너는 오프라인(`HF_HUB_OFFLINE=1`)이라 원본은 미리 로컬에 둔다.

```bash
# 1) 고정할 revision 조회
curl -s https://huggingface.co/api/datasets/abisee/cnn_dailymail | python3 -c "import json,sys;print(json.load(sys.stdin)['sha'])"
# 2) 그 revision 으로 test split 1파일 다운로드(managed = 공유 NAS 의 데이터셋 루트 아래)
SHA=<위 출력>
DS_ROOT=<NAS 데이터셋 루트>   # 운영자 환경값 — 모델 NAS(manifest nas_model_path)와 같은 공유 스토리지의 dataset 폴더
D=$DS_ROOT/abisee__cnn_dailymail/$SHA/3.0.0
mkdir -p "$D"
curl -sfL -o "$D/test-00000-of-00001.parquet" \
  "https://huggingface.co/datasets/abisee/cnn_dailymail/resolve/$SHA/3.0.0/test-00000-of-00001.parquet"
# 3) sha256 기록
sha256sum "$D/test-00000-of-00001.parquet"
```

공유 NAS 에 이미 있으면(다른 노드가 받았으면) 받지 않는다. `sha256sum` 만 돌려 값이 같은지 본다.

### 4.2 프리셋 파일

```bash
cd .claude/skills/adversarial-benchmark/config/guide-llm
cp default_preset.yaml cnn_preset.yaml      # 비추적 파일 — git 에 올라가지 않는다
```

`cnn_preset.yaml` 에서 데이터셋 knob 9개의 `default` 와 `options` 를 바꾼다. 자유값 knob 은 `options` 에 **그 값 하나만** 둔다
(`grade: auto`). 다른 knob 은 그대로 둔다.

```yaml
  dataset_kind:
    owner: user
    kind: scalar
    default: parquet_file
    serving_cap: none
    options:
      - {value: parquet_file, grade: auto}
    reasons: [preset_default, human_pin]
  dataset_source:
    owner: user
    kind: scalar
    default: managed
    serving_cap: none
    options:
      - {value: managed, grade: auto}
    reasons: [preset_default, human_pin]
  dataset_id:
    owner: user
    kind: text
    default: abisee/cnn_dailymail 3.0.0 test
    serving_cap: none
    options:
      - {value: abisee/cnn_dailymail 3.0.0 test, grade: auto}
    reasons: [preset_default, human_pin]
  dataset_path:
    owner: user
    kind: path
    default: <NAS 데이터셋 루트>/abisee__cnn_dailymail/<SHA>/3.0.0/test-00000-of-00001.parquet
    serving_cap: none
    options:
      - {value: <NAS 데이터셋 루트>/abisee__cnn_dailymail/<SHA>/3.0.0/test-00000-of-00001.parquet, grade: auto}
    reasons: [preset_default, human_pin]
  dataset_revision:
    owner: user
    kind: hex40
    default: <SHA>
    serving_cap: none
    options:
      - {value: <SHA>, grade: auto}
    reasons: [preset_default, human_pin]
  source_sha256:
    owner: user
    kind: hex64
    default: <sha256sum 결과>
    serving_cap: none
    options:
      - {value: <sha256sum 결과>, grade: auto}
    reasons: [preset_default, human_pin]
  column_map:
    owner: user
    kind: map
    default: {text_column: article}
    serving_cap: none
    options:
      - {value: {text_column: article}, grade: auto}
    reasons: [preset_default, human_pin]
  preprocess_strategy:
    owner: user
    kind: scalar
    default: concatenate
    serving_cap: none
    options:
      - {value: concatenate, grade: auto}
    reasons: [preset_default, human_pin]
  seed:
    owner: user
    kind: scalar
    default: 42
    serving_cap: none
    options:
      - {value: 42, grade: auto}
    reasons: [preset_default, human_pin]
```

`<SHA>`·`<sha256sum 결과>` 는 4.1 의 출력으로 바꾼다. `custom` 이면 `dataset_source: custom` 과 그 노드의 로컬 절대경로를 쓴다.

### 4.3 검증과 실행

```bash
S=.claude/skills/adversarial-benchmark/scripts
python3 $S/bench_preset.py validate --preset cnn_preset          # 모양 검사 → OK 여야 한다
bash $S/sweep_bench.sh <config> --tool guidellm --preset cnn_preset --dry-run   # 명령만 확인
bash $S/sweep_bench.sh <config> --tool guidellm --preset cnn_preset \
     --backend <openai|openai-chat> --knob-reason backend=<근거> \
     --bench-budget-mib <N> --knob-reason bench_budget_mib=host_budget
```

`validate` 가 거부하면 메시지가 어느 knob 의 어느 규칙인지 말한다. 실측에서 원본 sha256 이 다르면 측정을 시작하지 않는다.
